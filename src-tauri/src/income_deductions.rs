//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::income_deductions as service;
pub use heyday_core::services::income_deductions::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn list_income_deductions(
    pool: tauri::State<'_, SqlitePool>,
    income_id: String,
) -> Result<Vec<IncomeDeduction>, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_income_deductions(pool.inner(), income_id).await
}

#[tauri::command]
pub async fn save_salary_deductions(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveSalaryDeductions,
) -> Result<crate::incomes::Income, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_salary_deductions(pool.inner(), input).await
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
            debt_account_id: None,
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
        heyday_core::database::MIGRATOR.run(pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000)").execute(pool).await.unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary','bank','salary',5000000,25),('other','Other','bank','other',5000000,25)").execute(pool).await.unwrap();
    }
    #[tokio::test]
    async fn link_student_loan_validate_archive_and_preserve_balances() {
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
        sqlx::query("INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('student','Student Loan','loan','student_loan',10000000,10000000)").execute(&pool).await.unwrap();
        let mut deduction = row(None, "Student Loan", "200000");
        deduction.debt_account_id = Some("student".into());
        save(&pool, input(vec![deduction])).await.unwrap();
        let saved: IncomeDeduction = sqlx::query_as(SELECT).fetch_one(&pool).await.unwrap();
        assert_eq!(saved.debt_account_id.as_deref(), Some("student"));
        assert_eq!(saved.debt_account_name.as_deref(), Some("Student Loan"));
        let balance: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id='student'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 10000000);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        sqlx::query("INSERT INTO planner_deduction_amounts VALUES(?,'2026-12',0)")
            .bind(&saved.id)
            .execute(&pool)
            .await
            .unwrap();
        for invalid in ["bank", "missing"] {
            let mut deduction = row(Some(&saved.id), "Student Loan", "100000");
            deduction.debt_account_id = Some(invalid.into());
            assert!(save(&pool, input(vec![deduction])).await.is_err());
        }
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='student'")
            .execute(&pool)
            .await
            .unwrap();
        let mut retained = row(Some(&saved.id), "Student Loan", "200000");
        retained.debt_account_id = Some("student".into());
        save(&pool, input(vec![retained])).await.unwrap();
        let mut new = row(None, "Another loan deduction", "200000");
        new.debt_account_id = Some("student".into());
        assert!(save(&pool, input(vec![new])).await.is_err());
        save(
            &pool,
            input(vec![row(Some(&saved.id), "Student Loan", "200000")]),
        )
        .await
        .unwrap();
        let unlinked: IncomeDeduction = sqlx::query_as(SELECT).fetch_one(&pool).await.unwrap();
        assert!(unlinked.debt_account_id.is_none());
        let amount: i64 =
            sqlx::query_scalar("SELECT amount FROM planner_deduction_amounts WHERE deduction_id=?")
                .bind(&saved.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 0);
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
        crate::test_support::remove_file(&path).await.unwrap();
    }
}
