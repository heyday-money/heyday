//! Desktop IPC adapters; financial operations live in heyday-core.
pub use heyday_core::services::reset::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn clear_all_data(
    pool: tauri::State<'_, SqlitePool>,
    auth: tauri::State<'_, crate::cloud_auth::CloudAuthState>,
    confirmation: String,
) -> Result<(), String> {
    let _database_operation = heyday_core::database::operations::operation()?;
    let _auth = auth.operation.lock().await;
    clear_data(pool.inner(), &confirmation).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[tokio::test]
    async fn reset_requires_confirmation_is_atomic_and_preserves_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB', period_start_day = 25")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type) VALUES ('bank','Bank','bank')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql("INSERT INTO accounts(id,name,type) VALUES('card','Visa','credit_card');
        INSERT INTO card_statements(id,account_id,start_date,end_date,due_date,amount,minimum) VALUES('bill','card','2024-01-01','2024-01-31','2024-02-10',1000,100);
        INSERT INTO card_payment_plans VALUES('bill','bank','2024-02-10','full',1000);
        INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('payment','repayment','bank','card',100,'2024-02-01','');
        INSERT INTO card_payment_allocations VALUES('payment','bill');
        INSERT INTO card_limit_groups VALUES('shared','Shared','shared',10000);
        INSERT INTO card_limit_members VALUES('card','shared');
        INSERT INTO card_statement_entries VALUES('bill','evidence');").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES ('salary','Salary','bank','salary',50000,25)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO planner_amounts VALUES ('expenses-0','2026-09',100)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES('logo','hash','image/png',1,1,X'01');
        UPDATE institutions SET logo_asset_id='logo',logo_mode='custom' WHERE id='bank-002';
        INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('shop','Shop','shop','logo');")
            .execute(&pool).await.unwrap();
        assert!(clear_data(&pool, "delete").await.is_err());
        sqlx::query("CREATE TRIGGER prevent_reset BEFORE DELETE ON accounts BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&pool).await.unwrap();
        assert!(clear_data(&pool, "DELETE ALL DATA").await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM planner_amounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1); // Earlier deletions were rolled back.
        let logo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logo_assets")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(logo_count, 1);
        sqlx::query("DROP TRIGGER prevent_reset")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE institutions SET name='Renamed',name_key='renamed',is_archived=1 WHERE id='bank-002'").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO institutions(id,name,name_key) VALUES('custom','Custom','custom')",
        )
        .execute(&pool)
        .await
        .unwrap();
        clear_data(&pool, "DELETE ALL DATA").await.unwrap();
        let catalog: (i64, i64) =
            sqlx::query_as("SELECT count(*),sum(is_archived) FROM institutions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(catalog, (21, 0));
        let logo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logo_assets")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(logo_count, 0);
        let custom_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM institutions WHERE logo_mode <> 'default' OR logo_asset_id IS NOT NULL")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(custom_count, 0);
        let name: String = sqlx::query_scalar("SELECT name FROM institutions WHERE id='bank-002'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "Bangkok Bank Public Company Limited");
        for table in DATA_TABLES {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, if *table == "subscription_providers" { 12 } else { 0 }, "{table}");
        }
        let settings: (Option<String>, i64) =
            sqlx::query_as("SELECT currency, period_start_day FROM settings")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(settings, (None, 1));
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM planner_categories")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 6);
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        clear_data(&pool, "DELETE ALL DATA").await.unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type) VALUES ('new','New bank','bank')")
            .execute(&pool)
            .await
            .unwrap();
    }
}
