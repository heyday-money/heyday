use crate::cloud_auth::{self, CloudAuthState, Credentials, AUTH_ERROR, NETWORK_ERROR};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tauri::Manager;

const ID_ERROR: &str =
    "Could not read or save this installation's identity. Your local finances are unchanged.";
const INVALID: &str = "Invalid device response. Please update the app or try again.";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDevice {
    id: String,
    name: String,
    platform: String,
    created_at: String,
    last_seen_at: String,
    revoked_at: Option<String>,
    current: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePage {
    devices: Vec<CloudDevice>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    current_device: Option<CloudDevice>,
    saved_name: Option<String>,
    page: DevicePage,
}

#[derive(Deserialize)]
struct MeAccount {
    id: String,
}
#[derive(Deserialize)]
struct Me {
    account: MeAccount,
    device: Option<CloudDevice>,
    entitlement: Option<cloud_auth::CloudEntitlement>,
    billing: Option<cloud_auth::CloudBilling>,
}

pub(super) async fn subscription_status(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
) -> Result<Option<cloud_auth::CloudSubscription>, String> {
    let current = me(client, base, credentials).await?;
    Ok(current
        .entitlement
        .map(|entitlement| cloud_auth::CloudSubscription {
            entitlement,
            billing: current.billing,
        }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokedDevice {
    current: bool,
    credentials_cleared: bool,
}

pub(super) fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn device_name(value: &str) -> Result<&str, String> {
    let name = value.trim();
    // Match the API's JavaScript UTF-16 length bound.
    if name.is_empty()
        || name.encode_utf16().count() > 80
        || value.chars().any(|c| c.is_ascii_control())
    {
        return Err("Enter a device name of 1–80 characters without control characters.".into());
    }
    Ok(name)
}

fn validate_device(device: &CloudDevice) -> Result<(), String> {
    if !valid_uuid(&device.id)
        || device_name(&device.name).is_err()
        || !matches!(device.platform.as_str(), "macos" | "windows" | "linux")
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn identity_path(root: &Path, origin: &str, account: &str) -> PathBuf {
    let scope = serde_json::to_vec(&(origin, account)).expect("string tuple");
    root.join(format!("{:x}.id", Sha256::digest(scope)))
}

fn local_path(app: &tauri::AppHandle, base: &Url, account: &str) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| ID_ERROR)?
        .join("cloud-installations")
        .join(if cfg!(debug_assertions) {
            "development"
        } else {
            "release"
        });
    Ok(identity_path(
        &root,
        &base.origin().ascii_serialization(),
        account,
    ))
}

fn saved_name(path: &Path) -> Result<Option<String>, String> {
    let path = path.with_extension("name");
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.len() <= 320 => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        _ => return Err(ID_ERROR.into()),
    }
    let name = std::fs::read_to_string(path).map_err(|_| ID_ERROR)?;
    device_name(&name).map_err(|_| ID_ERROR)?;
    Ok(Some(name))
}

fn registration_identity(path: &Path, requested_name: &str) -> Result<(String, String), String> {
    let id = installation_id(path)?;
    if let Some(name) = saved_name(path)? {
        return Ok((id, name));
    }
    let name = device_name(requested_name)?.to_owned();
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or(ID_ERROR)?).map_err(|_| ID_ERROR)?;
    file.write_all(name.as_bytes()).map_err(|_| ID_ERROR)?;
    file.as_file().sync_all().map_err(|_| ID_ERROR)?;
    match file.persist_noclobber(path.with_extension("name")) {
        Ok(_) => Ok((id, name)),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok((id, saved_name(path)?.ok_or(ID_ERROR)?))
        }
        Err(_) => Err(ID_ERROR.into()),
    }
}

// Read only: reconnect must never invent a new identity or bypass revocation.
fn remembered_identity(path: &Path) -> Result<Option<(String, String)>, String> {
    if path
        .with_extension("forgotten")
        .try_exists()
        .map_err(|_| ID_ERROR)?
    {
        return Ok(None);
    }
    let name = saved_name(path)?;
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && name.is_none() => {
            return Ok(None)
        }
        Err(_) => return Err(ID_ERROR.into()),
    };
    if metadata.len() != 36 {
        return Err(ID_ERROR.into());
    }
    let id = std::fs::read_to_string(path).map_err(|_| ID_ERROR)?;
    if !valid_uuid(&id) {
        return Err(ID_ERROR.into());
    }
    // An interrupted first registration may have saved its ID but not its name.
    Ok(name.map(|name| (id, name)))
}

