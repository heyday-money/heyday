use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    time::{timeout, Instant},
};

const CLIENT_ID: &str = "heyday-desktop-beta";
pub(super) const NETWORK_ERROR: &str =
    "Could not reach sign-in. Check your connection and try again.";
pub(super) const AUTH_ERROR: &str =
    "Your sign-in has expired or device access was revoked. Please sign in again.";
const STORE_ERROR: &str = "Could not access the secure credential store. Please try again.";

#[derive(Default)]
pub struct CloudAuthState {
    pub(super) operation: Mutex<()>,
    pub(super) generation: AtomicU64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CloudAccount {
    pub(super) id: String,
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    account: Option<CloudAccount>,
    online: bool,
    device_registered: bool,
    device_error: Option<String>,
    subscription: Option<CloudSubscription>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudEntitlement {
    can_sync: bool,
    reason: String,
    access_until: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SubscriptionPeriod {
    status: String,
    period_end: Option<String>,
    cancel_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct CloudBilling {
    enabled: bool,
    subscriptions: Vec<SubscriptionPeriod>,
}

#[derive(Serialize)]
pub(super) struct CloudSubscription {
    pub(super) entitlement: CloudEntitlement,
    pub(super) billing: Option<CloudBilling>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Credentials {
    pub(super) access_token: String,
    refresh_token: String,
    expires_at: u64,
    pub(super) account: CloudAccount,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
}

#[derive(Deserialize)]
struct UserInfo {
    sub: String,
    email: String,
    email_verified: bool,
}

pub(super) fn origin() -> Result<Url, String> {
    validate_origin(
        option_env!("HEYDAY_API_URL").unwrap_or("https://heyday.money"),
        cfg!(debug_assertions),
    )
}

fn validate_origin(value: &str, development: bool) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Invalid sign-in server configuration.")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || !(url.scheme() == "https" || (development && local && url.scheme() == "http"))
    {
        return Err("Invalid sign-in server configuration.".into());
    }
    Ok(url)
}

fn endpoint(base: &Url, path: &str) -> String {
    format!("{}api/auth/{path}", base.as_str())
}

/// Website sessions own billing. No tokens or account information go in the URL.
#[tauri::command]
pub fn cloud_manage_subscription(app: tauri::AppHandle) -> Result<(), String> {
    let url = origin()?
        .join("account")
        .map_err(|_| "Invalid sign-in server configuration.")?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| "Could not open subscription management. Please try again.".into())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn random_value() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Could not start secure sign-in.")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn entry(base: &Url) -> Result<keyring::Entry, String> {
    let service = if cfg!(debug_assertions) {
        "money.heyday.desktop.cloud.development"
    } else {
        "money.heyday.desktop.cloud"
    };
    keyring::Entry::new(service, &base.origin().ascii_serialization())
        .map_err(|_| STORE_ERROR.into())
}

pub(super) async fn load(base: &Url) -> Result<Option<Credentials>, String> {
    let base = base.clone();
    tauri::async_runtime::spawn_blocking(move || match entry(&base)?.get_password() {
        Ok(value) => serde_json::from_str(&value)
            .map(Some)
            .map_err(|_| STORE_ERROR.into()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(STORE_ERROR.into()),
    })
    .await
    .map_err(|_| STORE_ERROR.to_owned())?
}

async fn save(base: &Url, credentials: &Credentials) -> Result<(), String> {
    let base = base.clone();
    let value = serde_json::to_string(credentials).map_err(|_| STORE_ERROR)?;
    tauri::async_runtime::spawn_blocking(move || {
        entry(&base)?
            .set_password(&value)
            .map_err(|_| STORE_ERROR.into())
    })
    .await
    .map_err(|_| STORE_ERROR.to_owned())?
}

pub(super) async fn forget(base: &Url) -> Result<(), String> {
    if let Some(credentials) = load(base).await? {
        crate::cloud_e2ee::lock_saved(base, &credentials.account.id).await?;
    }
    clear_credentials(base).await
}

async fn clear_credentials(base: &Url) -> Result<(), String> {
    let base = base.clone();
    tauri::async_runtime::spawn_blocking(move || match entry(&base)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(STORE_ERROR.into()),
    })
    .await
    .map_err(|_| STORE_ERROR.to_owned())?
}

pub(super) fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| NETWORK_ERROR.into())
}

pub(super) async fn json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| NETWORK_ERROR)? {
        if body.len() + chunk.len() > 65_536 {
            return Err("Invalid sign-in response.".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "Invalid sign-in response.".into())
}

fn validate_tokens(tokens: &TokenResponse) -> Result<(), String> {
    if !tokens.token_type.eq_ignore_ascii_case("bearer")
        || tokens.access_token.is_empty()
        || tokens.refresh_token.is_empty()
        || tokens.expires_in == 0
        || tokens.expires_in > 86400
    {
        return Err("Invalid sign-in response.".into());
    }
    Ok(())
}

async fn userinfo(client: &Client, base: &Url, token: &str) -> Result<CloudAccount, String> {
    let response = client
        .get(endpoint(base, "oauth2/userinfo"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AUTH_ERROR.into());
    }
    if !response.status().is_success() {
        return Err(NETWORK_ERROR.into());
    }
    let user: UserInfo = json(response).await?;
    if user.sub.is_empty() || user.email.is_empty() || !user.email_verified {
        return Err("Invalid sign-in response.".into());
    }
    Ok(CloudAccount {
        id: user.sub,
        email: user.email,
    })
}

fn callback(target: &str, state: &str, issuer: &str) -> Result<Option<String>, String> {
    if !target.starts_with("/callback?") {
        return Ok(None);
    }
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "Invalid sign-in callback.")?;
    let pairs: Vec<_> = url.query_pairs().collect();
    let one = |name: &str| {
        let values: Vec<_> = pairs
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .collect();
        if values.len() == 1 {
            Some(values[0])
        } else {
            None
        }
    };
    if one("state") != Some(state) || one("iss") != Some(issuer) {
        return Ok(None);
    }
    if one("error").is_some() {
        return Err("Sign-in was not completed. Please try again.".into());
    }
    Ok(one("code")
        .filter(|code| !code.is_empty() && code.len() <= 2048)
        .map(str::to_owned))
}

