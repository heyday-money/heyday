use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{SqliteConnection, SqlitePool};
use std::io::Cursor;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LogoChange {
    Custom { data: String },
    Default,
    None,
}
pub const MAX_BYTES: usize = 102_400;

// The WebView normalizes selected PNG/JPEG/WebP images. Validate and re-encode
// that small PNG here so persisted bytes never rely on browser validation.
pub fn normalize(data: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let error = || "Choose a valid logo image, resized to at most 128 × 128 pixels.".to_string();
    let encoded = data
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(error)?;
    if encoded.len() > MAX_BYTES.div_ceil(3) * 4 {
        return Err("Logo image is too large.".into());
    }
    let bytes = STANDARD.decode(encoded).map_err(|_| error())?;
    if bytes.len() > MAX_BYTES {
        return Err("Logo image is too large.".into());
    }
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: 1_048_576 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| error())?;
    let (width, height) = (reader.info().width, reader.info().height);
    if !(1..=128).contains(&width)
        || !(1..=128).contains(&height)
        || reader.info().animation_control.is_some()
    {
        return Err(error());
    }
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .filter(|size| *size <= 128 * 128 * 4)
            .ok_or_else(error)?
    ];
    let info = reader.next_frame(&mut buffer).map_err(|_| error())?;
    let mut normalized = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut normalized, width, height);
        encoder.set_color(info.color_type);
        encoder.set_depth(info.bit_depth);
        let mut writer = encoder.write_header().map_err(|_| error())?;
        writer
            .write_image_data(&buffer[..info.buffer_size()])
            .map_err(|_| error())?;
    }
    if normalized.len() > MAX_BYTES {
        return Err("Logo image is too large.".into());
    }
    Ok((normalized, width, height))
}

pub async fn asset(
    conn: &mut SqliteConnection,
    change: &LogoChange,
) -> Result<Option<String>, String> {
    let LogoChange::Custom { data } = change else {
        return Ok(None);
    };
    let (bytes, width, height) = normalize(data)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    sqlx::query("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES(lower(hex(randomblob(16))),?,'image/png',?,?,?) ON CONFLICT(content_hash) DO NOTHING")
        .bind(&hash).bind(width).bind(height).bind(bytes).execute(&mut *conn).await.map_err(|e| e.to_string())?;
    sqlx::query_scalar("SELECT id FROM logo_assets WHERE content_hash=?")
        .bind(hash)
        .fetch_one(&mut *conn)
        .await
        .map(Some)
        .map_err(|e| e.to_string())
}

pub async fn save_account(
    conn: &mut SqliteConnection,
    account: &str,
    change: Option<&LogoChange>,
) -> Result<(), String> {
    let Some(change) = change else {
        return Ok(());
    };
    let cash: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='cash')")
            .bind(account)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    if !cash {
        return Err("Only cash accounts support an account logo.".into());
    }
    if let Some(asset) = asset(conn, change).await? {
        sqlx::query("INSERT INTO account_logos(account_id,logo_asset_id) VALUES(?,?) ON CONFLICT(account_id) DO UPDATE SET logo_asset_id=EXCLUDED.logo_asset_id").bind(account).bind(asset).execute(&mut *conn).await.map_err(|e| e.to_string())?;
    } else {
        sqlx::query("DELETE FROM account_logos WHERE account_id=?")
            .bind(account)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    prune(conn).await
}

pub async fn prune(conn: &mut SqliteConnection) -> Result<(), String> {
    sqlx::query("DELETE FROM logo_assets WHERE NOT EXISTS(SELECT 1 FROM account_logos WHERE logo_asset_id=logo_assets.id) AND NOT EXISTS(SELECT 1 FROM institutions WHERE logo_asset_id=logo_assets.id) AND NOT EXISTS(SELECT 1 FROM payees WHERE logo_asset_id=logo_assets.id) AND NOT EXISTS(SELECT 1 FROM subscriptions WHERE logo_asset_id=logo_assets.id) AND NOT EXISTS(SELECT 1 FROM subscription_providers WHERE logo_asset_id=logo_assets.id)")
        .execute(conn).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_logo_asset(pool: &SqlitePool, id: String) -> Result<String, String> {
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT data FROM logo_assets WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Logo is no longer available.")?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}