pub(super) async fn forget_registration(
    app: &tauri::AppHandle,
    base: &Url,
    account: &str,
) -> Result<(), String> {
    let path = local_path(app, base, account)?;
    tauri::async_runtime::spawn_blocking(move || {
        if path.try_exists().map_err(|_| ID_ERROR)? {
            std::fs::write(path.with_extension("forgotten"), b"1").map_err(|_| ID_ERROR)?;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|_| ID_ERROR)?
}

pub(super) async fn restore_registration(
    app: &tauri::AppHandle,
    client: &Client,
    base: &Url,
    credentials: &Credentials,
) -> Result<bool, String> {
    let path = local_path(app, base, &credentials.account.id)?;
    restore_at_path(&path, client, base, credentials).await
}

async fn restore_at_path(
    path: &Path,
    client: &Client,
    base: &Url,
    credentials: &Credentials,
) -> Result<bool, String> {
    let path = path.to_owned();
    let identity = tauri::async_runtime::spawn_blocking(move || remembered_identity(&path))
        .await
        .map_err(|_| ID_ERROR)??;
    let Some((id, name)) = identity else {
        return Ok(false);
    };
    if me(client, base, credentials).await?.device.is_some() {
        return Ok(true);
    }
    register(client, base, credentials, &id, &name).await?;
    Ok(true)
}

fn installation_id(path: &Path) -> Result<String, String> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() != 36 {
            return Err(ID_ERROR.into());
        }
    }
    match std::fs::read_to_string(path) {
        Ok(id) if valid_uuid(&id) => return Ok(id),
        Ok(_) => return Err(ID_ERROR.into()), // Never replace corrupt/revoked identity silently.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if path.with_extension("name").exists() {
                return Err(ID_ERROR.into());
            }
        }
        Err(_) => return Err(ID_ERROR.into()),
    }
    let directory = path.parent().ok_or(ID_ERROR)?;
    std::fs::create_dir_all(directory).map_err(|_| ID_ERROR)?;
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ID_ERROR)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let id = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| ID_ERROR)?;
    file.write_all(id.as_bytes()).map_err(|_| ID_ERROR)?;
    file.as_file().sync_all().map_err(|_| ID_ERROR)?;
    match file.persist_noclobber(path) {
        Ok(_) => Ok(id),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = std::fs::read_to_string(path).map_err(|_| ID_ERROR)?;
            if valid_uuid(&existing) {
                Ok(existing)
            } else {
                Err(ID_ERROR.into())
            }
        }
        Err(_) => Err(ID_ERROR.into()),
    }
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn api(base: &Url, path: &str) -> String {
    format!("{}api/v1/{path}", base.as_str())
}

fn response_error(status: reqwest::StatusCode, code: Option<&str>) -> &'static str {
    match (status.as_u16(), code) {
        (401, _) => AUTH_ERROR,
        (403, Some("DEVICE_REVOKED")) => "This installation was revoked. It cannot register again automatically. Your local finances remain available.",
        (409, _) => "This installation is already registered with different details. Refresh devices to continue.",
        (429, _) => "Too many device requests. Try again in a minute.",
        (404, Some("NOT_FOUND")) => "Device no longer exists. Refresh devices to continue.",
        (404, _) => "Device management is unavailable on this server. Update the service and try again.",
        _ => "Could not manage devices. Check your connection and try again.",
    }
}

async fn checked(response: Response, base: &Url) -> Result<Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        cloud_auth::forget(base).await?;
        return Err(AUTH_ERROR.into());
    }
    let body = cloud_auth::json::<serde_json::Value>(response).await.ok();
    if status == reqwest::StatusCode::FORBIDDEN
        && body
            .as_ref()
            .and_then(|b| b.get("code"))
            .and_then(|c| c.as_str())
            == Some("DEVICE_REVOKED")
    {
        cloud_auth::forget(base).await?;
        return Err(AUTH_ERROR.into());
    }
    Err(response_error(
        status,
        body.as_ref()
            .and_then(|b| b.get("code"))
            .and_then(|v| v.as_str()),
    )
    .into())
}

pub(super) async fn credentials(client: &Client, base: &Url) -> Result<Credentials, String> {
    let mut credentials = cloud_auth::load(base).await?.ok_or(AUTH_ERROR)?;
    if !cloud_auth::refresh_credentials(client, base, &mut credentials).await? {
        return Err(AUTH_ERROR.into());
    }
    Ok(credentials)
}