fn callback_response(accepted: bool, language: &str) -> Result<String, String> {
    let nonce = random_value()?;
    let language = if language == "th" { "th" } else { "en" };
    let body = include_str!("../assets/auth-callback.html")
        .replace("__LANG__", language)
        .replace("__RESULT__", if accepted { "accepted" } else { "invalid" })
        .replace("__NONCE__", &nonce);
    let status = if accepted {
        "200 OK"
    } else {
        "400 Bad Request"
    };
    Ok(format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'nonce-{nonce}'; style-src 'nonce-{nonce}'; img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}", body.len()))
}

async fn wait_for_code(
    listener: TcpListener,
    state: &str,
    issuer: &str,
    generation: u64,
    auth: &CloudAuthState,
    language: &str,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        if auth.generation.load(Ordering::SeqCst) != generation {
            return Err("Sign-in cancelled.".into());
        }
        let accepted = timeout(Duration::from_millis(500), listener.accept()).await;
        let Ok(Ok((mut socket, _))) = accepted else {
            continue;
        };
        let mut request = Vec::new();
        let read = timeout(Duration::from_secs(2), async {
            let mut buf = [0u8; 1024];
            while request.len() < 8192 {
                let count = socket.read(&mut buf).await?;
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..count]);
                if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await;
        if !matches!(read, Ok(Ok(()))) {
            continue;
        }
        let request = String::from_utf8_lossy(&request);
        let mut parts = request
            .lines()
            .next()
            .unwrap_or_default()
            .split_whitespace();
        let outcome = if parts.next() == Some("GET") {
            callback(parts.next().unwrap_or_default(), state, issuer)
        } else {
            Ok(None)
        };
        let response = callback_response(matches!(outcome, Ok(Some(_))), language)?;
        let _ = timeout(
            Duration::from_secs(2),
            socket.write_all(response.as_bytes()),
        )
        .await;
        if let Some(code) = outcome? {
            return Ok(code);
        }
    }
    Err("Sign-in timed out. Please try again.".into())
}

