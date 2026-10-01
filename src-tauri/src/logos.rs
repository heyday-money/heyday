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
const MAX_BYTES: usize = 102_400;

// The WebView normalizes selected PNG/JPEG/WebP images. Validate and re-encode
// that small PNG here so persisted bytes never rely on browser validation.
fn normalize(data: &str) -> Result<(Vec<u8>, u32, u32), String> {
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

pub async fn prune(conn: &mut SqliteConnection) -> Result<(), String> {
    sqlx::query("DELETE FROM logo_assets WHERE NOT EXISTS(SELECT 1 FROM institutions WHERE logo_asset_id=logo_assets.id) AND NOT EXISTS(SELECT 1 FROM payees WHERE logo_asset_id=logo_assets.id)")
        .execute(conn).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_logo_asset(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<String, String> {
    let _database_operation = crate::backups::operation()?;
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT data FROM logo_assets WHERE id=?")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Logo is no longer available.")?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::institutions::{save as save_institution, SaveInstitution};
    use crate::transaction_options::{save as save_option, OptionKind, SaveOption};
    fn sample(width: u32, height: u32) -> String {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .add_text_chunk("Comment".into(), "Private original metadata".into())
                .unwrap();
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&vec![120; (width * height * 4) as usize])
                .unwrap();
        }
        format!("data:image/png;base64,{}", STANDARD.encode(bytes))
    }
    fn institution(
        id: Option<&str>,
        name: &str,
        logo_change: Option<LogoChange>,
    ) -> SaveInstitution {
        SaveInstitution {
            id: id.map(str::to_owned),
            name: name.into(),
            is_archived: false,
            logo_change,
        }
    }
    fn payee(id: Option<&str>, name: &str, logo_change: Option<LogoChange>) -> SaveOption {
        SaveOption {
            id: id.map(str::to_owned),
            name: name.into(),
            is_archived: false,
            icon: None,
            kind: OptionKind::Payee,
            logo_change,
        }
    }
    async fn database() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }
    #[test]
    fn validates_pixels_bytes_and_format_and_strips_metadata() {
        let (data, width, height) = normalize(&sample(128, 64)).unwrap();
        assert_eq!((width, height), (128, 64));
        let parsed = png::Decoder::new(Cursor::new(&data)).read_info().unwrap();
        assert!(parsed.info().uncompressed_latin1_text.is_empty());
        assert!(data.len() < MAX_BYTES);
        assert!(normalize(&sample(129, 1)).is_err());
        assert!(normalize("data:image/svg+xml;base64,PHN2Zy8+").is_err());
        assert!(normalize("data:image/png;base64,not-a-png").is_err());
        assert!(normalize(&format!(
            "data:image/png;base64,{}",
            "A".repeat(MAX_BYTES * 2)
        ))
        .is_err());
    }
    #[tokio::test]
    async fn saves_logos_atomically_preserves_archived_links_and_cleans_unreferenced_assets() {
        let pool = database().await;
        let custom = || {
            Some(LogoChange::Custom {
                data: sample(64, 32),
            })
        };
        let bank = save_institution(&pool, institution(Some("bank-002"), "My bank", custom()))
            .await
            .unwrap();
        let id = bank.logo_asset_id.clone().unwrap();
        assert_eq!(bank.logo_mode, "custom");
        let shop = save_option(&pool, payee(None, "Shop", custom()))
            .await
            .unwrap();
        assert_eq!(shop.logo_asset_id.as_deref(), Some(id.as_str()));
        // A failed image replacement cannot partially rename its owner.
        assert!(save_institution(
            &pool,
            institution(
                Some("bank-002"),
                "Should roll back",
                Some(LogoChange::Custom { data: "bad".into() })
            )
        )
        .await
        .is_err());
        assert!(save_option(
            &pool,
            payee(
                Some(&shop.id),
                "Should roll back",
                Some(LogoChange::Custom {
                    data: sample(129, 1)
                })
            )
        )
        .await
        .is_err());
        let kept = save_option(
            &pool,
            SaveOption {
                is_archived: true,
                ..payee(Some(&shop.id), "Shop", None)
            },
        )
        .await
        .unwrap();
        assert_eq!(kept.logo_asset_id.as_deref(), Some(id.as_str()));
        let bank = save_institution(
            &pool,
            institution(Some("bank-002"), "My bank", Some(LogoChange::None)),
        )
        .await
        .unwrap();
        assert_eq!(bank.logo_mode, "none");
        assert!(bank.logo_asset_id.is_none());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM logo_assets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1); // Archived payees still own their logos.
        let bank = save_institution(
            &pool,
            institution(Some("bank-002"), "My bank", Some(LogoChange::Default)),
        )
        .await
        .unwrap();
        assert_eq!(bank.logo.as_deref(), Some("bbl.svg"));
        assert_eq!(bank.logo_mode, "default");
        save_option(&pool, payee(Some(&shop.id), "Shop", Some(LogoChange::None)))
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM logo_assets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert!(save_option(
            &pool,
            SaveOption {
                kind: OptionKind::Category,
                ..payee(None, "Category", custom())
            }
        )
        .await
        .is_err());
        // A downstream write failure rolls back asset creation too.
        sqlx::query("CREATE TRIGGER reject_logo BEFORE UPDATE OF logo_asset_id ON institutions BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&pool).await.unwrap();
        assert!(
            save_institution(&pool, institution(None, "New bank", custom()))
                .await
                .is_err()
        );
        let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM logo_assets),(SELECT count(*) FROM institutions WHERE name='New bank')").fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (0, 0));
    }
    #[tokio::test]
    async fn migration_preserves_records_and_snapshot_carries_logo_bytes_and_links() {
        let directory = std::env::temp_dir().join(format!(
            "heyday-logos-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let pool = SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(directory.join("live.db")).create_if_missing(true).foreign_keys(true)).await.unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 32)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("INSERT INTO payees(id,name,name_key,is_archived) VALUES('p','Shop','shop',1); INSERT INTO accounts(id,name,type,institution_id,institution,opening_balance,current_balance) VALUES('a','Savings','bank','bank-002','Bangkok Bank Public Company Limited',9007199254740993,9007199254740990); INSERT INTO transactions(id,type,account_id,amount,date,description,payee_id) VALUES('t','expense','a',3,'2026-01-01','','p');").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0032_custom_logos.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let baseline: (String, Option<String>) =
            sqlx::query_as("SELECT logo_mode,logo_asset_id FROM institutions WHERE id='bank-002'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(baseline, ("default".into(), None));
        let saved = save_institution(
            &pool,
            institution(
                Some("bank-002"),
                "My bank",
                Some(LogoChange::Custom {
                    data: sample(128, 64),
                }),
            ),
        )
        .await
        .unwrap();
        let shop = save_option(
            &pool,
            payee(
                Some("p"),
                "Shop",
                Some(LogoChange::Custom {
                    data: sample(128, 64),
                }),
            ),
        )
        .await
        .unwrap();
        assert_eq!(saved.logo_asset_id, shop.logo_asset_id);
        crate::updates::backup_database(&pool, &directory)
            .await
            .unwrap();
        pool.close().await;
        let file = std::fs::read_dir(directory.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let reopened = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(file)
                .read_only(true),
        )
        .await
        .unwrap();
        let (bytes,id): (Vec<u8>,String)=sqlx::query_as("SELECT l.data,l.id FROM logo_assets l JOIN payees p ON p.logo_asset_id=l.id WHERE p.id='p'").fetch_one(&reopened).await.unwrap();
        assert_eq!(bytes, normalize(&sample(128, 64)).unwrap().0);
        assert_eq!(Some(id), saved.logo_asset_id);
        let balance: (i64,i64,i64)=sqlx::query_as("SELECT a.opening_balance,a.current_balance,t.amount FROM accounts a JOIN transactions t ON t.account_id=a.id").fetch_one(&reopened).await.unwrap();
        assert_eq!(balance, (9007199254740993, 9007199254740990, 3));
        reopened.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}
