//! Shared sync execution. Callers hold the database operation guard and serialize
//! account/key operations. Adapters retain credentials and checkpoint storage.
use crate::{
    cloud_crypto::KeyEnvelope,
    cloud_key_protocol::{Ring, Rotation},
    cloud_sync_protocol::{self as protocol, Head, Mutation, Page, PushResult, UNSAFE},
    cloud_sync_store::{self as store, Status},
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::future::Future;
const NETWORK: &str = "Could not sync. Check your connection and try again.";
/// Authenticated transport and durable platform-secure checkpoint storage.
/// A successful checkpoint write must survive restart; never store it in SQLite.
pub trait Adapter: Sync {
    fn page(
        &self,
        dataset: &str,
        cursor: Option<&str>,
        boundary: Option<&str>,
        bootstrap: bool,
    ) -> impl Future<Output = Result<Page, String>> + Send;
    fn push(&self, sent: &Mutation) -> impl Future<Output = Result<PushResult, String>> + Send;
    fn save_checkpoint(
        &self,
        envelope: &KeyEnvelope,
        head: &Head,
    ) -> impl Future<Output = Result<(), String>> + Send;
}
pub struct SyncKey {
    pub ring: Ring,
    pub envelope: KeyEnvelope,
    pub checkpoint: crate::cloud_sync_protocol::Head,
    pub rotation: Option<Rotation>,
    pub unverified_history: bool,
    pub rotation_supported: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    dataset_id: String,
    sequence: String,
    hash: String,
}
fn cursor_head(cursor: &str, dataset: &str) -> Result<Head, String> {
    let bytes = crate::cloud_crypto::decode(cursor, 1, 384)?;
    let parsed: Cursor = serde_json::from_slice(&bytes).map_err(|_| UNSAFE)?;
    if parsed.dataset_id != dataset {
        return Err(UNSAFE.into());
    }
    let head = Head {
        sequence: parsed.sequence,
        hash: parsed.hash,
    };
    head.number()?;
    Ok(head)
}
async fn pin(adapter: &impl Adapter, key: &mut SyncKey, head: &Head) -> Result<(), String> {
    if head.number()? == key.checkpoint.number()? && head.hash != key.checkpoint.hash {
        return Err(UNSAFE.into());
    }
    if head.number()? > key.checkpoint.number()? {
        adapter.save_checkpoint(&key.envelope, head).await?;
        key.checkpoint = head.clone();
    }
    Ok(())
}
pub async fn verified_pages(
    adapter: &impl Adapter,
    key: &SyncKey,
    start: Head,
    cursor: Option<&str>,
) -> Result<(Vec<tempfile::NamedTempFile>, Head), String> {
    use std::io::Write;
    let mut after = start;
    let mut cursor = cursor.map(str::to_owned);
    let mut boundary: Option<String> = None;
    let mut files = Vec::new();
    let mut total = 0usize;
    for _ in 0..8192 {
        let page = adapter
            .page(
                &key.envelope.dataset_id,
                cursor.as_deref(),
                boundary.as_deref(),
                true,
            )
            .await?;
        let target = cursor_head(&page.boundary, &key.envelope.dataset_id)?;
        if page.dataset_id != key.envelope.dataset_id
            || page.changes.len() > 50
            || (page.has_more && page.changes.is_empty())
            || page.head.number()? < key.checkpoint.number()?
            || (page.head.sequence == key.checkpoint.sequence
                && page.head.hash != key.checkpoint.hash)
            || target.number()? < after.number()?
            || target.number()? > page.head.number()?
            || boundary.as_ref().is_some_and(|b| b != &page.boundary)
            || (boundary.is_none() && target != page.head)
        {
            return Err(UNSAFE.into());
        }
        for change in &page.changes {
            after = protocol::verify_change(change, &after)?;
            key.ring.decode(change)?;
            if after.sequence == key.checkpoint.sequence && after.hash != key.checkpoint.hash {
                return Err(UNSAFE.into());
            }
        }
        if cursor_head(&page.cursor, &key.envelope.dataset_id)? != after
            || after.number()? > target.number()?
            || (!page.has_more && after != target)
        {
            return Err(UNSAFE.into());
        }
        let bytes = serde_json::to_vec(&page).map_err(|_| UNSAFE)?;
        total += bytes.len();
        if total > 536_870_912 {
            return Err("Cloud history is too large to verify in this app version. Update the app before syncing.".into());
        }
        let mut file = tempfile::NamedTempFile::new().map_err(|_| NETWORK)?;
        file.write_all(&bytes).map_err(|_| NETWORK)?;
        files.push(file);
        if !page.has_more {
            if after.number()? < key.checkpoint.number()? {
                return Err(UNSAFE.into());
            }
            if let Some(last) = key.ring.history.last() {
                if after.number()? < last.until.number()?
                    || (after.sequence == last.until.sequence && after.hash != last.until.hash)
                {
                    return Err(UNSAFE.into());
                }
            }
            return Ok((files, after));
        }
        cursor = Some(page.cursor);
        boundary = Some(page.boundary);
    }
    Err(
        "Cloud history is too large to verify in this app version. Update the app before syncing."
            .into(),
    )
}
pub async fn verified_head(adapter: &impl Adapter, key: &SyncKey) -> Result<Head, String> {
    let (_, head) = verified_pages(adapter, key, Head::default(), None).await?;
    Ok(head)
}
async fn push(
    pool: &SqlitePool,
    adapter: &impl Adapter,
    key: &mut SyncKey,
    sent: &Mutation,
) -> Result<(), String> {
    let response = adapter.push(sent).await?;
    match response {
        PushResult::Accepted { change } => {
            if change.request != *sent {
                return Err(UNSAFE.into());
            }
            let head = protocol::verify_change(&change, &sent.previous)?;
            key.ring.decode(&change)?;
            // An accepted write is a durable checkpoint even before the next pull.
            pin(adapter, key, &head).await?;
        }
        PushResult::Conflict { head, current } => {
            if head.number()? < key.checkpoint.number()?
                || (head.sequence == key.checkpoint.sequence && head.hash != key.checkpoint.hash)
            {
                return Err(UNSAFE.into());
            }
            if let Some(current) = current {
                protocol::verify_change(&current, &current.request.previous)?;
                key.ring.decode(&current)?;
                if current.request.record_id != sent.record_id {
                    return Err(UNSAFE.into());
                }
            }
            // Definitive rejection permits re-encryption after pulling the winner.
            // Network failures never discard a pending request.
            store::discard_pending(pool).await?;
        }
    }
    Ok(())
}
pub async fn perform(
    pool: &SqlitePool,
    origin: &str,
    account: &str,
    adapter: &impl Adapter,
    mut key: SyncKey,
) -> Result<Status, String> {
    let (identity, identity_id) = key.ring.identity()?;
    let initial = store::binding(pool).await?.ok_or(store::ACCOUNT)?;
    initial.owner(origin, account)?;
    if initial.dataset_id != key.envelope.dataset_id || initial.key_id != identity_id {
        return Err(UNSAFE.into());
    }
    let epoch_head = key
        .ring
        .history
        .last()
        .map(|h| h.until.number())
        .transpose()?
        .unwrap_or(0);
    if initial.enabled
        && (initial.sequence < key.checkpoint.number()? || initial.sequence < epoch_head)
    {
        // Verify the complete prefix against the independent checkpoint BEFORE any
        // financial data is applied. Temporary files contain ciphertext only.
        let (pages, head) =
            verified_pages(adapter, &key, initial.head(), initial.cursor.as_deref()).await?;
        pin(adapter, &mut key, &head).await?;
        let mut previous = initial.head();
        for file in pages {
            let page: Page =
                serde_json::from_slice(&std::fs::read(file.path()).map_err(|_| NETWORK)?)
                    .map_err(|_| UNSAFE)?;
            let mut decoded = Vec::new();
            let mut end = previous.clone();
            for change in page.changes {
                end = protocol::verify_change(&change, &end)?;
                let record = key.ring.decode(&change)?;
                decoded.push((change, record));
            }
            store::apply_page(
                pool,
                &previous,
                &decoded,
                &end,
                &page.cursor,
                !page.has_more,
            )
            .await?;
            previous = end;
        }
    }
    // Bounded work; the scheduler continues any remaining queue next time.
    for _ in 0..12 {
        let bound = store::binding(pool).await?.ok_or(store::ACCOUNT)?;
        bound.owner(origin, account)?;
        if bound.dataset_id != key.envelope.dataset_id || bound.key_id != identity_id {
            return Err(UNSAFE.into());
        }
        if !bound.enabled {
            break;
        }
        if bound.sequence.to_string() == key.checkpoint.sequence
            && bound.hash != key.checkpoint.hash
        {
            return Err(UNSAFE.into());
        }
        if let Some(sent) = store::pending(pool).await? {
            match push(pool, adapter, &mut key, &sent).await {
                Err(e)
                    if e == crate::cloud_key_protocol::CHANGED
                        && sent.payload.key_id != key.envelope.key_id =>
                {
                    // An explicit server rejection proves this old request was not
                    // accepted. Preserve dirty revisions and re-encrypt after pull.
                    store::discard_pending(pool).await?;
                }
                result => result?,
            }
        }
        let mut boundary: Option<String> = None;
        let mut complete = false;
        for _ in 0..8 {
            let bound = store::binding(pool).await?.ok_or(store::ACCOUNT)?;
            let page = adapter
                .page(
                    &bound.dataset_id,
                    bound.cursor.as_deref(),
                    boundary.as_deref(),
                    !bound.bootstrapped,
                )
                .await?;
            let target = cursor_head(&page.boundary, &bound.dataset_id)?;
            if page.dataset_id != bound.dataset_id
                || page.changes.len() > 50
                || (page.has_more && page.changes.is_empty())
                || page.head.number()? < key.checkpoint.number()?
                || (page.head.sequence == key.checkpoint.sequence
                    && page.head.hash != key.checkpoint.hash)
                || target.number()? < bound.sequence
                || target.number()? > page.head.number()?
                || boundary.as_ref().is_some_and(|b| b != &page.boundary)
                || (boundary.is_none() && target != page.head)
            {
                return Err(UNSAFE.into());
            }
            let mut end = bound.head();
            let mut decoded = Vec::new();
            for change in page.changes {
                end = protocol::verify_change(&change, &end)?;
                let record = key.ring.decode(&change)?;
                if end.sequence == key.checkpoint.sequence && end.hash != key.checkpoint.hash {
                    return Err(UNSAFE.into());
                }
                decoded.push((change, record));
            }
            if cursor_head(&page.cursor, &bound.dataset_id)? != end
                || end.number()? > target.number()?
                || (!page.has_more && end != target)
            {
                return Err(UNSAFE.into());
            }
            // Pin before applying data. A crash can leave the pin ahead of SQLite;
            // replaying the immutable history verifies and repairs that situation.
            if end.number()? >= key.checkpoint.number()? {
                pin(adapter, &mut key, &end).await?;
            }
            store::apply_page(
                pool,
                &bound.head(),
                &decoded,
                &end,
                &page.cursor,
                !page.has_more,
            )
            .await?;
            boundary = Some(page.boundary);
            if !page.has_more {
                complete = true;
                break;
            }
        }
        if !complete {
            break;
        }
        if store::pending(pool).await?.is_some() {
            return Err(UNSAFE.into());
        }
        let bound = store::binding(pool).await?.ok_or(store::ACCOUNT)?;
        if store::prepare_epoch(pool, &key.ring.key, &key.envelope.key_id, &identity, &bound)
            .await?
            .is_none()
        {
            break;
        }
    }
    store::status(pool, origin, account).await
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::{cloud_crypto as crypto, cloud_key_protocol::Ring};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use std::{collections::VecDeque, sync::Mutex};
    use zeroize::Zeroizing;
    fn cursor(dataset: &str, head: &Head) -> String {
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(
                &serde_json::json!({"datasetId":dataset,"sequence":head.sequence,"hash":head.hash}),
            )
            .unwrap(),
        )
    }
    struct MemoryAdapter {
        pages: Mutex<VecDeque<Page>>,
        sent: Mutex<Vec<Mutation>>,
        pins: Mutex<Vec<Head>>,
        fail_push: bool,
        fail_pin: bool,
    }
    impl MemoryAdapter {
        fn new(pages: Vec<Page>) -> Self {
            Self {
                pages: Mutex::new(pages.into()),
                sent: Mutex::new(vec![]),
                pins: Mutex::new(vec![]),
                fail_push: false,
                fail_pin: false,
            }
        }
    }
    impl Adapter for MemoryAdapter {
        async fn page(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
            _: bool,
        ) -> Result<Page, String> {
            self.pages
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| "Unexpected page request".into())
        }
        async fn push(&self, sent: &Mutation) -> Result<PushResult, String> {
            self.sent.lock().unwrap().push(sent.clone());
            if self.fail_push {
                return Err(NETWORK.into());
            }
            Ok(PushResult::Accepted {
                change: protocol::Change {
                    sequence: (sent.previous.number()? + 1).to_string(),
                    version: (protocol::number(&sent.base_version)? + 1).to_string(),
                    request: sent.clone(),
                },
            })
        }
        async fn save_checkpoint(&self, _: &KeyEnvelope, head: &Head) -> Result<(), String> {
            if self.fail_pin {
                return Err("Secure store unavailable".into());
            }
            self.pins.lock().unwrap().push(head.clone());
            Ok(())
        }
    }
    #[tokio::test]
    async fn history_stages_only_ciphertext_and_rejects_late_forks_before_returning_any_pages() {
        let root = Zeroizing::new([7; 32]);
        let envelope = crypto::wrap("alice", "dataset", &root, &Zeroizing::new([9; 32])).unwrap();
        let ring = Ring::open("alice", root.clone(), &envelope, None).unwrap();
        let make = |head: &Head, version| {
            let request=protocol::encrypt(&root,"dataset",&envelope.key_id,"test-category",version,head,false,br#"{"schema":1,"entity":"category","id":"test-category","value":{"name":"Private groceries","icon":"utensils","isArchived":false}}"#).unwrap();
            protocol::Change {
                sequence: (head.number().unwrap() + 1).to_string(),
                version: (version + 1).to_string(),
                request,
            }
        };
        let a = make(&Head::default(), 0);
        let h1 = protocol::verify_change(&a, &Head::default()).unwrap();
        let b = make(&h1, 1);
        let h2 = protocol::verify_change(&b, &h1).unwrap();
        let pages = || {
            vec![
                Page {
                    dataset_id: "dataset".into(),
                    head: h2.clone(),
                    boundary: cursor("dataset", &h2),
                    changes: vec![a.clone()],
                    cursor: cursor("dataset", &h1),
                    has_more: true,
                },
                Page {
                    dataset_id: "dataset".into(),
                    head: h2.clone(),
                    boundary: cursor("dataset", &h2),
                    changes: vec![b.clone()],
                    cursor: cursor("dataset", &h2),
                    has_more: false,
                },
            ]
        };
        let mut key = SyncKey {
            ring,
            envelope,
            checkpoint: h2.clone(),
            rotation: None,
            unverified_history: false,
            rotation_supported: true,
        };
        let adapter = MemoryAdapter::new(pages());
        let (files, head) = verified_pages(&adapter, &key, Head::default(), None)
            .await
            .unwrap();
        assert_eq!(head, h2);
        assert_eq!(files.len(), 2);
        let paths: Vec<_> = files.iter().map(|f| f.path().to_owned()).collect();
        for path in &paths {
            let text = std::fs::read_to_string(path).unwrap();
            assert!(!text.contains("Private groceries"));
            assert!(!text.contains(&URL_SAFE_NO_PAD.encode(&*root)));
        }
        drop(files);
        assert!(paths.iter().all(|p| !p.exists()));
        // The server advertises the trusted head but forks on the last page.
        // Earlier valid pages must never be returned for application.
        let mut forked = pages();
        forked[1].changes[0].request.commit_mac = URL_SAFE_NO_PAD.encode([0; 32]);
        let adapter = MemoryAdapter::new(forked);
        assert!(verified_pages(&adapter, &key, Head::default(), None)
            .await
            .is_err());
        key.checkpoint.hash = "a".repeat(64);
        let adapter = MemoryAdapter::new(vec![pages().remove(0)]);
        assert!(verified_pages(&adapter, &key, Head::default(), None)
            .await
            .is_err());
    }
    fn key() -> SyncKey {
        // Reopen the same test installation across simulated process attempts.
        static PACKAGE: std::sync::OnceLock<KeyEnvelope> = std::sync::OnceLock::new();
        let root = Zeroizing::new([7; 32]);
        let envelope = PACKAGE
            .get_or_init(|| {
                crypto::wrap("alice", "dataset", &root, &Zeroizing::new([9; 32])).unwrap()
            })
            .clone();
        let ring = Ring::open("alice", root, &envelope, None).unwrap();
        SyncKey {
            ring,
            envelope,
            checkpoint: Head::default(),
            rotation: None,
            unverified_history: false,
            rotation_supported: true,
        }
    }
    async fn pending_database() -> (SqlitePool, Mutation) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&pool).await.unwrap();
        let k = key();
        store::enable(
            &pool,
            "https://example.test/",
            "alice",
            "dataset",
            &k.envelope.key_id,
        )
        .await
        .unwrap();
        store::apply_page(
            &pool,
            &Head::default(),
            &[],
            &Head::default(),
            &cursor("dataset", &Head::default()),
            true,
        )
        .await
        .unwrap();
        // Isolate one local edit from bundled category bootstrap uploads.
        sqlx::query("DELETE FROM cloud_sync_dirty")
            .execute(&pool)
            .await
            .unwrap();
        crate::transaction_options::save(
            &pool,
            crate::transaction_options::SaveOption {
                logo_change: None,
                kind: crate::transaction_options::OptionKind::Category,
                id: None,
                name: "Offline category".into(),
                is_archived: false,
                icon: Some("tag".into()),
            },
        )
        .await
        .unwrap();
        let bound = store::binding(&pool).await.unwrap().unwrap();
        let sent = store::prepare_epoch(
            &pool,
            &k.ring.key,
            &k.envelope.key_id,
            &k.ring.identity().unwrap().0,
            &bound,
        )
        .await
        .unwrap()
        .unwrap();
        (pool, sent)
    }
    #[tokio::test]
    async fn engine_retries_exact_pending_ciphertext_and_pins_before_application() {
        let (pool, sent) = pending_database().await;
        let mut offline = MemoryAdapter::new(vec![]);
        offline.fail_push = true;
        assert!(
            perform(&pool, "https://example.test/", "alice", &offline, key())
                .await
                .is_err()
        );
        assert_eq!(store::pending(&pool).await.unwrap().unwrap(), sent);
        assert_eq!(*offline.sent.lock().unwrap(), vec![sent.clone()]);
        let mut unavailable = MemoryAdapter::new(vec![]);
        unavailable.fail_pin = true;
        assert!(
            perform(&pool, "https://example.test/", "alice", &unavailable, key())
                .await
                .is_err()
        );
        assert_eq!(
            store::binding(&pool).await.unwrap().unwrap().head(),
            Head::default()
        );
        assert_eq!(store::pending(&pool).await.unwrap().unwrap(), sent);
        let change = protocol::Change {
            sequence: "1".into(),
            version: "1".into(),
            request: sent.clone(),
        };
        let head = protocol::verify_change(&change, &Head::default()).unwrap();
        let online = MemoryAdapter::new(vec![Page {
            dataset_id: "dataset".into(),
            head: head.clone(),
            boundary: cursor("dataset", &head),
            changes: vec![change],
            cursor: cursor("dataset", &head),
            has_more: false,
        }]);
        perform(&pool, "https://example.test/", "alice", &online, key())
            .await
            .unwrap();
        assert_eq!(*online.sent.lock().unwrap(), vec![sent]);
        assert_eq!(*online.pins.lock().unwrap(), vec![head.clone()]);
        assert_eq!(store::binding(&pool).await.unwrap().unwrap().head(), head);
        assert!(store::pending(&pool).await.unwrap().is_none());
        assert_eq!(
            store::status(&pool, "https://example.test/", "alice")
                .await
                .unwrap()
                .pending,
            0
        );
    }
    #[tokio::test]
    async fn engine_rejects_late_fork_before_any_record_or_checkpoint_is_applied() {
        let (pool, _) = pending_database().await;
        store::discard_pending(&pool).await.unwrap();
        let root = Zeroizing::new([7; 32]);
        let mut k = key();
        let request = protocol::encrypt(&root, "dataset", &k.envelope.key_id, "remote-category", 0, &Head::default(), false,
            br#"{"schema":1,"entity":"category","id":"remote-category","value":{"name":"Remote","icon":"tag","isArchived":false}}"#).unwrap();
        let first = protocol::Change {
            sequence: "1".into(),
            version: "1".into(),
            request,
        };
        let h1 = protocol::verify_change(&first, &Head::default()).unwrap();
        let request = protocol::encrypt(&root, "dataset", &k.envelope.key_id, "remote-category", 1, &h1, false,
            br#"{"schema":1,"entity":"category","id":"remote-category","value":{"name":"Later","icon":"tag","isArchived":false}}"#).unwrap();
        let mut second = protocol::Change {
            sequence: "2".into(),
            version: "2".into(),
            request,
        };
        let h2 = protocol::verify_change(&second, &h1).unwrap();
        k.checkpoint = h2.clone();
        second.request.commit_mac = URL_SAFE_NO_PAD.encode([0; 32]);
        let adapter = MemoryAdapter::new(vec![
            Page {
                dataset_id: "dataset".into(),
                head: h2.clone(),
                boundary: cursor("dataset", &h2),
                changes: vec![first],
                cursor: cursor("dataset", &h1),
                has_more: true,
            },
            Page {
                dataset_id: "dataset".into(),
                head: h2.clone(),
                boundary: cursor("dataset", &h2),
                changes: vec![second],
                cursor: cursor("dataset", &h2),
                has_more: false,
            },
        ]);
        assert!(
            perform(&pool, "https://example.test/", "alice", &adapter, k)
                .await
                .is_err()
        );
        assert_eq!(
            store::binding(&pool).await.unwrap().unwrap().head(),
            Head::default()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM categories WHERE id='remote-category'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
        assert!(adapter.pins.lock().unwrap().is_empty());
    }
}
