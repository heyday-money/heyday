use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashSet;

#[derive(Serialize, sqlx::FromRow)]
pub struct IncomeDeduction {
    pub id: String,
    pub income_id: String,
    pub name: String,
    pub description: String,
    pub amount: String,
}
pub(crate) const SELECT: &str =
    "SELECT id,income_id,name,description,CAST(amount AS TEXT) AS amount FROM income_deductions";

#[derive(Deserialize)]
pub struct DeductionInput {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub amount: String,
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
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,description,amount) VALUES(COALESCE(?,lower(hex(randomblob(16)))),?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,amount=excluded.amount")
            .bind(&row.id).bind(income_id).bind(row.name.trim()).bind(row.description.trim()).bind(amount).execute(&mut *conn).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub async fn list_income_deductions(
    pool: tauri::State<'_, SqlitePool>,
    income_id: String,
) -> Result<Vec<IncomeDeduction>, String> {
    sqlx::query_as(&format!("{SELECT} WHERE income_id=? ORDER BY rowid"))
        .bind(income_id)
        .fetch_all(pool.inner())
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
#[tauri::command]
pub async fn save_salary_deductions(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveSalaryDeductions,
) -> Result<crate::incomes::Income, String> {
    save(pool.inner(), input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    fn row(id: Option<&str>, name: &str, amount: &str) -> DeductionInput {
        DeductionInput {
            id: id.map(String::from),
            name: name.into(),
            description: "Per salary payment".into(),
            amount: amount.into(),
        }
    }
    fn input(rows: Vec<DeductionInput>) -> SaveSalaryDeductions {
        SaveSalaryDeductions {
            income_id: "salary".into(),
            currency: "THB".into(),
            deductions: rows,
        }
    }
    async fn seed(pool: &SqlitePool) {
        sqlx::migrate!("./migrations").run(pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000)").execute(pool).await.unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary','bank','salary',5000000,25),('other','Other','bank','other',5000000,25)").execute(pool).await.unwrap();
    }
    #[tokio::test]
    async fn validate_salary_deductions_and_preserve_ids_overrides_and_ledger() {
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
        save(
            &pool,
            input(vec![
                row(None, "Tax", "200000"),
                row(None, "Social", "75000"),
            ]),
        )
        .await
        .unwrap();
        let rows: Vec<IncomeDeduction> = sqlx::query_as(SELECT).fetch_all(&pool).await.unwrap();
        let id = rows.iter().find(|r| r.name == "Tax").unwrap().id.clone();
        let social = rows.iter().find(|r| r.name == "Social").unwrap().id.clone();
        sqlx::query("INSERT INTO planner_deduction_amounts VALUES(?,'2026-12',0)")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        save(
            &pool,
            input(vec![
                row(Some(&id), "Withholding Tax", "100000"),
                row(Some(&social), "Social", "75000"),
            ]),
        )
        .await
        .unwrap();
        let retained:(String,i64)=sqlx::query_as("SELECT d.name,p.amount FROM income_deductions d JOIN planner_deduction_amounts p ON p.deduction_id=d.id").fetch_one(&pool).await.unwrap();
        assert_eq!(retained, ("Withholding Tax".into(), 0));
        for rows in [
            vec![row(None, "Invalid", "-1")],
            vec![row(None, "Invalid", "1.5")],
            vec![row(None, "Too much", "5000001")],
            vec![row(Some("missing"), "Invalid", "1")],
            vec![row(Some(&id), "Dup", "1"), row(Some(&id), "Dup", "1")],
            vec![row(None, " ", "1")],
        ] {
            assert!(save(&pool, input(rows)).await.is_err());
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM income_deductions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
        let mut wrong = input(vec![row(None, "Other tax", "1")]);
        wrong.income_id = "other".into();
        assert!(save(&pool, wrong).await.is_err());
        let mut wrong = input(vec![]);
        wrong.currency = "USD".into();
        assert!(save(&pool, wrong).await.is_err());
        assert!(validate(
            &[
                row(None, "Large", "9223372036854775807"),
                row(None, "Overflow", "1")
            ],
            i64::MAX,
            true
        )
        .is_err());
        save(&pool, input(vec![])).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM planner_deduction_amounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let state:(i64,i64,i64)=sqlx::query_as("SELECT estimated_amount,current_balance,(SELECT count(*) FROM transactions) FROM incomes,accounts WHERE incomes.id='salary' AND accounts.id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(state, (5000000, 10000, 0));
    }
    #[tokio::test]
    async fn salary_deductions_and_zero_overrides_survive_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "heyday-deductions-{}-{}.db",
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
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        seed(&pool).await;
        save(
            &pool,
            input(vec![
                row(None, "Tax", "200000"),
                row(None, "Explicit zero", "0"),
            ]),
        )
        .await
        .unwrap();
        let id: String = sqlx::query_scalar("SELECT id FROM income_deductions WHERE name='Tax'")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO planner_deduction_amounts VALUES(?,'2026-12',0)")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let rows: Vec<IncomeDeduction> = sqlx::query_as(SELECT).fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.id == id && r.amount == "200000"));
        assert!(rows.iter().any(|r| r.amount == "0"));
        let amount: i64 =
            sqlx::query_scalar("SELECT amount FROM planner_deduction_amounts WHERE deduction_id=?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 0);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
