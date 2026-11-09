//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::salary_payments as service;
pub use heyday_core::services::salary_payments::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn record_salary_payment(
    pool: tauri::State<'_, SqlitePool>,
    input: PaymentInput,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::record_salary_payment(pool.inner(), input).await
}

#[tauri::command]
pub async fn list_salary_payments(
    pool: tauri::State<'_, SqlitePool>,
    income_id: Option<String>,
    payment_id: Option<String>,
    account_id: Option<String>,
) -> Result<Vec<Payment>, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_salary_payments(pool.inner(), income_id, payment_id, account_id).await
}

#[tauri::command]
pub async fn delete_salary_payment(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
    confirmed: bool,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_salary_payment(pool.inner(), id, confirmed).await
}
#[cfg(test)]
pub async fn record(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    service::record(pool, input).await
}
#[cfg(test)]
pub async fn reverse(pool: &SqlitePool, id: &str, confirmed: bool) -> Result<(), String> {
    service::reverse(pool, id, confirmed).await
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    pub(crate) async fn database() -> SqlitePool {
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
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000),('home','Home loan','loan',2000000,2000000),('student','Education loan','loan',1000000,1000000); INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary','bank','salary',5000000,25); INSERT INTO income_deductions(id,income_id,name,amount,debt_account_id) VALUES('home','salary','Home',920000,'home'),('student','salary','Education',126000,'student'),('tax','salary','Tax',100000,NULL);").execute(&pool).await.unwrap();
        pool
    }
    pub(crate) fn input() -> PaymentInput {
        serde_json::from_value(serde_json::json!({"income_id":"salary","destination_account_id":"bank","occurrence":"2024-01","date":"2024-01-25","gross":"5000000","currency":"THB","confirmed":true,"deductions":[
            {"id":"home","debt_account_id":"home","amount":"920000","interest":"200000","fee":"20000","contract_id":null},
            {"id":"student","debt_account_id":"student","amount":"126000","interest":"0","fee":"0","contract_id":null},
            {"id":"tax","debt_account_id":null,"amount":"100000","interest":"0","fee":"0","contract_id":null}
        ]})).unwrap()
    }
    async fn balances(p: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(p)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn payroll_records_net_once_and_principal_only_then_reverses_whole_group() {
        let p = database().await;
        let before = balances(&p).await;
        record(&p, input()).await.unwrap();
        assert_eq!(
            balances(&p).await,
            vec![
                ("bank".into(), 3864000),
                ("home".into(), 1300000),
                ("student".into(), 874000)
            ]
        );
        let transactions: Vec<crate::transactions::Transaction> =
            sqlx::query_as(crate::transactions::SELECT)
                .fetch_all(&p)
                .await
                .unwrap();
        assert_eq!(transactions.len(), 3);
        let saved: String = sqlx::query_scalar("SELECT breakdown FROM salary_payments")
            .fetch_one(&p)
            .await
            .unwrap();
        let parts: Vec<Breakdown> = serde_json::from_str(&saved).unwrap();
        assert_eq!(parts[0].principal, "700000");
        assert_eq!(parts[0].values.interest, "200000");
        sqlx::query("UPDATE income_deductions SET amount=1,name='Changed'")
            .execute(&p)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT breakdown FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            saved
        );
        assert!(record(&p, input())
            .await
            .unwrap_err()
            .contains("already exists"));
        let principal: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='principal' LIMIT 1",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        crate::transactions::remove(&p, &principal).await.unwrap();
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        record(&p, input()).await.unwrap();
    }
    #[tokio::test]
    async fn invalid_or_stale_payroll_rolls_back_all_balances_and_history() {
        let p = database().await;
        let before = balances(&p).await;
        for case in 0..10 {
            let mut value = input();
            match case {
                0 => value.confirmed = false,
                1 => value.gross = "1".into(),
                2 => value.deductions[0].interest = "920001".into(),
                3 => value.deductions[1].amount = "999999999".into(),
                4 => value.deductions[0].debt_account_id = Some("student".into()),
                5 => value.deductions[0].amount = "9223372036854775808".into(),
                6 => value.date = "2024-02-30".into(),
                7 => value.currency = "USD".into(),
                8 => value.deductions[1] = value.deductions[0].clone(),
                _ => value.deductions[0].contract_id = Some("missing".into()),
            }
            assert!(record(&p, value).await.is_err(), "case {case}");
            assert_eq!(balances(&p).await, before);
        }
        sqlx::query("CREATE TRIGGER fail_payroll BEFORE INSERT ON salary_payment_transactions WHEN NEW.role='principal' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&p).await.unwrap();
        assert!(record(&p, input()).await.is_err());
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn existing_receipt_blocks_payroll_and_zero_net_does_not_create_bank_income() {
        let p = database().await;
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description,income_source_id) VALUES('old','income','bank',1,'2024-01-24','','salary')").execute(&p).await.unwrap();
        assert!(record(&p, input())
            .await
            .unwrap_err()
            .contains("already exists"));
        sqlx::query("DELETE FROM transactions WHERE id='old'")
            .execute(&p)
            .await
            .unwrap();
        let mut value = input();
        value.gross = "1146000".into();
        record(&p, value).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&p)
                .await
                .unwrap(),
            10000
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT net FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn contract_principal_and_exact_large_salary_survive_reversal() {
        let p = database().await;
        sqlx::raw_sql("UPDATE accounts SET loan_type='personal_loan' WHERE id='home'; INSERT INTO loan_facilities VALUES('home',3000000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','home','Contract',2000000,2000000,'2023-01-01','bank','bank',0,920000,12,'2024-01-25','existing');").execute(&p).await.unwrap();
        let mut value = input();
        value.gross = "9007199254740993".into();
        value.deductions[0].contract_id = Some("contract".into());
        record(&p, value).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&p)
                .await
                .unwrap(),
            9007199253604993
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
                .fetch_one(&p)
                .await
                .unwrap(),
            1300000
        );
        let id: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='net'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        crate::transactions::remove(&p, &id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
                .fetch_one(&p)
                .await
                .unwrap(),
            2000000
        );
    }

    #[tokio::test]
    async fn interest_only_zero_net_payment_remains_in_loan_history_and_can_be_reversed() {
        let p = database().await;
        let before = balances(&p).await;
        let mut value = input();
        value.gross = "1146000".into();
        value.deductions[0].interest = "920000".into();
        value.deductions[0].fee = "0".into();
        value.deductions[1].interest = "126000".into();
        record(&p, value).await.unwrap();
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        let saved = history(&p, None, None, Some("home".into())).await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].net, "0");
        assert_eq!(
            history(&p, None, None, Some("unrelated".into()))
                .await
                .unwrap()
                .len(),
            0
        );
        assert!(reverse(&p, &saved[0].id, false).await.is_err());
        reverse(&p, &saved[0].id, true).await.unwrap();
        assert!(history(&p, None, None, None).await.unwrap().is_empty());
    }
}
