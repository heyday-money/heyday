use crate::{
    cloud_auth::{self, CloudAuthState, Credentials, NETWORK_ERROR},
    cloud_crypto::{self as crypto, KeyEnvelope, Secret},
    cloud_key_protocol::{self as epochs, Ring, Rotation},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::Mutex;
use zeroize::{Zeroize, Zeroizing};

const STORE: &str = "Could not access the encryption key in the secure store. Please try again.";
const CHANGED: &str = "The saved encryption package does not match the server. Access was blocked to protect your data.";
const EXPIRED: &str = "Encryption setup expired or was cancelled. Start setup again.";
const EXISTS: &str = "Encryption is already configured. Use your existing recovery key.";

struct Pending {
    origin: String,
    account: String,
    generation: u64,
    created: Instant,
    key: Secret,
    recovery: Secret,
    envelope: KeyEnvelope,
    rotation: Option<Rotation>,
}
#[derive(Default)]
pub struct EncryptionState {
    pending: Arc<Mutex<Option<Pending>>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalKey {
    version: u8,
    envelope: KeyEnvelope,
    key: Option<String>,
    confirmed: bool,
    #[serde(default)]
    remember: bool,
    #[serde(default)]
    sync_checkpoint: Option<crate::cloud_sync_protocol::Head>,
    #[serde(default)]
    rotation_hash: Option<String>,
    #[serde(default)]
    unverified_history: bool,
    #[serde(default)]
    pending_rotation: Option<PendingRotation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingRotation {
    envelope: KeyEnvelope,
    key: Option<String>,
    hash: String,
}
impl Drop for PendingRotation {
    fn drop(&mut self) {
        if let Some(key) = self.key.as_mut() {
            key.zeroize();
        }
    }
}
impl Drop for LocalKey {
    fn drop(&mut self) {
        if let Some(key) = self.key.as_mut() {
            key.zeroize();
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote {
    dataset_id: String,
    key: Option<KeyEnvelope>,
    #[serde(default)]
    rotation: Option<Rotation>,
    #[serde(default)]
    key_management_version: Option<u8>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionStatus {
    state: &'static str,
    remember_encryption: bool,
    history_verified: bool,
    rotation_pending: bool,
    unverified_history: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupKey {
    recovery_key: String,
}

fn entry(base: &Url, account: &str) -> Result<keyring::Entry, String> {
    let service = if cfg!(debug_assertions) {
        "money.heyday.desktop.encryption.development"
    } else {
        "money.heyday.desktop.encryption"
    };
    let scope =
        serde_json::to_vec(&(base.origin().ascii_serialization(), account)).map_err(|_| STORE)?;
    keyring::Entry::new(service, &format!("{:x}", Sha256::digest(scope))).map_err(|_| STORE.into())
}
async fn load(base: &Url, account: &str) -> Result<Option<LocalKey>, String> {
    let base = base.clone();
    let account = account.to_owned();
    tauri::async_runtime::spawn_blocking(move || match entry(&base, &account)?.get_password() {
        Ok(value) => {
            let value = Zeroizing::new(value);
            if value.len() > 4096 {
                return Err(STORE.into());
            }
            let record: LocalKey = serde_json::from_str(&value).map_err(|_| STORE)?;
            if record.version != 1 {
                return Err(STORE.into());
            }
            record.envelope.validate()?;
            if let Some(key) = &record.key {
                crypto::secret(key)?;
            }
            if let Some(p) = &record.pending_rotation {
                p.envelope.validate()?;
                if let Some(key) = &p.key {
                    crypto::secret(key)?;
                }
            }
            Ok(Some(record))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(STORE.into()),
    })
    .await
    .map_err(|_| STORE.to_owned())?
}
async fn save(base: &Url, account: &str, record: &LocalKey) -> Result<(), String> {
    let base = base.clone();
    let account = account.to_owned();
    let value = Zeroizing::new(serde_json::to_string(record).map_err(|_| STORE)?);
    tauri::async_runtime::spawn_blocking(move || {
        entry(&base, &account)?
            .set_password(&value)
            .map_err(|_| STORE.into())
    })
    .await
    .map_err(|_| STORE.to_owned())?
}
pub(super) async fn lock_saved(base: &Url, account: &str) -> Result<(), String> {
    if let Some(mut record) = load(base, account).await? {
        clear_key(&mut record);
        save(base, account, &record).await?;
    }
    Ok(())
}

fn clear_key(record: &mut LocalKey) {
    if let Some(mut key) = record.key.take() {
        key.zeroize();
    }
    record.remember = false;
    if let Some(p) = record.pending_rotation.as_mut() {
        if let Some(mut key) = p.key.take() {
            key.zeroize();
        }
    }
}

fn sign_out_key(record: &mut LocalKey, forget_device: bool) {
    if forget_device || !record.remember {
        clear_key(record);
    }
}

pub(super) async fn sign_out_saved(
    base: &Url,
    account: &str,
    forget_device: bool,
) -> Result<(), String> {
    if let Some(mut record) = load(base, account).await? {
        sign_out_key(&mut record, forget_device);
        save(base, account, &record).await?;
    }
    Ok(())
}

fn check_pin(local: Option<&LocalKey>, remote: &Remote) -> Result<(), String> {
    if let Some(local) = local {
        if local.envelope.dataset_id != remote.dataset_id
            || (local.confirmed && remote.key.as_ref() != Some(&local.envelope))
        {
            return Err(CHANGED.into());
        }
    }
    Ok(())
}
fn status(local: Option<&LocalKey>, remote: &Remote) -> Result<EncryptionStatus, String> {
    if let Some(local) = local {
        if local.confirmed
            && remote.key.as_ref() != Some(&local.envelope)
            && remote.rotation.is_some()
        {
            if local.envelope.dataset_id != remote.dataset_id {
                return Err(CHANGED.into());
            }
            return Ok(EncryptionStatus {
                state: "changed",
                remember_encryption: false,
                history_verified: local.sync_checkpoint.is_some() && !local.unverified_history,
                rotation_pending: local.pending_rotation.is_some(),
                unverified_history: local.unverified_history,
            });
        }
    }
    check_pin(local, remote)?;
    let state = match (local, &remote.key) {
        (None, None) => "not_set",
        (Some(_), None) => "pending",
        (Some(local), Some(envelope)) if local.envelope == *envelope && local.key.is_some() => {
            "unlocked"
        }
        (_, Some(_)) => "locked",
    };
    Ok(EncryptionStatus {
        state,
        remember_encryption: local.is_some_and(|l| l.remember && l.key.is_some()),
        history_verified: local
            .is_some_and(|l| l.sync_checkpoint.is_some() && !l.unverified_history),
        unverified_history: local.is_some_and(|l| l.unverified_history),
        rotation_pending: local.is_some_and(|l| l.pending_rotation.is_some()),
    })
}

fn endpoint(base: &Url) -> String {
    format!("{}api/v1/encryption/keys", base.as_str())
}
async fn checked(response: Response, base: &Url) -> Result<Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status.as_u16() == 401 {
        cloud_auth::forget(base).await?;
        return Err(cloud_auth::AUTH_ERROR.into());
    }
    let body = cloud_auth::json::<serde_json::Value>(response).await.ok();
    let code = body
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(|v| v.as_str());
    Err(match (status.as_u16(), code) {
        (403, Some("DEVICE_REQUIRED")) => {
            "Register this device in Manage devices before setting up encryption."
        }
        (403, _) => "This device cannot access encryption setup. Refresh your account and devices.",
        (409, _) => EXISTS,
        (429, _) => "Too many encryption requests. Try again in a minute.",
        (404, _) => {
            "Encryption setup is unavailable on this server. Update the service and try again."
        }
        _ => "Could not reach encryption setup. Check your connection and try again.",
    }
    .into())
}
async fn remote(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    proposed: Option<&KeyEnvelope>,
) -> Result<Remote, String> {
    let request = if let Some(envelope) = proposed {
        client.put(endpoint(base)).json(envelope)
    } else {
        client.get(endpoint(base))
    };
    let response = request
        .bearer_auth(&credentials.access_token)
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let remote: Remote = key_json(checked(response, base).await?).await?;
    if !crate::cloud_devices::valid_uuid(&remote.dataset_id) {
        return Err(crypto::INVALID.into());
    }
    if let Some(key) = &remote.key {
        key.validate()?;
        if key.dataset_id != remote.dataset_id {
            return Err(crypto::INVALID.into());
        }
    }
    if let Some(rotation) = &remote.rotation {
        rotation.validate()?;
        if remote.key.as_ref() != Some(&rotation.key) {
            return Err(CHANGED.into());
        }
    }
    if proposed.is_some_and(|key| remote.key.as_ref() != Some(key)) {
        return Err(CHANGED.into());
    }
    Ok(remote)
}
async fn key_json(mut response: Response) -> Result<Remote, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| NETWORK_ERROR)? {
        if bytes.len() + chunk.len() > 196608 {
            return Err(crypto::INVALID.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| crypto::INVALID.into())
}
fn check_rotation_pin(local: &LocalKey, remote: &Remote, account: &str) -> Result<(), String> {
    let hash = remote.rotation.as_ref().map(|r| r.fingerprint(account));
    if local.rotation_hash != hash {
        return Err(CHANGED.into());
    }
    Ok(())
}
async fn acknowledge(
    base: &Url,
    account: &str,
    local: &mut Option<LocalKey>,
    remote: &Remote,
) -> Result<(), String> {
    if let Some(local) = local {
        if let Some(p) = &local.pending_rotation {
            if remote.key.as_ref() == Some(&p.envelope) {
                if remote
                    .rotation
                    .as_ref()
                    .map(|r| r.fingerprint(account))
                    .as_ref()
                    != Some(&p.hash)
                {
                    return Err(CHANGED.into());
                }
                local.envelope = p.envelope.clone();
                if let Some(mut old) = local.key.take() {
                    old.zeroize();
                }
                local.key = p.key.clone();
                local.rotation_hash = Some(p.hash.clone());
                local.confirmed = true;
                local.pending_rotation = None;
                save(base, account, local).await?;
            }
        }
        if remote.key.as_ref() == Some(&local.envelope) {
            check_rotation_pin(local, remote, account)?;
        }
    }
    Ok(())
}
async fn context() -> Result<(Client, Url, Credentials), String> {
    let base = cloud_auth::origin()?;
    let client = cloud_auth::client()?;
    let credentials = crate::cloud_devices::credentials(&client, &base).await?;
    Ok((client, base, credentials))
}

#[tauri::command]
pub async fn cloud_encryption_status(
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<EncryptionStatus, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote = remote(&client, &base, &credentials, None).await?;
    let mut local = load(&base, &credentials.account.id).await?;
    acknowledge(&base, &credentials.account.id, &mut local, &remote).await?;
    status(local.as_ref(), &remote)?;
    // A read-after-timeout can acknowledge an already committed enrollment.
    if let Some(local) = &mut local {
        if !local.confirmed && remote.key.as_ref() == Some(&local.envelope) {
            local.confirmed = true;
            local.sync_checkpoint = Some(local.sync_checkpoint.clone().unwrap_or_default());
            save(&base, &credentials.account.id, local).await?;
        }
    }
    status(local.as_ref(), &remote)
}

#[tauri::command]
pub async fn cloud_prepare_encryption(
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
) -> Result<SetupKey, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote = remote(&client, &base, &credentials, None).await?;
    let local = load(&base, &credentials.account.id).await?;
    check_pin(local.as_ref(), &remote)?;
    if remote.key.is_some() || local.is_some() {
        return Err(EXISTS.into());
    }
    let generation = auth.generation.load(Ordering::SeqCst);
    let mut pending = state.pending.lock().await;
    if let Some(p) = pending.as_ref() {
        if p.origin == base.as_str()
            && p.account == credentials.account.id
            && p.generation == generation
            && p.created.elapsed() < Duration::from_secs(600)
        {
            return Ok(SetupKey {
                recovery_key: crypto::recovery_text(&p.recovery).to_string(),
            });
        }
    }
    let key = Zeroizing::new(crypto::random::<32>()?);
    let recovery = Zeroizing::new(crypto::random::<32>()?);
    let envelope = crypto::wrap(&credentials.account.id, &remote.dataset_id, &key, &recovery)?;
    let recovery_key = crypto::recovery_text(&recovery).to_string();
    *pending = Some(Pending {
        origin: base.to_string(),
        account: credentials.account.id,
        generation,
        created: Instant::now(),
        key,
        recovery,
        envelope,
        rotation: None,
    });
    let pending = Arc::clone(&state.pending);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(600)).await;
        let mut pending = pending.lock().await;
        if pending
            .as_ref()
            .is_some_and(|p| p.created.elapsed() >= Duration::from_secs(600))
        {
            *pending = None;
        }
    });
    Ok(SetupKey { recovery_key })
}

#[tauri::command]
pub async fn cloud_confirm_encryption(
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
    recovery_key: String,
    saved: bool,
    remember: Option<bool>,
) -> Result<EncryptionStatus, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    if !saved {
        return Err("Save your recovery key and confirm that support cannot recover it.".into());
    }
    let recovery = crypto::parse_recovery(&recovery_key)?;
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote_state = remote(&client, &base, &credentials, None).await?;
    let local = load(&base, &credentials.account.id).await?;
    check_pin(local.as_ref(), &remote_state)?;
    // This command is only initial enrollment; it must not reset a rotation pin
    // or erase an unverified-recovery notice on an existing confirmed key.
    if remote_state.rotation.is_some()
        || local
            .as_ref()
            .is_some_and(|l| l.confirmed || l.pending_rotation.is_some())
    {
        return Err(EXISTS.into());
    }
    let mut pending = state.pending.lock().await;
    let envelope = if let Some(local) = &local {
        local.envelope.clone()
    } else {
        let p = pending.as_ref().ok_or(EXPIRED)?;
        if p.origin != base.as_str()
            || p.account != credentials.account.id
            || p.generation != auth.generation.load(Ordering::SeqCst)
            || p.created.elapsed() >= Duration::from_secs(600)
        {
            return Err(EXPIRED.into());
        }
        p.envelope.clone()
    };
    if remote_state
        .key
        .as_ref()
        .is_some_and(|key| key != &envelope)
    {
        return Err(EXISTS.into());
    }
    let key = crypto::unwrap(
        &credentials.account.id,
        &remote_state.dataset_id,
        &envelope,
        &recovery,
    )?;
    if let Some(p) = pending.as_ref() {
        if p.envelope == envelope && !bool::from(key.as_slice().ct_eq(p.key.as_slice())) {
            return Err(crypto::RECOVERY_ERROR.into());
        }
    }
    let mut record = LocalKey {
        version: 1,
        envelope,
        key: Some(URL_SAFE_NO_PAD.encode(&*key)),
        confirmed: remote_state.key.is_some(),
        remember: remember.unwrap_or(false),
        sync_checkpoint: local.as_ref().and_then(|l| l.sync_checkpoint.clone()),
        rotation_hash: None,
        unverified_history: false,
        pending_rotation: None,
    };
    // Persist before PUT. After a crash, the same envelope can be resumed using
    // the user's saved recovery key; never generate a replacement on a timeout.
    save(&base, &credentials.account.id, &record).await?;
    let accepted = remote(&client, &base, &credentials, Some(&record.envelope)).await?;
    record.confirmed = true;
    record.sync_checkpoint = Some(record.sync_checkpoint.clone().unwrap_or_default());
    save(&base, &credentials.account.id, &record).await?;
    *pending = None;
    status(Some(&record), &accepted)
}

#[tauri::command]
pub async fn cloud_recover_encryption(
    auth: tauri::State<'_, CloudAuthState>,
    recovery_key: String,
    remember: Option<bool>,
    history_code: Option<String>,
    allow_unverified_history: Option<bool>,
) -> Result<EncryptionStatus, String> {
    let recovery_key = Zeroizing::new(recovery_key);
    let recovery = crypto::parse_recovery(&recovery_key)?;
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote = remote(&client, &base, &credentials, None).await?;
    let mut local = load(&base, &credentials.account.id).await?;
    acknowledge(&base, &credentials.account.id, &mut local, &remote).await?;
    let envelope = remote
        .key
        .as_ref()
        .ok_or("Encryption has not been set up for this account.")?;
    let key = crypto::unwrap(
        &credentials.account.id,
        &remote.dataset_id,
        envelope,
        &recovery,
    )?;
    let ring = Ring::open(
        &credentials.account.id,
        key.clone(),
        envelope,
        remote.rotation.as_ref(),
    )?;
    if let Some(local) = &local {
        if local.envelope.dataset_id != remote.dataset_id
            || (local.confirmed
                && !ring.matches_pin(
                    &credentials.account.id,
                    &local.envelope,
                    local.rotation_hash.as_deref(),
                ))
        {
            return Err(CHANGED.into());
        }
        if &local.envelope == envelope {
            check_rotation_pin(local, &remote, &credentials.account.id)?;
        }
    }
    let proof = if let Some(code) = history_code.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(epochs::verify_code(
            code,
            base.as_str(),
            &credentials.account.id,
            &ring,
            remote.rotation.as_ref(),
        )?)
    } else {
        None
    };
    let (checkpoint, unverified_history) = recovery_checkpoint(
        local.as_ref(),
        proof,
        allow_unverified_history == Some(true),
    )?;
    let record = LocalKey {
        version: 1,
        envelope: envelope.clone(),
        key: Some(URL_SAFE_NO_PAD.encode(&*key)),
        confirmed: true,
        remember: remember.unwrap_or(false),
        sync_checkpoint: Some(checkpoint),
        unverified_history,
        rotation_hash: remote
            .rotation
            .as_ref()
            .map(|r| r.fingerprint(&credentials.account.id)),
        pending_rotation: None,
    };
    save(&base, &credentials.account.id, &record).await?;
    status(Some(&record), &remote)
}

fn recovery_checkpoint(
    local: Option<&LocalKey>,
    proof: Option<(crate::cloud_sync_protocol::Head, bool)>,
    allow_unverified: bool,
) -> Result<(crate::cloud_sync_protocol::Head, bool), String> {
    let old = local.and_then(|l| l.sync_checkpoint.clone());
    if let Some((head, unverified)) = proof {
        head.number()?;
        if let Some(old) = old {
            if head.number()? < old.number()?
                || (head.sequence == old.sequence && head.hash != old.hash)
            {
                return Err(crate::cloud_sync_protocol::UNSAFE.into());
            }
        }
        return Ok((head, unverified));
    }
    if let Some(old) = old {
        old.number()?;
        return Ok((old, local.is_some_and(|l| l.unverified_history)));
    }
    if !allow_unverified {
        return Err(epochs::HISTORY.into());
    }
    Ok((crate::cloud_sync_protocol::Head::default(), true))
}

#[tauri::command]
pub async fn cloud_remember_encryption(
    auth: tauri::State<'_, CloudAuthState>,
    remember: bool,
) -> Result<EncryptionStatus, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote = remote(&client, &base, &credentials, None).await?;
    let mut local = load(&base, &credentials.account.id).await?;
    acknowledge(&base, &credentials.account.id, &mut local, &remote).await?;
    if status(local.as_ref(), &remote)?.state != "unlocked" {
        return Err("Set up or unlock encryption before syncing.".into());
    }
    let record = local.as_mut().ok_or(STORE)?;
    record.remember = remember;
    save(&base, &credentials.account.id, record).await?;
    status(Some(record), &remote)
}

#[tauri::command]
pub async fn cloud_lock_encryption(
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
) -> Result<(), String> {
    let _guard = auth.operation.lock().await;
    let base = cloud_auth::origin()?;
    let credentials = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    lock_saved(&base, &credentials.account.id).await?;
    *state.pending.lock().await = None;
    Ok(())
}

pub(super) use heyday_core::sync::engine::SyncKey;
pub(super) async fn sync_key(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
) -> Result<SyncKey, String> {
    let remote = remote(client, base, credentials, None).await?;
    let mut local = load(base, &credentials.account.id).await?;
    acknowledge(base, &credentials.account.id, &mut local, &remote).await?;
    if status(local.as_ref(), &remote)?.state == "changed" {
        return Err(epochs::CHANGED.into());
    }
    check_pin(local.as_ref(), &remote)?;
    let local = local.ok_or("Set up or unlock encryption before syncing.")?;
    if remote.key.as_ref() != Some(&local.envelope) {
        return Err(CHANGED.into());
    }
    let key = crypto::secret(
        local
            .key
            .as_deref()
            .ok_or("Set up or unlock encryption before syncing.")?,
    )?;
    if local.pending_rotation.is_some() {
        return Err("Finish or cancel the pending recovery-key replacement before syncing.".into());
    }
    check_rotation_pin(&local, &remote, &credentials.account.id)?;
    let checkpoint = local.sync_checkpoint.clone().ok_or(epochs::HISTORY)?;
    checkpoint.number()?;
    Ok(SyncKey {
        ring: Ring::open(
            &credentials.account.id,
            key,
            &local.envelope,
            remote.rotation.as_ref(),
        )?,
        envelope: local.envelope.clone(),
        checkpoint,
        rotation: remote.rotation,
        unverified_history: local.unverified_history,
        rotation_supported: remote.key_management_version == Some(2),
    })
}
pub(super) async fn save_sync_checkpoint(
    base: &Url,
    account: &str,
    envelope: &KeyEnvelope,
    checkpoint: &crate::cloud_sync_protocol::Head,
) -> Result<(), String> {
    let mut local = load(base, account).await?.ok_or(STORE)?;
    if &local.envelope != envelope || local.key.is_none() {
        return Err(CHANGED.into());
    }
    let old = local.sync_checkpoint.clone().unwrap_or_default();
    if checkpoint.number()? < old.number()?
        || (checkpoint.sequence == old.sequence && checkpoint.hash != old.hash)
    {
        return Err(crate::cloud_sync_protocol::UNSAFE.into());
    }
    local.confirmed = true;
    local.sync_checkpoint = Some(checkpoint.clone());
    save(base, account, &local).await
}

fn rotation_path(
    app: &tauri::AppHandle,
    base: &Url,
    account: &str,
    hash: &str,
) -> Result<std::path::PathBuf, String> {
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(STORE.into());
    }
    let scope = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(base.as_str(), account)).map_err(|_| STORE)?)
    );
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| STORE)?
        .join("cloud-key-packages")
        .join(if cfg!(debug_assertions) {
            "development"
        } else {
            "release"
        })
        .join(scope)
        .join(format!("{hash}.json")))
}
fn persist_rotation(
    app: &tauri::AppHandle,
    base: &Url,
    account: &str,
    rotation: &Rotation,
) -> Result<(), String> {
    let path = rotation_path(app, base, account, &rotation.fingerprint(account))?;
    let data = serde_json::to_vec(rotation).map_err(|_| STORE)?;
    if path.exists() {
        if std::fs::read(&path).map_err(|_| STORE)? != data {
            return Err(STORE.into());
        }
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().ok_or(STORE)?).map_err(|_| STORE)?;
    // Only encrypted/public material; raw roots remain in the credential store.
    write_recovery(&path, std::str::from_utf8(&data).map_err(|_| STORE)?)
}
fn read_rotation(
    app: &tauri::AppHandle,
    base: &Url,
    account: &str,
    hash: &str,
) -> Result<Rotation, String> {
    let path = rotation_path(app, base, account, hash)?;
    if std::fs::metadata(&path).map_err(|_| STORE)?.len() > 196608 {
        return Err(STORE.into());
    }
    let value: Rotation =
        serde_json::from_slice(&std::fs::read(&path).map_err(|_| STORE)?).map_err(|_| STORE)?;
    value.validate()?;
    if value.fingerprint(account) != hash {
        return Err(CHANGED.into());
    }
    Ok(value)
}
#[tauri::command]
pub async fn cloud_prepare_key_rotation(
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
) -> Result<SetupKey, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let old = sync_key(&client, &base, &credentials).await?;
    if !old.rotation_supported {
        return Err(
            "Key rotation is unavailable on this server. Update the service and try again.".into(),
        );
    }
    let mut pending = state.pending.lock().await;
    let generation = auth.generation.load(Ordering::SeqCst);
    if let Some(p) = pending.as_ref() {
        if p.origin == base.as_str()
            && p.account == credentials.account.id
            && p.generation == generation
            && p.rotation.is_some()
            && p.created.elapsed() < Duration::from_secs(600)
        {
            return Ok(SetupKey {
                recovery_key: crypto::recovery_text(&p.recovery).to_string(),
            });
        }
    }
    let key = Zeroizing::new(crypto::random::<32>()?);
    let recovery = Zeroizing::new(crypto::random::<32>()?);
    let envelope = crypto::wrap(
        &credentials.account.id,
        &old.envelope.dataset_id,
        &key,
        &recovery,
    )?;
    let rotation = Rotation::create(
        &credentials.account.id,
        &old.ring,
        &key,
        envelope.clone(),
        &old.checkpoint,
    )?;
    let recovery_key = crypto::recovery_text(&recovery).to_string();
    *pending = Some(Pending {
        origin: base.to_string(),
        account: credentials.account.id,
        generation,
        created: Instant::now(),
        key,
        recovery,
        envelope,
        rotation: Some(rotation),
    });
    let pending = Arc::clone(&state.pending);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(600)).await;
        let mut p = pending.lock().await;
        if p.as_ref()
            .is_some_and(|p| p.created.elapsed() >= Duration::from_secs(600))
        {
            *p = None;
        }
    });
    Ok(SetupKey { recovery_key })
}
#[tauri::command]
pub async fn cloud_confirm_key_rotation(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
    recovery_key: String,
    saved: bool,
) -> Result<EncryptionStatus, String> {
    let text = Zeroizing::new(recovery_key);
    if !saved {
        return Err("Save your recovery key and confirm that support cannot recover it.".into());
    }
    let recovery = crypto::parse_recovery(&text)?;
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let mut local = load(&base, &credentials.account.id).await?.ok_or(STORE)?;
    let mut pending = state.pending.lock().await;
    let rotation = if let Some(p) = &local.pending_rotation {
        read_rotation(&app, &base, &credentials.account.id, &p.hash)?
    } else {
        let p = pending.as_ref().ok_or(EXPIRED)?;
        if p.origin != base.as_str()
            || p.account != credentials.account.id
            || p.generation != auth.generation.load(Ordering::SeqCst)
            || p.created.elapsed() >= Duration::from_secs(600)
        {
            return Err(EXPIRED.into());
        }
        p.rotation.clone().ok_or(EXPIRED)?
    };
    let key = crypto::unwrap(
        &credentials.account.id,
        &local.envelope.dataset_id,
        &rotation.key,
        &recovery,
    )?;
    let ring = Ring::open(
        &credentials.account.id,
        key.clone(),
        &rotation.key,
        Some(&rotation),
    )?;
    if local.confirmed
        && local.envelope == rotation.key
        && local.rotation_hash.as_deref() == Some(&rotation.fingerprint(&credentials.account.id))
    {
        let current = remote(&client, &base, &credentials, None).await?;
        check_pin(Some(&local), &current)?;
        check_rotation_pin(&local, &current, &credentials.account.id)?;
        *pending = None;
        return status(Some(&local), &current);
    }
    if !ring.matches_pin(
        &credentials.account.id,
        &local.envelope,
        local.rotation_hash.as_deref(),
    ) || local.envelope != rotation.previous_key
    {
        return Err(CHANGED.into());
    }
    persist_rotation(&app, &base, &credentials.account.id, &rotation)?;
    local.pending_rotation = Some(PendingRotation {
        envelope: rotation.key.clone(),
        key: Some(URL_SAFE_NO_PAD.encode(&*key)),
        hash: rotation.fingerprint(&credentials.account.id),
    });
    // Keep the old usable key and pin until the exact new package is acknowledged.
    save(&base, &credentials.account.id, &local).await?;
    let response = client
        .post(endpoint(&base))
        .bearer_auth(&credentials.access_token)
        .json(&rotation)
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    if response.status().as_u16() == 409 {
        return Err("Cloud encryption or history changed. Cancel this pending replacement, sync, and prepare a new recovery key.".into());
    }
    let accepted: Remote = key_json(checked(response, &base).await?).await?;
    if accepted.dataset_id != local.envelope.dataset_id
        || accepted.key.as_ref() != Some(&rotation.key)
        || accepted.rotation.as_ref() != Some(&rotation)
    {
        return Err(CHANGED.into());
    }
    let mut local = Some(local);
    acknowledge(&base, &credentials.account.id, &mut local, &accepted).await?;
    *pending = None;
    status(local.as_ref(), &accepted)
}
#[tauri::command]
pub async fn cloud_cancel_key_rotation(
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
) -> Result<EncryptionStatus, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let remote = remote(&client, &base, &credentials, None).await?;
    let mut local = load(&base, &credentials.account.id).await?;
    acknowledge(&base, &credentials.account.id, &mut local, &remote).await?;
    check_pin(local.as_ref(), &remote)?;
    if let Some(local) = local.as_mut() {
        check_rotation_pin(local, &remote, &credentials.account.id)?;
        local.pending_rotation = None;
        save(&base, &credentials.account.id, local).await?;
    }
    *state.pending.lock().await = None;
    status(local.as_ref(), &remote)
}
#[tauri::command]
pub async fn cloud_history_verification_code(
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<String, String> {
    let _guard = auth.operation.lock().await;
    let (client, base, credentials) = context().await?;
    let mut key = sync_key(&client, &base, &credentials).await?;
    // Explicit read-only verification; never enables sync or uploads local data.
    let head = crate::cloud_sync::verified_head(&client, &base, &credentials, &key).await?;
    save_sync_checkpoint(&base, &credentials.account.id, &key.envelope, &head).await?;
    key.checkpoint = head;
    epochs::history_code(
        base.as_str(),
        &credentials.account.id,
        &key.ring,
        key.rotation.as_ref(),
        &key.checkpoint,
        key.unverified_history,
    )
}
#[tauri::command]
pub async fn cloud_cancel_encryption(
    state: tauri::State<'_, EncryptionState>,
) -> Result<(), String> {
    *state.pending.lock().await = None;
    Ok(())
}

fn write_recovery(path: &Path, content: &str) -> Result<(), String> {
    let mut file = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or("Choose a recovery-key file location.")?,
    )
    .map_err(|_| "Could not save the recovery-key file.")?;
    file.write_all(content.as_bytes())
        .map_err(|_| "Could not save the recovery-key file.")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "Could not save the recovery-key file.")?;
    file.persist_noclobber(path)
        .map_err(|_| "Could not save the recovery-key file. Choose a new filename.")?;
    Ok(())
}
#[tauri::command]
pub async fn cloud_save_recovery_key(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    state: tauri::State<'_, EncryptionState>,
) -> Result<bool, String> {
    let _guard = auth.operation.lock().await;
    let base = cloud_auth::origin()?;
    let credentials = cloud_auth::load(&base)
        .await?
        .ok_or(cloud_auth::AUTH_ERROR)?;
    let pending = state.pending.lock().await;
    let pending = pending.as_ref().ok_or(EXPIRED)?;
    if pending.origin != base.as_str()
        || pending.account != credentials.account.id
        || pending.generation != auth.generation.load(Ordering::SeqCst)
        || pending.created.elapsed() >= Duration::from_secs(600)
    {
        return Err(EXPIRED.into());
    }
    let text = Zeroizing::new(format!("Heyday Money recovery key\n\n{}\n\nKeep this key private and store it separately from your computer.\nSupport cannot recover it. Anyone with this key and the encrypted key package can unlock your cloud data.\nDataset: {}\n", crypto::recovery_text(&pending.recovery).as_str(), pending.envelope.dataset_id));
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name("heyday-recovery-key.txt")
            .add_filter("Recovery key", &["txt"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = path
            .into_path()
            .map_err(|_| "Choose a local recovery-key file.")?;
        write_recovery(&path, &text)?;
        Ok(true)
    })
    .await
    .map_err(|_| "Could not save the recovery-key file.")?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(confirmed: bool) -> LocalKey {
        let key = Zeroizing::new([7; 32]);
        LocalKey {
            version: 1,
            envelope: crypto::wrap("alice", "dataset", &key, &Zeroizing::new([9; 32])).unwrap(),
            key: Some(URL_SAFE_NO_PAD.encode(&*key)),
            confirmed,
            remember: false,
            sync_checkpoint: None,
            rotation_hash: None,
            unverified_history: false,
            pending_rotation: None,
        }
    }
    #[test]
    fn remembering_survives_sign_out_but_forgetting_and_locking_preserve_only_the_pin() {
        let mut local = record(true);
        local.sync_checkpoint = Some(crate::cloud_sync_protocol::Head::default());
        let original = local.envelope.clone();
        let checkpoint = serde_json::to_value(&local.sync_checkpoint).unwrap();
        // Legacy secure-store entries do not silently opt in to retention.
        let mut legacy = serde_json::to_value(&local).unwrap();
        legacy.as_object_mut().unwrap().remove("remember");
        let mut legacy: LocalKey = serde_json::from_value(legacy).unwrap();
        sign_out_key(&mut legacy, false);
        assert!(legacy.key.is_none());
        local.remember = true;
        let before = local.key.clone();
        sign_out_key(&mut local, false);
        assert_eq!(local.key, before);
        let remote = Remote {
            dataset_id: original.dataset_id.clone(),
            key: Some(original.clone()),
            rotation: None,
            key_management_version: None,
        };
        assert!(status(Some(&local), &remote).unwrap().remember_encryption);
        sign_out_key(&mut local, true);
        assert!(local.key.is_none());
        assert!(!local.remember);
        assert!(local.envelope == original);
        assert_eq!(
            serde_json::to_value(&local.sync_checkpoint).unwrap(),
            checkpoint
        );
        assert_eq!(status(Some(&local), &remote).unwrap().state, "locked");
        let mut changed = remote;
        changed.key = Some(record(true).envelope.clone());
        assert!(status(Some(&local), &changed).is_err());
        let mut locked = record(true);
        locked.remember = true;
        clear_key(&mut locked); // explicit lock and revocation use this same path
        assert!(locked.key.is_none());
        assert!(!locked.remember);
    }
    #[test]
    fn unverified_recovery_requires_acknowledgment_and_never_erases_an_existing_pin() {
        use crate::cloud_sync_protocol::Head;
        assert!(recovery_checkpoint(None, None, false).is_err());
        assert_eq!(
            recovery_checkpoint(None, None, true).unwrap(),
            (Head::default(), true)
        );
        let mut local = record(true);
        let head = Head {
            sequence: "8".into(),
            hash: "a".repeat(64),
        };
        local.sync_checkpoint = Some(head.clone());
        assert_eq!(
            recovery_checkpoint(Some(&local), None, true).unwrap(),
            (head.clone(), false)
        );
        local.unverified_history = true;
        assert_eq!(
            recovery_checkpoint(Some(&local), None, false).unwrap(),
            (head.clone(), true)
        );
        assert!(recovery_checkpoint(Some(&local), Some((Head::default(), false)), true).is_err());
        assert!(recovery_checkpoint(
            Some(&local),
            Some((
                Head {
                    sequence: "8".into(),
                    hash: "b".repeat(64)
                },
                false
            )),
            true
        )
        .is_err());
        assert_eq!(
            recovery_checkpoint(Some(&local), Some((head.clone(), false)), false).unwrap(),
            (head.clone(), false)
        );
        clear_key(&mut local);
        assert!(local.unverified_history);
        assert_eq!(local.sync_checkpoint, Some(head));
    }
    #[test]
    fn pending_rotation_keeps_credential_payload_small_and_clears_both_usable_keys_on_forget() {
        let mut local = record(true);
        local.remember = true;
        local.pending_rotation = Some(PendingRotation {
            envelope: record(true).envelope.clone(),
            key: Some(URL_SAFE_NO_PAD.encode([13; 32])),
            hash: "a".repeat(64),
        });
        let serialized = serde_json::to_string(&local).unwrap();
        assert!(
            serialized.len() < 2560,
            "Windows credential value must remain compact"
        );
        assert!(!serialized.contains("history\":{\"salt"));
        sign_out_key(&mut local, false);
        assert!(local.key.is_some());
        assert!(local.pending_rotation.as_ref().unwrap().key.is_some());
        sign_out_key(&mut local, true);
        assert!(local.key.is_none());
        assert!(local.pending_rotation.as_ref().unwrap().key.is_none());
        assert_eq!(
            local.pending_rotation.as_ref().unwrap().hash,
            "a".repeat(64)
        );
    }
    #[test]
    fn setup_state_handles_lost_response_and_blocks_confirmed_key_substitution() {
        let mut local = record(false);
        let mut remote = Remote {
            dataset_id: "dataset".into(),
            key: None,
            rotation: None,
            key_management_version: None,
        };
        assert_eq!(status(None, &remote).unwrap().state, "not_set");
        assert_eq!(status(Some(&local), &remote).unwrap().state, "pending");
        remote.key = Some(local.envelope.clone());
        assert_eq!(status(Some(&local), &remote).unwrap().state, "unlocked");
        assert_eq!(status(None, &remote).unwrap().state, "locked");
        local.confirmed = true;
        local.key = None;
        assert_eq!(status(Some(&local), &remote).unwrap().state, "locked");
        remote.key = Some(record(false).envelope.clone());
        assert!(status(Some(&local), &remote).is_err());
        remote.key = None;
        assert!(status(Some(&local), &remote).is_err());
    }
    #[test]
    fn user_selected_recovery_export_never_overwrites_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recovery.txt");
        write_recovery(&path, "test key").unwrap();
        assert!(write_recovery(&path, "replacement").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
    }

    #[tokio::test]
    async fn native_api_sends_only_wrapped_key_and_rejects_changed_acknowledgement() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../crates/heyday-core/tests/fixtures/e2ee-v1.json")).unwrap();
        let envelope: KeyEnvelope = serde_json::from_value(fixture["envelope"].clone()).unwrap();
        let sent = envelope.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            for step in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut buffer = [0; 1024];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&buffer[..size]);
                    assert!(bytes.len() < 8192);
                    if let Some(offset) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        break offset + 4;
                    }
                };
                let headers = String::from_utf8(bytes[..header_end].to_vec())
                    .unwrap()
                    .to_lowercase();
                assert!(headers.starts_with(if step == 0 {
                    "get /api/v1/encryption/keys "
                } else {
                    "put /api/v1/encryption/keys "
                }));
                assert!(headers.contains("authorization: bearer unit-test-token\r\n"));
                let size = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map(|s| s.parse::<usize>().unwrap())
                    .unwrap_or(0);
                while bytes.len() < header_end + size {
                    let mut buffer = [0; 1024];
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&buffer[..read]);
                }
                if step == 0 {
                    assert_eq!(size, 0);
                } else {
                    let body: serde_json::Value =
                        serde_json::from_slice(&bytes[header_end..header_end + size]).unwrap();
                    assert_eq!(body, serde_json::to_value(&sent).unwrap());
                    assert!(!String::from_utf8(bytes.clone())
                        .unwrap()
                        .contains(fixture["recoveryKey"].as_str().unwrap()));
                    assert!(!String::from_utf8(bytes)
                        .unwrap()
                        .contains(fixture["dataKey"].as_str().unwrap()));
                }
                let mut returned = sent.clone();
                if step == 2 {
                    returned.key_id = "substituted-key".into();
                }
                let body = serde_json::json!({"datasetId":sent.dataset_id,"key":if step == 0 { None } else { Some(returned) }}).to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let credentials: Credentials = serde_json::from_value(serde_json::json!({"access_token":"unit-test-token","refresh_token":"unit-test-refresh","expires_at":0,"account":{"id":"test-owner","email":"alice@example.test"}})).unwrap();
        let client = cloud_auth::client().unwrap();
        assert!(remote(&client, &base, &credentials, None)
            .await
            .unwrap()
            .key
            .is_none());
        assert!(
            remote(&client, &base, &credentials, Some(&envelope))
                .await
                .unwrap()
                .key
                .as_ref()
                == Some(&envelope)
        );
        assert!(
            matches!(remote(&client, &base, &credentials, Some(&envelope)).await, Err(error) if error == CHANGED)
        );
        server.await.unwrap();
    }
}
