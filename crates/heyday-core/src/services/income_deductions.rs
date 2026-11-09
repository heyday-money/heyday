use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashSet;

#[derive(Serialize, sqlx::FromRow)]
pub struct IncomeDeduction {
    pub debt_account_name: Option<String>,
    pub id: String,
    pub income_id: String,
    pub name: String,
    pub description: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
}
pub const SELECT: &str =
    "SELECT id,income_id,name,description,CAST(amount AS TEXT) AS amount,debt_account_id,(SELECT name FROM accounts WHERE id=income_deductions.debt_account_id) AS debt_account_name FROM income_deductions";

#[derive(Deserialize)]
pub struct DeductionInput {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
}
#[derive(Deserialize)]
pub struct SaveSalaryDeductions {
    pub income_id: String,
    pub currency: String,
    pub deductions: Vec<DeductionInput>,
}
pub fn validate(rows: &[DeductionInput], gross: i64, salary: bool) -> Result<(), String> {
    if !salary && !rows.is_empty() {
        return Err("Only salary sources can have income deductions.".into());
    }
    if rows.len() > 100 {
        return Err("A salary can have at most 100 deductions.".into());
    }
    let mut ids = HashSet::new();
    let mut total = 0i64;
    for row in rows {
        if row.name.trim().is_empty()
            || row.name.trim().chars().count() > 100
            || row.description.chars().count() > 2000
        {
            return Err("Enter a deduction name up to 100 characters and description up to 2000 characters.".into());
        }
        if let Some(id) = &row.id {
            if !ids.insert(id) {
                return Err("Duplicate deduction ID.".into());
            }
        }
        if row.amount.is_empty() || !row.amount.bytes().all(|c| c.is_ascii_digit()) {
            return Err("Deductions must be non-negative integer minor-unit amounts.".into());
        }
        let amount: i64 = row
            .amount
            .parse()
            .map_err(|_| "Deduction amount is outside the supported range.")?;
        total = total
            .checked_add(amount)
            .ok_or("Total deductions exceed the supported range.")?;
    }
    if total > gross {
        return Err("Total deductions cannot exceed gross salary.".into());
    }
    Ok(())
}
pub async fn replace(
    conn: &mut SqliteConnection,
    income_id: &str,
    rows: &[DeductionInput],
) -> Result<(), String> {
    let owned: Vec<String> =
        sqlx::query_scalar("SELECT id FROM income_deductions WHERE income_id=?")
            .bind(income_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    for row in rows {
        if let Some(account_id) = &row.debt_account_id {
            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card') AND (is_archived=0 OR EXISTS(SELECT 1 FROM income_deductions WHERE id=? AND income_id=? AND debt_account_id=accounts.id)))")
                .bind(account_id).bind(&row.id).bind(income_id).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
            if !valid {
                return Err(
                    "Choose an active loan or credit card account for the deduction.".into(),
                );
            }
        }
        if row.id.as_ref().is_some_and(|id| !owned.contains(id)) {
            return Err("Deduction is unavailable or belongs to another income source.".into());
        }
    }
    for id in &owned {
        if !rows.iter().any(|row| row.id.as_ref() == Some(id)) {
            sqlx::query("DELETE FROM income_deductions WHERE id=? AND income_id=?")
                .bind(id)
                .bind(income_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    for row in rows {
        let amount: i64 = row
            .amount
            .parse()
            .map_err(|_| "Invalid deduction amount.")?;
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,description,amount,debt_account_id) VALUES(COALESCE(?,lower(hex(randomblob(16)))),?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,amount=excluded.amount,debt_account_id=excluded.debt_account_id")
            .bind(&row.id).bind(income_id).bind(row.name.trim()).bind(row.description.trim()).bind(amount).bind(&row.debt_account_id).execute(&mut *conn).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub async fn list_income_deductions(
    pool: &SqlitePool,
    income_id: String,
) -> Result<Vec<IncomeDeduction>, String> {
    sqlx::query_as(&format!("{SELECT} WHERE income_id=? ORDER BY rowid"))
        .bind(income_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}
pub async fn save(
    pool: &SqlitePool,
    input: SaveSalaryDeductions,
) -> Result<crate::incomes::Income, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(&input.currency) {
        return Err("Currency changed. Reload Income before saving.".into());
    }
    let (kind, gross): (String, i64) =
        sqlx::query_as("SELECT type,estimated_amount FROM incomes WHERE id=?")
            .bind(&input.income_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Income source is unavailable.")?;
    if kind != "salary" {
        return Err("Only salary sources can have income deductions.".into());
    }
    validate(&input.deductions, gross, true)?;
    replace(&mut tx, &input.income_id, &input.deductions).await?;
    sqlx::query("UPDATE incomes SET updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(&input.income_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let income = sqlx::query_as(&format!("{} WHERE i.id=?", crate::incomes::SELECT))
        .bind(&input.income_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(income)
}
pub async fn save_salary_deductions(
    pool: &SqlitePool,
    input: SaveSalaryDeductions,
) -> Result<crate::incomes::Income, String> {
    save(pool, input).await
}
