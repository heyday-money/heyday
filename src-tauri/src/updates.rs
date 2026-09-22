use semver::Version;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{path::Path, time::Duration};
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

const REPOSITORY: &str = "https://api.github.com/repos/heyday-money/heyday/releases?per_page=100";
const PUBLIC_KEY: Option<&str> = option_env!("HEYDAY_UPDATER_PUBLIC_KEY");

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<Asset>,
}
#[derive(Serialize)]
pub struct UpdateInfo {
    current_version: String,
    version: Option<String>,
    notes: Option<String>,
    can_install: bool,
    message: String,
}

fn newest(releases: Vec<Release>, current: &Version) -> Option<(Version, Release)> {
    releases
        .into_iter()
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
            if release.draft
                || version <= *current
                || (current.pre.is_empty() && (release.prerelease || !version.pre.is_empty()))
            {
                return None;
            }
            Some((version, release))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
}

async fn available(app: &tauri::AppHandle) -> Result<Option<(Version, Release)>, String> {
    let current = &app.package_info().version;
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Heyday-Money-updater")
        .build()
        .map_err(|e| e.to_string())?
        .get(REPOSITORY)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| {
            "Could not reach GitHub. Check your internet connection and try again.".to_string()
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub update check failed (HTTP {}). Try again later.",
            response.status().as_u16()
        ));
    }
    let releases: Vec<Release> = response
        .json()
        .await
        .map_err(|_| "GitHub returned an invalid release list.".to_string())?;
    Ok(newest(releases, current))
}

fn manifest(release: &Release) -> Option<&str> {
    release
        .assets
        .iter()
        .find(|asset| {
            asset.name == "latest.json"
                && asset
                    .browser_download_url
                    .starts_with("https://github.com/heyday-money/heyday/releases/download/")
        })
        .map(|asset| asset.browser_download_url.as_str())
}

#[tauri::command]
pub async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let mut info = UpdateInfo {
        current_version: app.package_info().version.to_string(),
        version: None,
        notes: None,
        can_install: false,
        message: "You’re using the latest available version for your release channel.".into(),
    };
    if let Some((version, release)) = available(&app).await? {
        info.version = Some(version.to_string());
        info.can_install = !cfg!(debug_assertions)
            && cfg!(target_os = "macos")
            && PUBLIC_KEY.is_some_and(|key| !key.trim().is_empty())
            && manifest(&release).is_some();
        info.message = if cfg!(debug_assertions) { "Update available. Installation is disabled in development builds." }
        else if info.can_install { "Update available. Save your work before installing." }
        else { "Update available on GitHub Releases. This build or release does not have signed in-app updates configured; install its DMG manually." }.into();
        info.notes = release.body;
    }
    Ok(info)
}

// VACUUM INTO creates a consistent snapshot even when SQLite uses WAL.
pub async fn backup_database(pool: &SqlitePool, directory: &Path) -> Result<(), String> {
    let directory = directory.join("backups");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let file = directory.join(format!("before-update-{stamp}.db"));
    sqlx::query("VACUUM INTO ?")
        .bind(file.to_string_lossy().as_ref())
        .execute(pool)
        .await
        .map_err(|e| format!("Could not back up the database; update stopped: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn install_app_update(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    version: String,
) -> Result<(), String> {
    if cfg!(debug_assertions) || !cfg!(target_os = "macos") {
        return Err("Installation is available only in installed macOS release builds.".into());
    }
    let key = PUBLIC_KEY
        .filter(|key| !key.trim().is_empty())
        .ok_or("Updater signing key is not configured in this build.")?;
    let (latest, release) = available(&app)
        .await?
        .ok_or("No newer release is available.")?;
    if latest.to_string() != version {
        return Err("The available release changed. Check for updates again.".into());
    }
    let endpoint = manifest(&release).ok_or("The release has no signed updater manifest.")?;
    let update = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint
            .parse()
            .map_err(|_| "Invalid updater URL.")?])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No compatible update is available.")?;
    if update.version != version {
        return Err("The updater manifest version does not match the release.".into());
    }
    // Download verifies the signature before any installation is attempted.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    let directory = app.path().app_data_dir().map_err(|e| e.to_string())?;
    backup_database(pool.inner(), &directory).await?;
    update
        .install(bytes)
        .map_err(|e| format!("Could not install update. Your database backup is retained: {e}"))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, draft: bool, prerelease: bool) -> Release {
        Release {
            tag_name: tag.into(),
            draft,
            prerelease,
            body: None,
            assets: vec![],
        }
    }
    #[test]
    fn channels_ignore_drafts_old_versions_and_invalid_tags() {
        let candidates = || {
            vec![
                release("v1.0.1", false, false),
                release("v1.1.0-alpha.1", false, true),
                release("v9.0.0", true, false),
                release("invalid", false, false),
            ]
        };
        assert_eq!(
            newest(candidates(), &Version::parse("1.0.0").unwrap())
                .unwrap()
                .0
                .to_string(),
            "1.0.1"
        );
        assert_eq!(
            newest(candidates(), &Version::parse("1.0.0-alpha.0").unwrap())
                .unwrap()
                .0
                .to_string(),
            "1.1.0-alpha.1"
        );
        assert!(newest(candidates(), &Version::parse("2.0.0").unwrap()).is_none());
    }
    #[tokio::test]
    async fn backup_preserves_records_and_reopens() {
        let directory = std::env::temp_dir().join(format!(
            "heyday-backup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(directory.join("live.db"))
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE example (amount INTEGER); INSERT INTO example VALUES (123456)")
            .execute(&pool)
            .await
            .unwrap();
        backup_database(&pool, &directory).await.unwrap();
        let path = std::fs::read_dir(directory.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let backup = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .read_only(true),
        )
        .await
        .unwrap();
        let amount: i64 = sqlx::query_scalar("SELECT amount FROM example")
            .fetch_one(&backup)
            .await
            .unwrap();
        assert_eq!(amount, 123456);
        backup.close().await;
        pool.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}
