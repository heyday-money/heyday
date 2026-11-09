//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::planning as service;
pub use heyday_core::services::planning::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn save_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    input: SavePlan,
) -> Result<PaymentPlan, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_payment_plan(pool.inner(), input).await
}

#[tauri::command]
pub async fn delete_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_payment_plan(pool.inner(), id).await
}

#[tauri::command]
pub async fn get_financial_data(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<FinancialData, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_financial_data(pool.inner()).await
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
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance) VALUES ('bank','Bank','bank',10000,10000), ('card','Card','credit_card',5000,5000), ('archived','Archived','bank',10,10)").execute(pool).await.unwrap();
        sqlx::query("UPDATE accounts SET is_archived = 1 WHERE id = 'archived'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO categories (id, name, name_key, is_archived) VALUES ('food','Food','food',0), ('old','Old','old',1)").execute(pool).await.unwrap();
    }
    fn input() -> SavePlan {
        SavePlan {
            id: None,
            name: " Rent ".into(),
            kind: "expense".into(),
            account_id: "bank".into(),
            destination_account_id: None,
            category_id: Some("food".into()),
            amount: "9007199254740993".into(),
            date: "2999-02-28".into(),
            currency: "THB".into(),
        }
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
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
    async fn save_edit_delete_plans_never_change_balances_or_transactions() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let created = save(&pool, input()).await.unwrap();
        assert_eq!(created.amount, "9007199254740993");
        assert_eq!(created.name, "Rent");
        let mut edit = input();
        edit.id = Some(created.id.clone());
        edit.kind = "repayment".into();
        edit.destination_account_id = Some("card".into());
        edit.category_id = None;
        edit.amount = "1000".into();
        let updated = save(&pool, edit).await.unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.destination_account_name.as_deref(), Some("Card"));
        let data = snapshot(&pool).await.unwrap();
        assert_eq!(data.plans.len(), 1);
        assert_eq!(data.accounts.len(), 2);
        assert_eq!(data.transactions.len(), 0);
        assert_eq!(balances(&pool).await, before);
        remove(&pool, &created.id).await.unwrap();
        assert!(remove(&pool, &created.id).await.is_err());
        assert_eq!(balances(&pool).await, before);
        assert!(snapshot(&pool).await.unwrap().plans.is_empty());
    }
    #[tokio::test]
    async fn rejects_invalid_and_stale_plan_inputs_atomically() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let mut cases = vec![];
        for amount in ["0", "-1", "1.5", "9223372036854775808"] {
            let mut value = input();
            value.amount = amount.into();
            cases.push(value);
        }
        for date in ["2999-02-29", "2000-01-01", "bad"] {
            let mut value = input();
            value.date = date.into();
            cases.push(value);
        }
        let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
            .fetch_one(&pool)
            .await
            .unwrap();
        let mut value = input();
        value.date = today;
        cases.push(value);
        for id in ["missing", "card", "archived"] {
            let mut value = input();
            value.account_id = id.into();
            cases.push(value);
        }
        for id in ["missing", "old"] {
            let mut value = input();
            value.category_id = Some(id.into());
            cases.push(value);
        }
        let mut value = input();
        value.currency = "USD".into();
        cases.push(value);
        let mut value = input();
        value.kind = "income".into();
        cases.push(value);
        let mut value = input();
        value.kind = "repayment".into();
        cases.push(value);
        let mut value = input();
        value.name = " ".into();
        cases.push(value);
        let mut value = input();
        value.name = "x".repeat(101);
        cases.push(value);
        let mut value = input();
        value.id = Some("missing".into());
        cases.push(value);
        for target in ["bank", "missing", "archived"] {
            let mut value = input();
            value.kind = "repayment".into();
            value.category_id = None;
            value.destination_account_id = Some(target.into());
            cases.push(value);
        }
        let before = balances(&pool).await;
        for value in cases {
            assert!(save(&pool, value).await.is_err());
        }
        assert_eq!(balances(&pool).await, before);
        assert!(snapshot(&pool).await.unwrap().plans.is_empty());
    }
    #[tokio::test]
    async fn plans_and_snapshot_survive_restart() {
        let path = std::env::temp_dir().join(format!(
            "heyday-plans-{}-{}.db",
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
        let created = save(&pool, input()).await.unwrap();
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        heyday_core::database::MIGRATOR.run(&reopened).await.unwrap();
        let data = snapshot(&reopened).await.unwrap();
        assert_eq!(data.plans[0].id, created.id);
        assert_eq!(data.plans[0].amount, "9007199254740993");
        assert_eq!(data.plans[0].category_name.as_deref(), Some("Food"));
        assert!(data.transactions.is_empty());
        reopened.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
}
