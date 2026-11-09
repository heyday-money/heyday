use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Income {
    pub id: String,
    pub name: String,
    pub destination_account_id: String,
    pub destination_account_name: String,
    #[serde(rename = "type")]
    pub income_type: String,
    pub estimated_amount: String,
    pub deductions_total: String,
    pub recurrence_frequency: String,
    pub recurrence_day_of_month: i64,
    pub is_auto_create_transaction: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub const SELECT: &str = "SELECT i.id, i.name, i.destination_account_id, a.name AS destination_account_name, i.type AS income_type, CAST(i.estimated_amount AS TEXT) AS estimated_amount, CAST(COALESCE((SELECT SUM(d.amount) FROM income_deductions d WHERE d.income_id=i.id),0) AS TEXT) AS deductions_total, CASE WHEN i.is_irregular=1 THEN 'irregular' ELSE i.recurrence_frequency END AS recurrence_frequency, i.recurrence_day_of_month, i.is_auto_create_transaction, i.is_active, i.created_at, i.updated_at FROM incomes i JOIN accounts a ON a.id = i.destination_account_id";

#[derive(Deserialize)]
pub struct NewIncome {
    #[serde(default)]
    pub deductions: Vec<crate::income_deductions::DeductionInput>,
    pub name: String,
    pub destination_account_id: String,
    #[serde(rename = "type")]
    pub income_type: String,
    pub estimated_amount: String,
    pub currency: String,
    pub recurrence_frequency: String,
    pub recurrence_day_of_month: i64,
    pub is_auto_create_transaction: bool,
    pub is_active: bool,
}

pub fn validate_definition(input: &NewIncome) -> Result<i64, String> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter an income name between 1 and 100 characters.".into());
    }
    if !["salary", "variable", "investment", "other"].contains(&input.income_type.as_str()) {
        return Err("Choose a supported income type.".into());
    }
    let amount = input
        .estimated_amount
        .parse::<i64>()
        .map_err(|_| "Estimated amount is outside the supported range.")?;
    if amount < 0 {
        return Err("Estimated amount cannot be negative.".into());
    }
    crate::income_deductions::validate(&input.deductions, amount, input.income_type == "salary")?;
    if !["monthly", "irregular"].contains(&input.recurrence_frequency.as_str()) {
        return Err("Choose monthly or irregular income.".into());
    }
    if input.recurrence_frequency == "irregular" && amount != 0 {
        return Err(
            "Irregular income has no recurring estimate. Enter estimates in Outlook.".into(),
        );
    }
    if !(1..=31).contains(&input.recurrence_day_of_month) {
        return Err("Income day must be between 1 and 31.".into());
    }
    if input.is_auto_create_transaction {
        return Err("Automatic transaction creation is not available yet.".into());
    }
    Ok(amount)
}

pub async fn insert_income(pool: &SqlitePool, input: NewIncome) -> Result<Income, String> {
    let amount = validate_definition(&input)?;
    let name = input.name.trim();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err(
            "Choose the app currency in Settings, then reload income before saving.".into(),
        );
    }
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0)",
    )
    .bind(&input.destination_account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if !valid {
        return Err("Choose an existing, active destination account.".into());
    }
    let id: String = sqlx::query_scalar("INSERT INTO incomes (id, name, destination_account_id, type, estimated_amount, recurrence_frequency, recurrence_day_of_month, is_auto_create_transaction, is_active, is_irregular) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, 'monthly', ?, 0, ?, ?) RETURNING id")
        .bind(name).bind(input.destination_account_id).bind(input.income_type).bind(amount)
        .bind(input.recurrence_day_of_month).bind(input.is_active).bind(input.recurrence_frequency == "irregular")
        .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    crate::income_deductions::replace(&mut tx, &id, &input.deductions).await?;
    let income = sqlx::query_as::<_, Income>(&format!("{SELECT} WHERE i.id = ?"))
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(income)
}

#[derive(Deserialize)]
pub struct UpdateIncome {
    pub id: String,
    #[serde(flatten)]
    pub changes: NewIncome,
}
pub async fn update(pool: &SqlitePool, input: UpdateIncome) -> Result<Income, String> {
    let amount = validate_definition(&input.changes)?;
    if !input.changes.deductions.is_empty() {
        return Err("Manage existing salary deductions separately in Income.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.changes.currency.as_str()) {
        return Err("Currency changed. Reload Income before saving.".into());
    }
    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT type,destination_account_id FROM incomes WHERE id=?")
            .bind(&input.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (kind, destination) =
        existing.ok_or("Income source no longer exists. Reload to continue.")?;
    if kind != input.changes.income_type {
        return Err(
            "Income type cannot be changed. Add a separate source for a different type.".into(),
        );
    }
    let deductions: Vec<i64> =
        sqlx::query_scalar("SELECT amount FROM income_deductions WHERE income_id=?")
            .bind(&input.id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let total = deductions
        .into_iter()
        .try_fold(0i64, |sum, value| sum.checked_add(value))
        .ok_or("Deductions exceed the supported range.")?;
    if total > amount {
        return Err("Estimated gross salary cannot be lower than its existing deductions. Adjust deductions in Income first.".into());
    }
    let available: Option<bool> =
        sqlx::query_scalar("SELECT is_archived=0 FROM accounts WHERE id=?")
            .bind(&input.changes.destination_account_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    if available != Some(true)
        && !(available == Some(false)
            && destination == input.changes.destination_account_id
            && !input.changes.is_active)
    {
        return Err("Choose an active destination account. An inactive source may keep its existing archived destination.".into());
    }
    sqlx::query("UPDATE incomes SET name=?,destination_account_id=?,estimated_amount=?,is_irregular=?,recurrence_day_of_month=?,is_active=?,is_auto_create_transaction=0,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(input.changes.name.trim()).bind(input.changes.destination_account_id).bind(amount).bind(input.changes.recurrence_frequency == "irregular").bind(input.changes.recurrence_day_of_month).bind(input.changes.is_active).bind(&input.id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let result = sqlx::query_as::<_, Income>(&format!("{SELECT} WHERE i.id=?"))
        .bind(&input.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
pub async fn update_income(pool: &SqlitePool, input: UpdateIncome) -> Result<Income, String> {
    update(pool, input).await
}

pub async fn create_income(pool: &SqlitePool, input: NewIncome) -> Result<Income, String> {
    insert_income(pool, input).await
}

pub async fn list_incomes(pool: &SqlitePool) -> Result<Vec<Income>, String> {
    sqlx::query_as::<_, Income>(&format!("{SELECT} ORDER BY i.created_at, i.id"))
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}
