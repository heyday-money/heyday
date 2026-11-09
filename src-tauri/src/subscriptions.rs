//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::subscriptions as service;
pub use heyday_core::services::subscriptions::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn save_subscription(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveSubscription,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_subscription(pool.inner(), input).await
}

#[tauri::command]
pub async fn delete_subscription(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_subscription(pool.inner(), id).await
}

#[tauri::command]
pub async fn open_subscription_management(app: tauri::AppHandle, pool: tauri::State<'_, SqlitePool>, id: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let _database_operation = heyday_core::database::operations::operation()?;
    let url = service::saved_management_url(pool.inner(), &id).await?;
    app.opener().open_url(url, None::<&str>).map_err(|_| "Could not open the management link. Please try again.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn seed(pool: &SqlitePool) {
        heyday_core::database::MIGRATOR.run(pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance, is_archived) VALUES ('bank','Bank','bank',10000,9000,0),('card','Card','credit_card',5000,4000,0),('loan','Loan','loan',5000,4000,0),('old','Old','bank',100,100,1)").execute(pool).await.unwrap();
        sqlx::query("INSERT INTO categories (id,name,name_key,is_archived) VALUES ('services','Services','services',0),('old','Old','old',1)").execute(pool).await.unwrap();
    }
    fn input() -> SaveSubscription {
        SaveSubscription {
            id: None,
            name: " Streaming ".into(),
            account_id: "bank".into(),
            category_id: Some("services".into()),
            amount: "9007199254740993".into(),
            frequency: "monthly".into(),
            first_billing_date: "2024-01-31".into(),
            end_date: None,
            is_active: true,
            currency: "THB".into(),
            logo_change: None,
            provider_id: None,
            managed_via: None,
            management_url: None,
        }
    }
    async fn records(pool: &SqlitePool) -> Vec<Subscription> {
        sqlx::query_as(SELECT).fetch_all(pool).await.unwrap()
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id, opening_balance, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    #[test]
    fn management_links_require_https_without_credentials() {
        for value in ["javascript:alert(1)", "file:///tmp/file", "http://example.com", "https://user:pass@example.com", "https://user@example.com", "not a link", "https://example.com/\npath"] {
            assert!(management_url(Some(value)).is_err(), "{value}");
        }
        assert!(management_url(Some(&format!("https://example.com/{}", "x".repeat(2048)))).is_err());
        assert_eq!(management_url(Some("  https://example.com/settings  ")).unwrap().as_deref(), Some("https://example.com/settings"));
        assert_eq!(management_url(Some(" ")).unwrap(), None);
    }

    fn logo() -> crate::logos::LogoChange {
        use base64::Engine;
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&[128, 0, 255, 255]).unwrap();
        }
        crate::logos::LogoChange::Custom { data: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
    }

    #[tokio::test]
    async fn provider_links_reuse_icons_and_archived_links_remain_editable() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let mut create = input(); create.provider_id = Some("provider-netflix".into());
        save(&pool, create).await.unwrap();
        let row = records(&pool).await.remove(0);
        assert_eq!(row.provider_icon.as_deref(), Some("netflix"));
        let mut conn = pool.acquire().await.unwrap();
        let asset = crate::logos::asset(&mut conn, &logo()).await.unwrap().unwrap();
        sqlx::query("UPDATE subscription_providers SET logo_mode='custom',logo_asset_id=? WHERE id='provider-netflix'").bind(&asset).execute(&mut *conn).await.unwrap();
        drop(conn);
        assert_eq!(records(&pool).await[0].logo_asset_id.as_deref(), Some(asset.as_str()));
        sqlx::query("UPDATE subscription_providers SET is_archived=1,name='Renamed' WHERE id='provider-netflix'").execute(&pool).await.unwrap();
        assert_eq!(records(&pool).await[0].name, "Streaming");
        let mut new = input(); new.provider_id = Some("provider-netflix".into());
        assert!(save(&pool, new).await.is_err());
        let mut edit = input(); edit.id = Some(row.id.clone()); edit.provider_id = Some("provider-netflix".into());
        save(&pool, edit).await.unwrap();
        let mut invalid = input(); invalid.id = Some(row.id.clone()); invalid.provider_id = Some("missing".into());
        assert!(save(&pool, invalid).await.is_err());
        assert_eq!(records(&pool).await[0].provider_id.as_deref(), Some("provider-netflix"));
        let mut detach = input(); detach.id = Some(row.id); save(&pool, detach).await.unwrap();
        assert!(records(&pool).await[0].provider_id.is_none());
        let mut conn = pool.acquire().await.unwrap();
        crate::logos::prune(&mut conn).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM logo_assets WHERE id=?").bind(&asset).fetch_one(&mut *conn).await.unwrap(), 1);
        drop(conn);
        assert_eq!(balances(&pool).await, before);
    }

    #[tokio::test]
    async fn management_metadata_and_logos_are_atomic_and_preserve_shared_assets() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let mut create = input();
        create.logo_change = Some(logo());
        create.managed_via = Some("apple_app_store".into());
        create.management_url = Some(" https://example.com/subscriptions ".into());
        save(&pool, create).await.unwrap();
        let saved = records(&pool).await.remove(0);
        assert_eq!(saved.managed_via.as_deref(), Some("apple_app_store"));
        assert_eq!(saved.management_url.as_deref(), Some("https://example.com/subscriptions"));
        let asset = saved.logo_asset_id.unwrap();
        let mut edit = input();
        edit.id = Some(saved.id.clone());
        edit.managed_via = Some("in_app".into());
        save(&pool, edit).await.unwrap();
        let edited = records(&pool).await.remove(0);
        assert_eq!(edited.logo_asset_id.as_deref(), Some(asset.as_str()));
        assert_eq!(edited.managed_via.as_deref(), Some("in_app"));
        assert_eq!(edited.management_url, None);
        for invalid in [false, true] {
            let mut failed = input();
            failed.id = Some(saved.id.clone());
            failed.name = "Must roll back".into();
            if invalid { failed.managed_via = Some("invalid".into()); }
            else { failed.logo_change = Some(crate::logos::LogoChange::Custom { data: "bad".into() }); }
            assert!(save(&pool, failed).await.is_err());
            assert_eq!(records(&pool).await[0].name, "Streaming");
        }
        // Pruning for unrelated payee/institution edits must retain subscription-only logos.
        let mut conn = pool.acquire().await.unwrap();
        crate::logos::prune(&mut conn).await.unwrap();
        drop(conn);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM logo_assets").fetch_one(&pool).await.unwrap(), 1);
        sqlx::query("INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('shared','Shared','shared',?)").bind(&asset).execute(&pool).await.unwrap();
        remove(&pool, &saved.id).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM logo_assets").fetch_one(&pool).await.unwrap(), 1);
        let mut create = input(); create.logo_change = Some(logo()); save(&pool, create).await.unwrap();
        sqlx::query("UPDATE payees SET logo_asset_id=NULL WHERE id='shared'").execute(&pool).await.unwrap();
        let id = records(&pool).await[0].id.clone();
        let mut clear = input(); clear.id = Some(id); clear.logo_change = Some(crate::logos::LogoChange::None);
        save(&pool, clear).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM logo_assets").fetch_one(&pool).await.unwrap(), 0);
        assert_eq!(balances(&pool).await, before);
    }

    #[tokio::test]
    async fn metadata_migration_preserves_existing_subscriptions() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations").iter().filter(|m| m.version < 40) {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("INSERT INTO accounts(id,name,type) VALUES('bank','Bank','bank'); INSERT INTO subscriptions(id,name,account_id,amount,frequency,first_billing_date,is_active) VALUES('old','Existing','bank',9007199254740993,'yearly','2024-02-29',0);").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0040_subscription_management.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0003_subscription_providers.sql")).execute(&pool).await.unwrap();
        let row = records(&pool).await.remove(0);
        assert_eq!(row.id, "old"); assert_eq!(row.amount, "9007199254740993");
        assert_eq!(row.frequency, "yearly"); assert_eq!(row.first_billing_date, "2024-02-29"); assert!(!row.is_active);
        assert!(row.logo_asset_id.is_none() && row.managed_via.is_none() && row.management_url.is_none());
    }

    #[tokio::test]
    async fn wallet_can_fund_schedule_without_changing_balances() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        sqlx::query("UPDATE accounts SET type='wallet' WHERE id='bank'")
            .execute(&pool)
            .await
            .unwrap();
        let before = balances(&pool).await;
        save(&pool, input()).await.unwrap();
        assert_eq!(balances(&pool).await, before);
    }

    #[tokio::test]
    async fn schedules_persist_and_edits_pause_resume_remove_without_ledger_effects() {
        let path = std::env::temp_dir().join(format!(
            "heyday-subscriptions-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        save(&pool, input()).await.unwrap();
        let saved = records(&pool).await.remove(0);
        assert_eq!(saved.name, "Streaming");
        assert_eq!(saved.amount, "9007199254740993");
        pool.close().await;
        let pool = SqlitePool::connect_with(options).await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(records(&pool).await[0].id, saved.id);
        let mut edit = input();
        edit.id = Some(saved.id.clone());
        edit.is_active = false;
        edit.frequency = "yearly".into();
        edit.account_id = "card".into();
        edit.end_date = Some("2028-01-31".into());
        save(&pool, edit).await.unwrap();
        let stored = records(&pool).await.remove(0);
        assert!(!stored.is_active);
        assert_eq!(stored.frequency, "yearly");
        assert_eq!(stored.account_type, "credit_card");
        let mut resume = input();
        resume.id = Some(saved.id.clone());
        save(&pool, resume).await.unwrap();
        assert!(records(&pool).await[0].is_active);
        remove(&pool, &saved.id).await.unwrap();
        assert!(remove(&pool, &saved.id).await.is_err());
        assert_eq!(balances(&pool).await, before);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        pool.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
    #[tokio::test]
    async fn reject_invalid_and_stale_inputs_without_partial_writes() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let mut cases = vec![];
        for amount in ["0", "-1", "1.5", "9223372036854775808"] {
            let mut value = input();
            value.amount = amount.into();
            cases.push(value);
        }
        for date in ["bad", "2025-02-29", "0000-01-01"] {
            let mut value = input();
            value.first_billing_date = date.into();
            cases.push(value);
        }
        for date in ["2023-12-31", "bad"] {
            let mut value = input();
            value.end_date = Some(date.into());
            cases.push(value);
        }
        for account in ["loan", "old", "missing"] {
            let mut value = input();
            value.account_id = account.into();
            cases.push(value);
        }
        for category in ["old", "missing"] {
            let mut value = input();
            value.category_id = Some(category.into());
            cases.push(value);
        }
        let mut value = input();
        value.frequency = "weekly".into();
        cases.push(value);
        let mut value = input();
        value.name = " ".into();
        cases.push(value);
        let mut value = input();
        value.currency = "USD".into();
        cases.push(value);
        let mut value = input();
        value.id = Some("missing".into());
        cases.push(value);
        for value in cases {
            assert!(save(&pool, value).await.is_err());
        }
        assert!(records(&pool).await.is_empty());
        assert_eq!(balances(&pool).await, before);
        save(&pool, input()).await.unwrap();
        let id = records(&pool).await[0].id.clone();
        sqlx::query("UPDATE accounts SET is_archived = 1 WHERE id = 'bank'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE categories SET is_archived = 1 WHERE id = 'services'")
            .execute(&pool)
            .await
            .unwrap();
        let mut pause = input();
        pause.id = Some(id.clone());
        pause.is_active = false;
        save(&pool, pause).await.unwrap();
        let mut resume = input();
        resume.id = Some(id.clone());
        assert!(save(&pool, resume).await.is_err());
        assert!(!records(&pool).await[0].is_active);
        remove(&pool, &id).await.unwrap();
    }
}
