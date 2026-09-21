use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Subscription {
    id: String,
    name: String,
    account_id: String,
    account_name: String,
    account_type: String,
    category_id: Option<String>,
    category_name: Option<String>,
    amount: String,
    frequency: String,
    first_billing_date: String,
    end_date: Option<String>,
    is_active: bool,
}
pub(crate) const SELECT: &str = "SELECT s.id, s.name, s.account_id, a.name AS account_name, a.type AS account_type, s.category_id, c.name AS category_name, CAST(s.amount AS TEXT) AS amount, s.frequency, s.first_billing_date, s.end_date, s.is_active FROM subscriptions s JOIN accounts a ON a.id = s.account_id LEFT JOIN categories c ON c.id = s.category_id ORDER BY s.name COLLATE NOCASE, s.id";

#[derive(Deserialize)]
pub struct SaveSubscription {
    id: Option<String>,
    name: String,
    account_id: String,
    category_id: Option<String>,
    amount: String,
    frequency: String,
    first_billing_date: String,
    end_date: Option<String>,
    is_active: bool,
    currency: String,
}
async fn save(pool: &SqlitePool, input: SaveSubscription) -> Result<(), String> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a subscription name between 1 and 100 characters.".into());
    }
    let amount = input
        .amount
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if amount <= 0 {
        return Err("Amount must be greater than zero.".into());
    }
    if !["monthly", "yearly"].contains(&input.frequency.as_str()) {
        return Err("Choose monthly or yearly billing.".into());
    }
    if !crate::transactions::valid_date(&input.first_billing_date) {
        return Err("Enter a valid first billing date.".into());
    }
    if input.end_date.as_ref().is_some_and(|date| {
        !crate::transactions::valid_date(date) || date < &input.first_billing_date
    }) {
        return Err("End date must be valid and no earlier than the first billing date.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload before saving the subscription.".into());
    }
    let account: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank', 'credit_card'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    // A paused existing schedule can keep its unavailable account so it can be disabled.
    let unchanged_account: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id = ? AND account_id = ?)",
    )
    .bind(&input.id)
    .bind(&input.account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if !account && (input.is_active || !unchanged_account) {
        return Err("Choose an active cash, bank, or credit card account.".into());
    }
    if let Some(id) = &input.category_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE id = ? AND (is_archived = 0 OR EXISTS(SELECT 1 FROM subscriptions WHERE id = ? AND category_id = categories.id)))").bind(id).bind(&input.id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if !valid {
            return Err("Choose an active category.".into());
        }
    }
    let result = if let Some(id) = input.id {
        sqlx::query("UPDATE subscriptions SET name = ?, account_id = ?, category_id = ?, amount = ?, frequency = ?, first_billing_date = ?, end_date = ?, is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(name).bind(input.account_id).bind(input.category_id).bind(amount).bind(input.frequency).bind(input.first_billing_date).bind(input.end_date).bind(input.is_active).bind(id).execute(&mut *tx).await
    } else {
        sqlx::query("INSERT INTO subscriptions (id, name, account_id, category_id, amount, frequency, first_billing_date, end_date, is_active) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(name).bind(input.account_id).bind(input.category_id).bind(amount).bind(input.frequency).bind(input.first_billing_date).bind(input.end_date).bind(input.is_active).execute(&mut *tx).await
    }.map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Subscription no longer exists. Reload to continue.".into());
    }
    tx.commit().await.map_err(|e| e.to_string())
}
async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM subscriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Subscription no longer exists. Reload to continue.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn save_subscription(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveSubscription,
) -> Result<(), String> {
    save(pool.inner(), input).await
}
#[tauri::command]
pub async fn delete_subscription(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    remove(pool.inner(), &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn seed(pool: &SqlitePool) {
        sqlx::migrate!("./migrations").run(pool).await.unwrap();
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
        std::fs::remove_file(path).unwrap();
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