#[tauri::command]
pub async fn cloud_sign_in(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    language: Option<String>,
) -> Result<CloudStatus, String> {
    let _guard = auth
        .operation
        .try_lock()
        .map_err(|_| "Another sign-in operation is in progress.")?;
    let generation = auth.generation.load(Ordering::SeqCst);
    let base = origin()?;
    if load(&base).await?.is_some() {
        return Err("Sign out before signing in to another account.".into());
    }
    let verifier = random_value()?;
    let state = random_value()?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Could not start secure sign-in.")?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener
            .local_addr()
            .map_err(|_| "Could not start secure sign-in.")?
            .port()
    );
    let issuer = endpoint(&base, "").trim_end_matches('/').to_owned();
    let mut authorize = Url::parse(&endpoint(&base, "oauth2/authorize"))
        .map_err(|_| "Invalid sign-in server configuration.")?;
    authorize.query_pairs_mut().extend_pairs([
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", "openid email offline_access"),
        ("state", &state),
        ("code_challenge", &challenge(&verifier)),
        ("code_challenge_method", "S256"),
        ("prompt", "login"),
    ]);
    app.opener()
        .open_url(authorize.as_str(), None::<&str>)
        .map_err(|_| "Could not open your browser. Please try again.")?;
    let code = wait_for_code(
        listener,
        &state,
        &issuer,
        generation,
        &auth,
        language.as_deref().unwrap_or("en"),
    )
    .await?;
    let client = client()?;
    let response = client
        .post(endpoint(&base, "oauth2/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    if !response.status().is_success() {
        return Err("Sign-in was not completed. Please try again.".into());
    }
    let tokens: TokenResponse = json(response).await?;
    validate_tokens(&tokens)?;
    let account = userinfo(&client, &base, &tokens.access_token).await?;
    if auth.generation.load(Ordering::SeqCst) != generation {
        return Err("Sign-in cancelled.".into());
    }
    let credentials = Credentials {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: now() + tokens.expires_in,
        account: account.clone(),
    };
    save(&base, &credentials).await?;
    // Cancellation may arrive while the credential store is being accessed.
    if auth.generation.load(Ordering::SeqCst) != generation {
        clear_credentials(&base).await?;
        return Err("Sign-in cancelled.".into());
    }
    let status = connected_status(&app, &client, &base, &credentials).await?;
    if auth.generation.load(Ordering::SeqCst) != generation {
        clear_credentials(&base).await?;
        return Err("Sign-in cancelled.".into());
    }
    Ok(status)
}

async fn connected_status(
    app: &tauri::AppHandle,
    client: &Client,
    base: &Url,
    credentials: &Credentials,
) -> Result<CloudStatus, String> {
    let (device_registered, device_error) =
        match crate::cloud_devices::restore_registration(app, client, base, credentials).await {
            Ok(registered) => (registered, None),
            Err(error)
                if error == AUTH_ERROR
                    || error == "Sign-in account changed. Please sign in again." =>
            {
                return Err(error)
            }
            Err(error) => (false, Some(error)),
        };
    // Read verified access even before first device registration. Billing stays
    // on the website; only account-scoped display metadata crosses IPC.
    let subscription = match crate::cloud_devices::subscription_status(client, base, credentials)
        .await
    {
        Ok(value) => value,
        Err(error)
            if error == AUTH_ERROR || error == "Sign-in account changed. Please sign in again." =>
        {
            return Err(error)
        }
        Err(_) => None,
    };
    Ok(CloudStatus {
        account: Some(credentials.account.clone()),
        online: true,
        device_registered,
        device_error,
        subscription,
    })
}

#[tauri::command]
pub fn cloud_cancel_sign_in(auth: tauri::State<'_, CloudAuthState>) {
    auth.generation.fetch_add(1, Ordering::SeqCst);
}

