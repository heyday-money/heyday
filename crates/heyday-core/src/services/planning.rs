use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct PaymentPlan {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub account_name: String,
    pub destination_account_id: Option<String>,
    pub destination_account_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub amount: String,
    pub date: String,
}
pub const SELECT: &str = "SELECT p.id, p.name, p.type AS kind, p.account_id, a.name AS account_name, p.destination_account_id, d.name AS destination_account_name, p.category_id, c.name AS category_name, CAST(p.amount AS TEXT) AS amount, p.date FROM payment_plans p JOIN accounts a ON a.id = p.account_id LEFT JOIN accounts d ON d.id = p.destination_account_id LEFT JOIN categories c ON c.id = p.category_id";

#[derive(Deserialize)]
pub struct SavePlan {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub destination_account_id: Option<String>,
    pub category_id: Option<String>,
    pub amount: String,
    pub date: String,
    pub currency: String,
}

pub async fn save(pool: &SqlitePool, input: SavePlan) -> Result<PaymentPlan, String> {
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
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank', 'wallet'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if !valid {
        return Err("Choose an active cash, bank, or digital wallet account to pay from.".into());
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
pub async fn save_payment_plan(pool: &SqlitePool, input: SavePlan) -> Result<PaymentPlan, String> {
    save(pool, input).await
}
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
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
pub async fn delete_payment_plan(pool: &SqlitePool, id: String) -> Result<(), String> {
    remove(pool, &id).await
}

#[derive(Serialize)]
pub struct FinancialData {
    pub selective_defaults: Vec<crate::selective_defaults::Exclusion>,
    pub card_billing: crate::card_billing::BillingData,
    pub loan_contracts: Vec<crate::loans::Contract>,
    pub loan_facilities: Vec<crate::loans::Facility>,
    pub settings: crate::Settings,
    pub accounts: Vec<crate::accounts::Account>,
    pub incomes: Vec<crate::incomes::Income>,
    pub income_deductions: Vec<crate::income_deductions::IncomeDeduction>,
    pub transactions: Vec<crate::transactions::Transaction>,
    pub plans: Vec<PaymentPlan>,
    pub installments: Vec<crate::installments::Installment>,
    pub subscriptions: Vec<crate::subscriptions::Subscription>,
    pub categories: Vec<crate::transaction_options::TransactionOption>,
}
pub async fn snapshot(pool: &SqlitePool) -> Result<FinancialData, String> {
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
    let transactions = sqlx::query_as(&format!(
        "{} ORDER BY t.date DESC, t.created_at DESC, t.id DESC",
        crate::transactions::SELECT
    ))
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
    let loan_contracts = crate::loans::contracts(&mut tx)
        .await
        .map_err(|e| e.to_string())?;
    let loan_facilities = crate::loans::facilities(&mut tx)
        .await
        .map_err(|e| e.to_string())?;
    let card_billing = crate::card_billing::snapshot(&mut tx)
        .await
        .map_err(|e| e.to_string())?;
    let income_deductions = sqlx::query_as(crate::income_deductions::SELECT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let selective_defaults = crate::selective_defaults::periods(&mut tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(FinancialData {
        selective_defaults,
        card_billing,
        loan_contracts,
        loan_facilities,
        settings,
        accounts,
        incomes,
        income_deductions,
        transactions,
        plans,
        installments,
        subscriptions,
        categories,
    })
}
pub async fn get_financial_data(pool: &SqlitePool) -> Result<FinancialData, String> {
    snapshot(pool).await
}
