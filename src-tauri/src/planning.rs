use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct PaymentPlan {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    account_id: String,
    account_name: String,
    destination_account_id: Option<String>,
    destination_account_name: Option<String>,
    category_id: Option<String>,
    category_name: Option<String>,
    amount: String,
    date: String,
}
const SELECT: &str = "SELECT p.id, p.name, p.type AS kind, p.account_id, a.name AS account_name, p.destination_account_id, d.name AS destination_account_name, p.category_id, c.name AS category_name, CAST(p.amount AS TEXT) AS amount, p.date FROM payment_plans p JOIN accounts a ON a.id = p.account_id LEFT JOIN accounts d ON d.id = p.destination_account_id LEFT JOIN categories c ON c.id = p.category_id";

#[derive(Deserialize)]
pub struct SavePlan {
    id: Option<String>,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    account_id: String,
    destination_account_id: Option<String>,
    category_id: Option<String>,
    amount: String,
    date: String,
    currency: String,
}

async fn save(pool: &SqlitePool, input: SavePlan) -> Result<PaymentPlan, String> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a plan name between 1 and 100 characters.".into());
    }
    if !["expense", "repayment"].contains(&input.kind.as_str()) {
        return Err("Choose expense or repayment.".into());
    }
    if !crate::transactions::valid_date(&input.date) {
        return Err("Enter a valid date.".into());
    }
    let amount = input
        .amount
        .parse::<i64>()
        .map_err(|_| "Enter a valid integer amount within the supported range.")?;
    if amount <= 0 {
        return Err("Amount must be greater than zero.".into());
    }
    let repayment = input.kind == "repayment";
    if repayment != input.destination_account_id.is_some()
        || (repayment && input.category_id.is_some())
    {
        return Err("Repayments need a debt account; categories apply only to expenses.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if input.date <= today {
        return Err(
            "Plan a date after today. Record today's or past activity in Transactions.".into(),
        );
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload before saving the plan.".into());
    }
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if !valid {
        return Err("Choose an active cash or bank account to pay from.".into());
    }
    if let Some(id) = &input.destination_account_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('credit_card', 'loan'))").bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if !valid {
            return Err("Choose an active credit card or loan to repay.".into());
        }
    }
    if let Some(id) = &input.category_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ? AND is_archived = 0)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if !valid {
            return Err("Choose an active expense category.".into());
        }
    }
    let id: Option<String> = if let Some(id) = input.id {
        sqlx::query_scalar("UPDATE payment_plans SET name = ?, type = ?, account_id = ?, destination_account_id = ?, category_id = ?, amount = ?, date = ? WHERE id = ? RETURNING id")
            .bind(name).bind(input.kind).bind(input.account_id).bind(input.destination_account_id).bind(input.category_id).bind(amount).bind(input.date).bind(id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?
    } else {
        sqlx::query_scalar("INSERT INTO payment_plans (id, name, type, account_id, destination_account_id, category_id, amount, date) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?) RETURNING id")
            .bind(name).bind(input.kind).bind(input.account_id).bind(input.destination_account_id).bind(input.category_id).bind(amount).bind(input.date).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?
    };
    let id = id.ok_or("Plan no longer exists. Reload to continue.")?;
    let plan = sqlx::query_as(&format!("{SELECT} WHERE p.id = ?"))
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(plan)
}
#[tauri::command]
pub async fn save_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    input: SavePlan,
) -> Result<PaymentPlan, String> {
    save(pool.inner(), input).await
}
async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM payment_plans WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Plan no longer exists. Reload to continue.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn delete_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    remove(pool.inner(), &id).await
}

#[derive(Serialize)]
pub struct FinancialData {
    settings: crate::Settings,
    accounts: Vec<crate::accounts::Account>,
    incomes: Vec<crate::incomes::Income>,
    transactions: Vec<crate::transactions::Transaction>,
    plans: Vec<PaymentPlan>,
    installments: Vec<crate::installments::Installment>,
    subscriptions: Vec<crate::subscriptions::Subscription>,
    categories: Vec<crate::transaction_options::TransactionOption>,
}
async fn snapshot(pool: &SqlitePool) -> Result<FinancialData, String> {
    // All totals use a single consistent SQLite read snapshot.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let settings = sqlx::query_as("SELECT currency, period_start_day FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let accounts = sqlx::query_as(&format!(
        "SELECT {} FROM accounts WHERE is_archived = 0 ORDER BY created_at, id",
        crate::accounts::COLUMNS
    ))
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let incomes = sqlx::query_as(crate::incomes::SELECT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let transactions = sqlx::query_as(crate::transactions::SELECT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let plans = sqlx::query_as(&format!("{SELECT} ORDER BY p.date, p.created_at, p.id"))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let installments = sqlx::query_as(crate::installments::SELECT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let subscriptions = sqlx::query_as(crate::subscriptions::SELECT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let categories =
        sqlx::query_as("SELECT id, name, is_archived FROM categories ORDER BY name COLLATE NOCASE")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(FinancialData {
        settings,
        accounts,
        incomes,
        transactions,
        plans,
        installments,
        subscriptions,
        categories,
    })
}
#[tauri::command]
pub async fn get_financial_data(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<FinancialData, String> {
    snapshot(pool.inner()).await
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
        sqlx::migrate!("./migrations").run(&reopened).await.unwrap();
        let data = snapshot(&reopened).await.unwrap();
        assert_eq!(data.plans[0].id, created.id);
        assert_eq!(data.plans[0].amount, "9007199254740993");
        assert_eq!(data.plans[0].category_name.as_deref(), Some("Food"));
        assert!(data.transactions.is_empty());
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