/// Rotated tokens are persisted before any subsequent device/account request.
pub(super) async fn refresh_credentials(
    client: &Client,
    base: &Url,
    credentials: &mut Credentials,
) -> Result<bool, String> {
    if credentials.expires_at <= now() + 30 {
        let response = client
            .post(endpoint(&base, "oauth2/token"))
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", credentials.refresh_token.as_str()),
            ])
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let tokens: TokenResponse = json(response).await?;
                validate_tokens(&tokens)?;
                credentials.access_token = tokens.access_token;
                credentials.refresh_token = tokens.refresh_token;
                credentials.expires_at = now() + tokens.expires_in;
                // Persist rotated tokens before another request can fail.
                save(&base, &credentials).await?;
            }
            Ok(response) if response.status() == reqwest::StatusCode::BAD_REQUEST => {
                let error: serde_json::Value = json(response).await?;
                if error.get("error").and_then(|v| v.as_str()) == Some("invalid_grant") {
                    forget(&base).await?;
                    return Ok(false);
                }
                return Err(NETWORK_ERROR.into());
            }
            _ => return Err(NETWORK_ERROR.into()),
        }
    }
    Ok(true)
}

#[tauri::command]
pub async fn cloud_account(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
) -> Result<CloudStatus, String> {
    let _guard = auth.operation.lock().await;
    let base = origin()?;
    let Some(mut credentials) = load(&base).await? else {
        return Ok(CloudStatus {
            account: None,
            online: false,
            device_registered: false,
            device_error: None,
            subscription: None,
        });
    };
    let client = client()?;
    match refresh_credentials(&client, &base, &mut credentials).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok(CloudStatus {
                account: None,
                online: false,
                device_registered: false,
                device_error: None,
                subscription: None,
            })
        }
        Err(error) if error == NETWORK_ERROR => {
            return Ok(CloudStatus {
                account: Some(credentials.account),
                online: false,
                device_registered: false,
                device_error: None,
                subscription: None,
            })
        }
        Err(error) => return Err(error),
    }
    match userinfo(&client, &base, &credentials.access_token).await {
        Ok(account) if account.id == credentials.account.id => {
            if account.email != credentials.account.email {
                credentials.account = account.clone();
                save(&base, &credentials).await?;
            }
            connected_status(&app, &client, &base, &credentials).await
        }
        Ok(_) => {
            forget(&base).await?;
            Err("Sign-in account changed. Please sign in again.".into())
        }
        Err(error) if error == AUTH_ERROR => {
            forget(&base).await?;
            Ok(CloudStatus {
                account: None,
                online: false,
                device_registered: false,
                device_error: None,
                subscription: None,
            })
        }
        Err(_) => Ok(CloudStatus {
            account: Some(credentials.account),
            online: false,
            device_registered: false,
            device_error: None,
            subscription: None,
        }),
    }
}

