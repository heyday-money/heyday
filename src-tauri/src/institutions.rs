//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::institutions as service;
pub use heyday_core::services::institutions::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn list_institutions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<InstitutionDirectory, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_institutions(pool.inner()).await
}

#[tauri::command]
pub async fn delete_institution(pool: tauri::State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_institution(pool.inner(), id).await
}

#[tauri::command]
pub async fn save_institution(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveInstitution,
) -> Result<Institution, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_institution(pool.inner(), input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn database() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap()
    }
    fn account(kind: &str, id: Option<&str>, name: Option<&str>) -> crate::accounts::NewAccount {
        serde_json::from_value(serde_json::json!({"name":"My account", "type":kind,"loan_type":if kind=="loan" {Some("personal_loan")} else {None},"opening_balance":"9007199254740993","currency":"THB","institution_id":id,"institution":name})).unwrap()
    }
    #[tokio::test]
    async fn deletion_protects_accounts_history_and_is_atomic() {
        let pool = database().await;
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,institution_id,is_archived) VALUES('a','Archived','bank','bank-002',1),('b','Active','bank','bank-004',0);
        INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('t','transfer','a','b',125,'2026-01-01','');").execute(&pool).await.unwrap();
        for id in ["bank-002", "bank-004"] { assert!(delete(&pool, id).await.unwrap_err().contains("linked accounts")); }
        let history: i64 = sqlx::query_scalar("SELECT amount FROM transactions WHERE id='t'").fetch_one(&pool).await.unwrap();
        assert_eq!(history, 125);
        assert!(delete(&pool, "missing").await.unwrap_err().contains("no longer exists"));
        sqlx::raw_sql("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES('unused','one','image/png',1,1,X'01'),('shared','two','image/png',1,1,X'02');
        UPDATE institutions SET logo_asset_id='unused',logo_mode='custom',is_archived=1 WHERE id='bank-006';
        UPDATE institutions SET logo_asset_id='shared',logo_mode='custom' WHERE id='bank-014';
        INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('p','Shop','shop','shared');
        CREATE TRIGGER fail_prune BEFORE DELETE ON logo_assets BEGIN SELECT RAISE(ABORT,'test failure'); END;").execute(&pool).await.unwrap();
        assert!(delete(&pool, "bank-006").await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM institutions WHERE id='bank-006'").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        sqlx::query("DROP TRIGGER fail_prune").execute(&pool).await.unwrap();
        delete(&pool, "bank-006").await.unwrap();
        delete(&pool, "bank-014").await.unwrap();
        let assets: Vec<String> = sqlx::query_scalar("SELECT id FROM logo_assets").fetch_all(&pool).await.unwrap();
        assert_eq!(assets, vec!["shared"]);
        let broken: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(&pool).await.unwrap();
        assert_eq!(broken, 0);
    }
    #[tokio::test]
    async fn migrate_free_text_links_and_preserve_history() {
        let pool = database().await;
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 31)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,institution,opening_balance,current_balance,is_archived) VALUES ('bank','Savings','bank',' KBANK ',9007199254740993,9007199254740990,1),('custom','Other','wallet',' My  Provider ',100,99,0),('same','Second','investment','my provider',25,25,0); INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('t','expense','bank',3,'2026-01-01','');").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0031_institutions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let row: (String,String,i64,i64,i64) = sqlx::query_as("SELECT institution_id,institution,opening_balance,current_balance,is_archived FROM accounts WHERE id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "bank-004".into(),
                "Kasikornbank Public Company Limited".into(),
                9007199254740993,
                9007199254740990,
                1
            )
        );
        let custom: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT institution_id) FROM accounts WHERE id IN ('custom','same')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(custom, 1);
        save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: Some("bank-004".into()),
                name: "Renamed bank".into(),
                is_archived: false,
            },
        )
        .await
        .unwrap();
        let history: (String,i64,String) = sqlx::query_as("SELECT a.institution,t.amount,a.name FROM transactions t JOIN accounts a ON a.id=t.account_id").fetch_one(&pool).await.unwrap();
        assert_eq!(history, ("Renamed bank".into(), 3, "Savings".into()));
        let violations: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(violations, 0);
    }
    #[tokio::test]
    async fn stable_ids_validation_archive_and_atomic_creation() {
        let pool = database().await;
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        crate::save_currency(&pool, "THB").await.unwrap();
        for kind in ["bank", "wallet", "credit_card", "loan", "investment"] {
            crate::accounts::insert_account(
                &pool,
                account(kind, Some("bank-002"), Some("stale display name")),
            )
            .await
            .unwrap();
        }
        assert!(
            crate::accounts::insert_account(&pool, account("cash", Some("bank-002"), None))
                .await
                .is_err()
        );
        assert!(
            crate::accounts::insert_account(&pool, account("bank", Some("missing"), None))
                .await
                .is_err()
        );
        let renamed = save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: Some("bank-002".into()),
                name: "  My   BANK ".into(),
                is_archived: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(renamed.name, "My BANK");
        assert_eq!(renamed.logo.as_deref(), Some("bbl.svg"));
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: "my bank".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: " ".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: "a".repeat(101),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(
            crate::accounts::insert_account(&pool, account("bank", Some("bank-002"), None))
                .await
                .is_err()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM accounts WHERE institution='My BANK'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 5);
        // Existing links to archived institutions remain valid when editing other details.
        let mut tx = pool.begin().await.unwrap();
        assert!(
            resolve(&mut tx, "bank", Some("bank-002"), None, Some("bank-002"))
                .await
                .is_ok()
        );
        tx.rollback().await.unwrap();
        // Failure after institution resolution rolls back both new records.
        sqlx::query("CREATE TRIGGER fail_account BEFORE INSERT ON accounts BEGIN SELECT RAISE(ABORT,'test'); END").execute(&pool).await.unwrap();
        assert!(crate::accounts::insert_account(
            &pool,
            account("bank", None, Some("New custom provider"))
        )
        .await
        .is_err());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM institutions WHERE name='New custom provider'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        sqlx::query("DROP TRIGGER fail_account")
            .execute(&pool)
            .await
            .unwrap();
        crate::accounts::insert_account(
            &pool,
            account("wallet", None, Some("New custom provider")),
        )
        .await
        .unwrap();
        let logo: Option<String> =
            sqlx::query_scalar("SELECT logo FROM institutions WHERE name='New custom provider'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(logo, None);
    }
}
