//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::logos as service;
pub use heyday_core::services::logos::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_logo_asset(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<String, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_logo_asset(pool.inner(), id).await
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::io::Cursor;
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 32)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("INSERT INTO payees(id,name,name_key,is_archived) VALUES('p','Shop','shop',1); INSERT INTO accounts(id,name,type,institution_id,institution,opening_balance,current_balance) VALUES('a','Savings','bank','bank-002','Bangkok Bank Public Company Limited',9007199254740993,9007199254740990); INSERT INTO transactions(id,type,account_id,amount,date,description,payee_id) VALUES('t','expense','a',3,'2026-01-01','','p');").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0032_custom_logos.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0040_subscription_management.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0003_subscription_providers.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0011_cash_account_logos.sql").split("CREATE TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
        crate::updates::backup_database(&pool)
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
        crate::test_support::remove_dir_all(&directory).await.unwrap();
    }
}