#[tauri::command]
pub async fn cloud_sign_out(
    app: tauri::AppHandle,
    auth: tauri::State<'_, CloudAuthState>,
    encryption: tauri::State<'_, crate::cloud_e2ee::EncryptionState>,
    forget_device: Option<bool>,
) -> Result<bool, String> {
    auth.generation.fetch_add(1, Ordering::SeqCst);
    let _guard = auth.operation.lock().await;
    let base = origin()?;
    let credentials = load(&base).await?;
    if let Some(credentials) = &credentials {
        let forget_device = forget_device.unwrap_or(false);
        if forget_device {
            crate::cloud_devices::forget_registration(&app, &base, &credentials.account.id).await?;
        }
        crate::cloud_e2ee::sign_out_saved(&base, &credentials.account.id, forget_device).await?;
    }
    crate::cloud_e2ee::cloud_cancel_encryption(encryption).await?;
    clear_credentials(&base).await?;
    let Some(credentials) = credentials else {
        return Ok(true);
    };
    let client = client()?;
    let mut revoked = true;
    for (token, hint) in [
        (&credentials.access_token, "access_token"),
        (&credentials.refresh_token, "refresh_token"),
    ] {
        let response = client
            .post(endpoint(&base, "oauth2/revoke"))
            .form(&[
                ("client_id", CLIENT_ID),
                ("token", token.as_str()),
                ("token_type_hint", hint),
            ])
            .send()
            .await;
        revoked &= response.is_ok_and(|r| r.status().is_success());
    }
    Ok(revoked)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636_vector() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_ne!(random_value().unwrap(), random_value().unwrap());
    }
    #[test]
    fn callback_requires_state_issuer_and_unique_code() {
        let issuer = "https://heyday.money/api/auth";
        let good = format!("/callback?state=expected&iss={issuer}&code=accepted");
        assert_eq!(
            callback(&good, "expected", issuer).unwrap(),
            Some("accepted".into())
        );
        assert_eq!(callback(&good, "different", issuer).unwrap(), None);
        assert_eq!(
            callback(&good, "expected", "https://evil.example").unwrap(),
            None
        );
        assert_eq!(
            callback(&(good + "&code=another"), "expected", issuer).unwrap(),
            None
        );
        assert_eq!(
            callback("/callback?code=accepted", "expected", issuer).unwrap(),
            None
        );
    }
    #[test]
    fn callback_html_is_private_nonce_protected_and_language_bounded() {
        let response = callback_response(true, "th").unwrap();
        let (headers, html) = response.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("HTTP/1.1 200 OK"));
        assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
        assert!(headers.contains("Cache-Control: no-store"));
        assert!(headers.contains("Referrer-Policy: no-referrer"));
        assert!(headers.contains("default-src 'none'"));
        assert!(headers.contains(&format!("Content-Length: {}", html.len())));
        assert!(html.contains("lang=\"th\" data-result=\"accepted\""));
        assert!(!html.contains("__NONCE__"));
        let nonce = html
            .split("nonce=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(headers.contains(&format!("script-src 'nonce-{nonce}'")));
        let invalid = callback_response(false, "\"><script>attack</script>").unwrap();
        assert!(invalid.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(invalid.contains("lang=\"en\" data-result=\"invalid\""));
        assert!(!invalid.contains("attack"));
    }
    #[test]
    fn release_origin_requires_https_and_no_credentials() {
        assert!(validate_origin("https://heyday.money", false).is_ok());
        assert!(validate_origin("http://localhost:3000", true).is_ok());
        for url in [
            "http://localhost:3000",
            "http://public.example",
            "https://user:secret@example.com",
            "https://example.com/path",
            "https://example.com?query=1",
        ] {
            assert!(validate_origin(url, false).is_err());
        }
        assert!(validate_origin("http://public.example", true).is_err());
    }

    #[tokio::test]
    async fn loopback_ignores_wrong_state_and_accepts_matching_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let auth = std::sync::Arc::new(CloudAuthState::default());
        let task_auth = auth.clone();
        let waiting = tokio::spawn(async move {
            wait_for_code(
                listener,
                "expected",
                "https://heyday.money/api/auth",
                0,
                &task_auth,
                "en",
            )
            .await
        });
        for state in ["wrong", "expected"] {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(format!("GET /callback?state={state}&iss=https://heyday.money/api/auth&code=accepted HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes()).await.unwrap();
            let mut reply = Vec::new();
            stream.read_to_end(&mut reply).await.unwrap();
            assert!(
                String::from_utf8_lossy(&reply).contains(if state == "wrong" {
                    "400"
                } else {
                    "200"
                })
            );
        }
        assert_eq!(waiting.await.unwrap().unwrap(), "accepted");
    }

    #[tokio::test]
    async fn cancelled_loopback_closes_without_waiting_for_browser() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let auth = CloudAuthState::default();
        auth.generation.store(1, Ordering::SeqCst);
        let result = timeout(
            Duration::from_secs(1),
            wait_for_code(listener, "state", "issuer", 0, &auth, "en"),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err(), "Sign-in cancelled.");
    }
}