async fn me(client: &Client, base: &Url, credentials: &Credentials) -> Result<Me, String> {
    let response = client
        .get(api(base, "me"))
        .bearer_auth(&credentials.access_token)
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let me: Me = cloud_auth::json(checked(response, base).await?).await?;
    if me.account.id != credentials.account.id {
        cloud_auth::forget(base).await?;
        return Err("Sign-in account changed. Please sign in again.".into());
    }
    if let Some(device) = &me.device {
        validate_device(device)?;
        if !device.current || device.revoked_at.is_some() {
            return Err(INVALID.into());
        }
    }
    Ok(me)
}

async fn page(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    cursor: Option<&str>,
) -> Result<DevicePage, String> {
    if cursor.is_some_and(|c| !valid_uuid(c)) {
        return Err(INVALID.into());
    }
    let mut request = client
        .get(api(base, "devices"))
        .bearer_auth(&credentials.access_token)
        .query(&[("limit", "50")]);
    if let Some(cursor) = cursor {
        request = request.query(&[("cursor", cursor)]);
    }
    let response = request.send().await.map_err(|_| NETWORK_ERROR)?;
    let page: DevicePage = cloud_auth::json(checked(response, base).await?).await?;
    if page.devices.len() > 50
        || page
            .next_cursor
            .as_deref()
            .is_some_and(|c| !valid_uuid(c) || Some(c) <= cursor)
    {
        return Err(INVALID.into());
    }
    let mut previous = cursor;
    for device in &page.devices {
        validate_device(device)?;
        if previous.is_some_and(|id| id >= device.id.as_str()) {
            return Err(INVALID.into());
        }
        previous = Some(&device.id);
    }
    if page.next_cursor.is_some() && page.next_cursor.as_deref() != previous {
        return Err(INVALID.into());
    }
    Ok(page)
}

#[tauri::command]
pub async fn cloud_devices(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    cursor: Option<String>,
) -> Result<DeviceView, String> {
    let _guard = auth.operation.lock().await;
    let base = cloud_auth::origin()?;
    let client = cloud_auth::client()?;
    let credentials = credentials(&client, &base).await?;
    let current = me(&client, &base, &credentials).await?;
    let path = local_path(&app, &base, &credentials.account.id)?;
    let saved_name = tauri::async_runtime::spawn_blocking(move || saved_name(&path))
        .await
        .map_err(|_| ID_ERROR)??;
    Ok(DeviceView {
        current_device: current.device,
        saved_name,
        page: page(&client, &base, &credentials, cursor.as_deref()).await?,
    })
}

#[tauri::command]
pub async fn cloud_register_device(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    name: String,
) -> Result<CloudDevice, String> {
    let name = device_name(&name)?.to_owned();
    let _guard = auth.operation.lock().await;
    let base = cloud_auth::origin()?;
    let client = cloud_auth::client()?;
    let credentials = credentials(&client, &base).await?;
    let path = local_path(&app, &base, &credentials.account.id)?;
    // A retry after a lost response also clears a previously saved forget marker.
    let device = if let Some(device) = me(&client, &base, &credentials).await?.device {
        device
    } else {
        let (id, name) = tauri::async_runtime::spawn_blocking({
            let path = path.clone();
            move || registration_identity(&path, &name)
        })
        .await
        .map_err(|_| ID_ERROR)??;
        register(&client, &base, &credentials, &id, &name).await?
    };
    tauri::async_runtime::spawn_blocking(move || {
        match std::fs::remove_file(path.with_extension("forgotten")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ID_ERROR.to_owned()),
        }
    })
    .await
    .map_err(|_| ID_ERROR)??;
    Ok(device)
}

async fn register(
    client: &Client,
    base: &Url,
    credentials: &Credentials,
    id: &str,
    name: &str,
) -> Result<CloudDevice, String> {
    let response = client
        .post(api(&base, "devices"))
        .bearer_auth(&credentials.access_token)
        .json(&serde_json::json!({ "installationId": id, "name": name, "platform": platform() }))
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let device: CloudDevice = cloud_auth::json(checked(response, &base).await?).await?;
    validate_device(&device)?;
    if !device.current || device.revoked_at.is_some() {
        return Err(INVALID.into());
    }
    Ok(device)
}

