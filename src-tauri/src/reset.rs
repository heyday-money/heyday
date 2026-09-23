use sqlx::SqlitePool;

// Child-first order preserves foreign-key enforcement throughout the reset.
const DATA_TABLES: &[&str] = &[
    "transaction_verifications",
    "reconciliation_entries",
    "reconciliations",
    "planner_amounts",
    "planner_income_amounts",
    "planner_deduction_amounts",
    "planner_debt_amounts",
    "planner_installment_amounts",
    "planner_months",
    "planner_opening",
    "planner_items",
    "subscriptions",
    "payment_plans",
    "installments",
    "transactions",
    "income_deductions",
    "incomes",
    "accounts",
    "payees",
    "categories",
];

async fn clear_data(pool: &SqlitePool, confirmation: &str) -> Result<(), String> {
    if confirmation != "DELETE ALL DATA" {
        return Err("Type DELETE ALL DATA to confirm.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for table in DATA_TABLES {
        // Identifiers are a fixed internal allowlist, never user input.
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE settings SET currency = NULL, period_start_day = 1 WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_all_data(
    pool: tauri::State<'_, SqlitePool>,
    confirmation: String,
) -> Result<(), String> {
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB', period_start_day = 25")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type) VALUES ('bank','Bank','bank')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES ('salary','Salary','bank','salary',50000,25)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO planner_amounts VALUES ('expenses-0','2026-09',100)")
            .execute(&pool)
            .await
            .unwrap();
        assert!(clear_data(&pool, "delete").await.is_err());
        sqlx::query("CREATE TRIGGER prevent_reset BEFORE DELETE ON accounts BEGIN SELECT RAISE(ABORT, 'test failure'); END").execute(&pool).await.unwrap();
        assert!(clear_data(&pool, "DELETE ALL DATA").await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM planner_amounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1); // Earlier deletions were rolled back.
        sqlx::query("DROP TRIGGER prevent_reset")
            .execute(&pool)
            .await
            .unwrap();
        clear_data(&pool, "DELETE ALL DATA").await.unwrap();
        for table in DATA_TABLES {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table}");
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        clear_data(&pool, "DELETE ALL DATA").await.unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type) VALUES ('new','New bank','bank')")
            .execute(&pool)
            .await
            .unwrap();
    }
}
