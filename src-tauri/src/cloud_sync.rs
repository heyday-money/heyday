use crate::{
    cloud_auth::{self, CloudAuthState, Credentials},
    cloud_devices,
    cloud_e2ee::{self, SyncKey},
    cloud_sync_protocol::{self as protocol, Head, Mutation, Page, PushResult, UNSAFE},
    cloud_sync_store::{self as store, Status},
};
use reqwest::{Client, Response, Url};
use serde::Deserialize;
use sqlx::SqlitePool;

const NETWORK: &str = "Could not sync. Check your connection and try again.";
const ACCESS: &str = "Sync access has ended or has not been granted. Your cloud data is retained.";
const WAIT: &str = "Too many sync requests. Try again in a minute.";
async fn json<T: serde::de::DeserializeOwned>(
    mut response: Response,
    base: &Url,
) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 401 {
            cloud_auth::forget(base).await?;
            return Err(cloud_auth::AUTH_ERROR.into());
        }
        let error = cloud_auth::json::<serde_json::Value>(response).await.ok();
        return Err(match error
            .as_ref()
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
        {
            Some("PREMIUM_REQUIRED") => ACCESS,
            Some("DEVICE_REQUIRED") => "Register this device before syncing.",
            Some("DEVICE_REVOKED") => cloud_auth::AUTH_ERROR,
            Some("RATE_LIMITED") => WAIT,
            Some("UNAVAILABLE") => "Cloud sync is disabled on this service.",
            Some("MUTATION_ID_REUSED") | Some("CONFLICT") => UNSAFE,
            Some("KEY_CHANGED") => crate::cloud_key_protocol::CHANGED,
            Some("CLIENT_UPGRADE_REQUIRED") => {
                "This cloud data needs a newer app. Update before syncing."
            }
            _ => NETWORK,
        }
        .into());
    }
    if response.content_length().is_some_and(|n| n > 4_194_304) {
        return Err(UNSAFE.into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| NETWORK)? {
        if bytes.len() + chunk.len() > 4_194_304 {
            return Err(UNSAFE.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| UNSAFE.into())
}
async fn context() -> Result<(Client, Url, Credentials), String> {
    let base = cloud_auth::origin()?;
    let client = cloud_auth::client()?;
    let credentials = cloud_devices::credentials(&client, &base).await?;
    Ok((client, base, credentials))
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
async fn get_page(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    dataset: &str,
    cursor: Option<&str>,
    boundary: Option<&str>,
    bootstrap: bool,
) -> Result<Page, String> {
    let mut url = base
        .join(if bootstrap {
            "api/v1/sync/snapshot"
        } else {
            "api/v1/sync/pull"
        })
        .map_err(|_| NETWORK)?;
    url.query_pairs_mut().append_pair("datasetId", dataset);
    if let Some(cursor) = cursor {
        url.query_pairs_mut().append_pair("cursor", cursor);
    }
    if let Some(boundary) = boundary {
        url.query_pairs_mut().append_pair("boundary", boundary);
    }
    json(
        client
            .get(url)
            .header("X-Heyday-Sync", "5")
            .header("X-Heyday-Keys", "2")
            .bearer_auth(&credentials.access_token)
            .send()
            .await
            .map_err(|_| NETWORK)?,
        base,
    )
    .await
}
async fn pin(
    base: &Url,
    credentials: &Credentials,
    key: &mut SyncKey,
    head: &Head,
) -> Result<(), String> {
    if head.number()? == key.checkpoint.number()? && head.hash != key.checkpoint.hash {
        return Err(UNSAFE.into());
    }
    if head.number()? > key.checkpoint.number()? {
        cloud_e2ee::save_sync_checkpoint(base, &credentials.account.id, &key.envelope, head)
            .await?;
        key.checkpoint = head.clone();
    }
    Ok(())
}
async fn verified_pages(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
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
        let page = get_page(
            client,
            base,
            credentials,
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
pub(super) async fn verified_head(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    key: &SyncKey,
) -> Result<Head, String> {
    let (_, head) = verified_pages(client, base, credentials, key, Head::default(), None).await?;
    Ok(head)
}
async fn push(
    pool: &SqlitePool,
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    key: &mut SyncKey,
    sent: &Mutation,
) -> Result<(), String> {
    let response: PushResult = json(
        client
            .post(base.join("api/v1/sync/push").map_err(|_| NETWORK)?)
            .header("X-Heyday-Sync", "5")
            .header("X-Heyday-Keys", "2")
            .bearer_auth(&credentials.access_token)
            .json(sent)
            .send()
            .await
            .map_err(|_| NETWORK)?,
        base,
    )
    .await?;
    match response {
        PushResult::Accepted { change } => {
            if change.request != *sent {
                return Err(UNSAFE.into());
            }
            let head = protocol::verify_change(&change, &sent.previous)?;
            key.ring.decode(&change)?;
            // An accepted write is a durable checkpoint even before the next pull.
            pin(base, credentials, key, &head).await?;
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
async fn perform(
    pool: &SqlitePool,
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    mut key: SyncKey,
) -> Result<Status, String> {
    let (identity, identity_id) = key.ring.identity()?;
    let initial = store::binding(pool).await?.ok_or(store::ACCOUNT)?;
    initial.owner(base.as_str(), &credentials.account.id)?;
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
        let (pages, head) = verified_pages(
            client,
            base,
            credentials,
            &key,
            initial.head(),
            initial.cursor.as_deref(),
        )
        .await?;
        pin(base, credentials, &mut key, &head).await?;
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
        bound.owner(base.as_str(), &credentials.account.id)?;
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
            match push(pool, client, base, credentials, &mut key, &sent).await {
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
            let page = get_page(
                client,
                base,
                credentials,
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
                pin(base, credentials, &mut key, &end).await?;
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
    store::status(pool, base.as_str(), &credentials.account.id).await
}
#[tauri::command]
pub async fn cloud_sync_status(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<Status, String> {
    let _auth = auth.operation.lock().await;
    let _db = crate::backups::operation()?;
    let base = cloud_auth::origin()?;
    let credentials = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    store::status(&pool, base.as_str(), &credentials.account.id).await
}
#[tauri::command]
pub async fn cloud_enable_sync(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, CloudAuthState>,
    confirmed: bool,
) -> Result<Status, String> {
    if !confirmed {
        return Err("Confirm that you want to sync categories, accounts, and transactions with this account.".into());
    }
    let _auth = auth.operation.lock().await;
    let _db = crate::backups::operation()?;
    let (client, base, credentials) = context().await?;
    if let Some(bound) = store::binding(&pool).await? {
        bound.owner(base.as_str(), &credentials.account.id)?;
    }
    let key = cloud_e2ee::sync_key(&client, &base, &credentials).await?;
    let page = get_page(
        &client,
        &base,
        &credentials,
        &key.envelope.dataset_id,
        None,
        None,
        true,
    )
    .await?;
    if page.head.number()? < key.checkpoint.number()? {
        return Err(UNSAFE.into());
    }
    store::enable(
        &pool,
        base.as_str(),
        &credentials.account.id,
        &key.envelope.dataset_id,
        &key.ring.identity()?.1,
    )
    .await?;
    crate::cloud_sync_ledger::enable(&pool).await?;
    crate::cloud_sync_income::enable(&pool).await?;
    crate::cloud_sync_planner::enable(&pool).await?;
    crate::cloud_sync_metadata::enable(&pool).await?;
    perform(&pool, &client, &base, &credentials, key).await
}
#[tauri::command]
pub async fn cloud_sync_now(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<Status, String> {
    let _auth = auth.operation.lock().await;
    let _db = crate::backups::operation()?;
    let base = cloud_auth::origin()?;
    let saved = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    let bound = store::binding(&pool).await?;
    if let Some(bound) = &bound {
        bound.owner(base.as_str(), &saved.account.id)?;
    }
    if !bound.is_some_and(|b| b.enabled) {
        return store::status(&pool, base.as_str(), &saved.account.id).await;
    }
    let (client, base, credentials) = context().await?;
    let key = cloud_e2ee::sync_key(&client, &base, &credentials).await?;
    perform(&pool, &client, &base, &credentials, key).await
}
#[tauri::command]
pub async fn cloud_pause_sync(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<Status, String> {
    let _auth = auth.operation.lock().await;
    let _db = crate::backups::operation()?;
    let base = cloud_auth::origin()?;
    let credentials = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    store::pause(&pool, base.as_str(), &credentials.account.id).await?;
    store::status(&pool, base.as_str(), &credentials.account.id).await
}
#[tauri::command]
pub async fn cloud_resolve_sync(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, CloudAuthState>,
    record_id: String,
    local_revision: String,
    remote_version: String,
    choice: String,
    name: Option<String>,
) -> Result<Status, String> {
    let _auth = auth.operation.lock().await;
    let _db = crate::backups::operation()?;
    let base = cloud_auth::origin()?;
    let credentials = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    store::binding(&pool)
        .await?
        .ok_or(store::ACCOUNT)?
        .owner(base.as_str(), &credentials.account.id)?;
    store::resolve(
        &pool,
        &record_id,
        protocol::number(&local_revision)?,
        protocol::number(&remote_version)?,
        &choice,
        name,
    )
    .await?;
    store::status(&pool, base.as_str(), &credentials.account.id).await
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::{cloud_crypto as crypto, cloud_key_protocol::Ring};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use zeroize::Zeroizing;
    fn cursor(dataset: &str, head: &Head) -> String {
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(
                &serde_json::json!({"datasetId":dataset,"sequence":head.sequence,"hash":head.hash}),
            )
            .unwrap(),
        )
    }
    async fn serve(pages: Vec<Page>) -> (Url, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let task = tokio::spawn(async move {
            for page in pages {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut data = Vec::new();
                loop {
                    let mut buf = [0; 2048];
                    let n = stream.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buf[..n]);
                    if data.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let headers = String::from_utf8(data).unwrap().to_lowercase();
                assert!(headers.contains("x-heyday-keys: 2"));
                assert!(headers.contains("authorization: bearer test-token"));
                assert!(!headers.contains("cookie:"));
                let body = serde_json::to_string(&page).unwrap();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        (url, task)
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
        let credentials:Credentials=serde_json::from_value(serde_json::json!({"access_token":"test-token","refresh_token":"unused","expires_at":0,"account":{"id":"alice","email":"alice@example.test"}})).unwrap();
        let mut key = SyncKey {
            ring,
            envelope,
            checkpoint: h2.clone(),
            rotation: None,
            unverified_history: false,
            rotation_supported: true,
        };
        let (base, server) = serve(pages()).await;
        let (files, head) = verified_pages(
            &cloud_auth::client().unwrap(),
            &base,
            &credentials,
            &key,
            Head::default(),
            None,
        )
        .await
        .unwrap();
        server.await.unwrap();
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
        let (base, server) = serve(forked).await;
        assert!(verified_pages(
            &cloud_auth::client().unwrap(),
            &base,
            &credentials,
            &key,
            Head::default(),
            None
        )
        .await
        .is_err());
        server.await.unwrap();
        key.checkpoint.hash = "a".repeat(64);
        let (base, server) = serve(vec![pages().remove(0)]).await;
        assert!(verified_pages(
            &cloud_auth::client().unwrap(),
            &base,
            &credentials,
            &key,
            Head::default(),
            None
        )
        .await
        .is_err());
        server.await.unwrap();
    }
}