#[tauri::command]
pub async fn cloud_revoke_device(
    auth: tauri::State<'_, CloudAuthState>,
    device_id: String,
) -> Result<RevokedDevice, String> {
    if !valid_uuid(&device_id) {
        return Err(INVALID.into());
    }
    let _guard = auth.operation.lock().await;
    let base = cloud_auth::origin()?;
    let client = cloud_auth::client()?;
    let credentials = credentials(&client, &base).await?;
    // Determine self-revocation from the server, never from a UI-supplied flag.
    let current = me(&client, &base, &credentials)
        .await?
        .device
        .is_some_and(|d| d.id == device_id);
    let response = client
        .delete(api(&base, &format!("devices/{device_id}")))
        .bearer_auth(&credentials.access_token)
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let result: serde_json::Value = cloud_auth::json(checked(response, &base).await?).await?;
    if result.get("revoked").and_then(|v| v.as_bool()) != Some(true) {
        return Err(INVALID.into());
    }
    let credentials_cleared = !current || cloud_auth::forget(&base).await.is_ok();
    Ok(RevokedDevice {
        current,
        credentials_cleared,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscription_status_reads_scoped_bearer_metadata_before_device_registration() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            for legacy in [false, true] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 1024];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&buffer[..size]);
                    if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(bytes).unwrap().to_lowercase();
                assert!(request.starts_with("get /api/v1/me "));
                assert!(request.contains("authorization: bearer unit-test-token"));
                assert!(!request.contains("cookie:"));
                let mut body = serde_json::json!({"account":{"id":"alice"},"device":null});
                if !legacy {
                    body["entitlement"] = serde_json::json!({"canSync":true,"reason":"trial","accessUntil":"2027-01-05T00:00:00Z"});
                    body["billing"] = serde_json::json!({"enabled":true,"subscriptions":[{"status":"trialing","periodEnd":"2027-01-05T00:00:00Z","cancelAt":null,"customerId":"must-not-cross-ipc"}]});
                }
                let body = body.to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        let credentials: Credentials = serde_json::from_value(serde_json::json!({"access_token":"unit-test-token", "refresh_token":"unit-test-refresh", "expires_at":0,"account":{"id":"alice","email":"alice@example.test"}})).unwrap();
        let client = cloud_auth::client().unwrap();
        let result = subscription_status(&client, &base, &credentials)
            .await
            .unwrap()
            .unwrap();
        let wire = serde_json::to_value(result).unwrap();
        assert_eq!(wire["entitlement"]["canSync"], true);
        assert_eq!(wire["entitlement"]["reason"], "trial");
        assert_eq!(
            wire["billing"]["subscriptions"][0]["periodEnd"],
            "2027-01-05T00:00:00Z"
        );
        assert!(!wire.to_string().contains("must-not-cross-ipc"));
        assert!(subscription_status(&client, &base, &credentials)
            .await
            .unwrap()
            .is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn returning_login_reconnects_the_same_installation_without_registering_new_accounts() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let directory = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let path = identity_path(directory.path(), base.as_str(), "alice");
        let credentials: Credentials = serde_json::from_value(serde_json::json!({"access_token":"test-access", "refresh_token":"test-refresh", "expires_at":0,"account":{"id":"alice","email":"alice@example.test"}})).unwrap();
        let client = cloud_auth::client().unwrap();
        // First login is read-only until the user explicitly registers.
        assert!(!restore_at_path(&path, &client, &base, &credentials)
            .await
            .unwrap());
        assert!(!path.exists());
        let (id, name) = registration_identity(&path, "Personal Mac").unwrap();
        let original_id = id.clone();
        let server = tokio::spawn(async move {
            let device = serde_json::json!({"id":"22222222-2222-4222-8222-222222222222", "name":name, "platform":platform(), "createdAt":"2026-11-05T00:00:00Z", "lastSeenAt":"2026-11-05T00:00:00Z", "revokedAt":null,"current":true});
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
                assert!(headers.contains("authorization: bearer test-access\r\n"));
                let size = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map(|s| s.parse::<usize>().unwrap())
                    .unwrap_or(0);
                while bytes.len() < header_end + size {
                    let mut buffer = [0; 1024];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&buffer[..size]);
                }
                let body = if step == 1 {
                    assert!(headers.starts_with("post /api/v1/devices "));
                    let body: serde_json::Value = serde_json::from_slice(&bytes[header_end..header_end+size]).unwrap();
                    assert_eq!(body, serde_json::json!({"installationId":id,"name":name,"platform":platform()}));
                    device.clone()
                } else {
                    assert!(headers.starts_with("get /api/v1/me "));
                    serde_json::json!({"account":{"id":"alice"},"device":if step == 0 { serde_json::Value::Null } else { device.clone() }})
                }.to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        assert!(restore_at_path(&path, &client, &base, &credentials)
            .await
            .unwrap());
        assert!(restore_at_path(&path, &client, &base, &credentials)
            .await
            .unwrap());
        server.await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original_id);
        let other = identity_path(directory.path(), base.as_str(), "bob");
        assert!(!restore_at_path(&other, &client, &base, &credentials)
            .await
            .unwrap());
        std::fs::write(path.with_extension("forgotten"), b"1").unwrap();
        assert!(!restore_at_path(&path, &client, &base, &credentials)
            .await
            .unwrap());
        // Explicit registration reuses the same identity, including after forgetting.
        assert_eq!(
            registration_identity(&path, "Changed name").unwrap().0,
            original_id
        );
        std::fs::remove_file(path.with_extension("forgotten")).unwrap();
        std::fs::write(&path, "corrupt").unwrap();
        assert!(restore_at_path(&path, &client, &base, &credentials)
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "corrupt");
    }

    #[test]
    fn installation_identity_is_stable_scoped_and_not_silently_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = identity_path(directory.path(), "https://heyday.money", "alice");
        let id = installation_id(&path).unwrap();
        assert!(valid_uuid(&id));
        assert_eq!(&id[14..15], "4");
        assert_eq!(installation_id(&path).unwrap(), id);
        let other = identity_path(directory.path(), "https://heyday.money", "bob");
        assert_ne!(installation_id(&other).unwrap(), id);
        assert_ne!(
            path,
            identity_path(directory.path(), "http://localhost:3000", "alice")
        );
        std::fs::write(&path, "corrupt").unwrap();
        assert_eq!(installation_id(&path).unwrap_err(), ID_ERROR);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "corrupt");
    }

    #[test]
    fn concurrent_identity_creation_retains_one_id() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("installation.id");
        let ids: Vec<_> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| installation_id(&path).unwrap()))
                .collect();
            jobs.into_iter().map(|job| job.join().unwrap()).collect()
        });
        assert!(ids.iter().all(|id| id == &ids[0]));
    }

    #[test]
    fn registration_reuses_its_original_name_after_a_retry_or_new_login() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("installation.id");
        let first = registration_identity(&path, "Personal Mac").unwrap();
        assert_eq!(registration_identity(&path, "My computer").unwrap(), first);
        assert_eq!(saved_name(&path).unwrap().as_deref(), Some("Personal Mac"));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            registration_identity(&path, "New identity").unwrap_err(),
            ID_ERROR
        );
    }

    #[tokio::test]
    async fn native_device_requests_use_bearer_auth_and_bounded_keyset_pages() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            for invalid in [false, true] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 1024];
                    let size = stream.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&buffer[..size]);
                    if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                assert!(request.starts_with(
                    "GET /api/v1/devices?limit=50&cursor=11111111-1111-4111-8111-111111111111 "
                ));
                assert!(request
                    .to_lowercase()
                    .contains("authorization: bearer unit-test-token"));
                let body = serde_json::json!({"devices": [{"id":"22222222-2222-4222-8222-222222222222", "name":"Other computer", "platform":"windows", "createdAt":"2026-11-04T00:00:00.000Z", "lastSeenAt":"2026-11-04T00:00:00.000Z", "revokedAt":null,"current":false}],
                    "nextCursor": if invalid { Some("11111111-1111-4111-8111-111111111111") } else { None }}).to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let credentials: Credentials = serde_json::from_value(serde_json::json!({"access_token":"unit-test-token", "refresh_token":"unit-test-refresh", "expires_at":0,"account":{"id":"alice","email":"alice@example.test"}})).unwrap();
        let client = cloud_auth::client().unwrap();
        let cursor = Some("11111111-1111-4111-8111-111111111111");
        assert_eq!(
            page(&client, &base, &credentials, cursor)
                .await
                .unwrap()
                .devices
                .len(),
            1
        );
        assert!(
            matches!(page(&client, &base, &credentials, cursor).await, Err(error) if error == INVALID)
        );
        server.await.unwrap();
    }

    #[test]
    fn device_input_and_errors_are_bounded() {
        assert_eq!(device_name(" My Mac ").unwrap(), "My Mac");
        for name in [
            "".to_owned(),
            "\tMac".to_owned(),
            "x".repeat(81),
            "😀".repeat(41),
        ] {
            assert!(device_name(&name).is_err());
        }
        assert!(!valid_uuid("../../account"));
        assert_eq!(
            response_error(reqwest::StatusCode::UNAUTHORIZED, None),
            AUTH_ERROR
        );
        assert_ne!(
            response_error(reqwest::StatusCode::FORBIDDEN, Some("DEVICE_REVOKED")),
            NETWORK_ERROR
        );
    }
}
