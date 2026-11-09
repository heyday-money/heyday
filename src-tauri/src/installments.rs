//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::installments as service;
pub use heyday_core::services::installments::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn save_installment(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveInstallment,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_installment(pool.inner(), input).await
}

#[tauri::command]
pub async fn delete_installment(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_installment(pool.inner(), id).await
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
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance, is_archived) VALUES ('bank', 'Bank', 'bank', 10000, 9000, 0), ('card', 'Card', 'credit_card', 5000, 4000, 0), ('loan', 'Loan', 'loan', 8000, 7000, 0), ('old', 'Old', 'loan', 6000, 6000, 1)").execute(pool).await.unwrap();
    }
    fn input() -> SaveInstallment {
        SaveInstallment {
            id: None,
            name: " Laptop ".into(),
            account_id: "bank".into(),
            debt_account_id: "card".into(),
            monthly_amount: "1000".into(),
            installment_count: 12,
            interest_rate_millis: Some("0".into()),
            first_due_date: "2024-01-31".into(),
            currency: "THB".into(),
            purchase: None,
        }
    }
    async fn records(pool: &SqlitePool) -> Vec<Installment> {
        sqlx::query_as(SELECT).fetch_all(pool).await.unwrap()
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id, opening_balance, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn editing_start_date_removes_only_invalid_bill_occurrences_atomically() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        seed(&pool).await;
        save(&pool, input()).await.unwrap();
        let id = records(&pool).await.remove(0).id;
        sqlx::query("INSERT INTO card_statements(id,account_id,start_date,end_date,due_date,amount,minimum) VALUES('s','card','2024-01-01','2024-01-31','2024-02-10',1000,100)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO card_payment_plans VALUES('s','bank','2024-02-10','full',1000)").execute(&pool).await.unwrap();
        for date in ["2024-01-31", "2024-02-29"] {
            sqlx::query("INSERT INTO card_statement_installments VALUES('s',?,?)").bind(&id).bind(date).execute(&pool).await.unwrap();
        }
        let before = balances(&pool).await;
        let mut edit = input(); edit.id = Some(id.clone()); edit.first_due_date = "2024-02-31".into();
        assert!(save(&pool, edit).await.is_err());
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM card_statement_installments").fetch_one(&pool).await.unwrap(),2);
        // March 31 changes the schedule while preserving an original-day month-end clamp.
        let mut edit = input(); edit.id = Some(id.clone()); edit.first_due_date = "2024-03-31".into();
        save(&pool,edit).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM card_statement_installments").fetch_one(&pool).await.unwrap(),0);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT needs_review FROM card_statements WHERE id='s'").fetch_one(&pool).await.unwrap(),1);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT target FROM card_payment_plans WHERE statement_id='s'").fetch_one(&pool).await.unwrap(),1000);
        assert_eq!(balances(&pool).await,before);
        sqlx::query("INSERT INTO card_statement_installments VALUES('s',?,'2024-04-30')").bind(&id).execute(&pool).await.unwrap();
        let mut edit = input(); edit.id = Some(id); edit.first_due_date = "2024-03-31".into(); edit.name = "Renamed".into();
        save(&pool,edit).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_,String>("SELECT date FROM card_statement_installments").fetch_one(&pool).await.unwrap(),"2024-04-30");
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
    async fn schedules_persist_without_ledger_effects() {
        let path = std::env::temp_dir().join(format!(
            "heyday-installments-{}-{}.db",
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
        let card = records(&pool).await.remove(0);
        assert_eq!(card.name, "Laptop");
        assert_eq!(card.interest_rate_millis.as_deref(), Some("0"));
        assert_eq!(card.debt_account_name, "Card");
        let mut second = input();
        second.interest_rate_millis = Some("1195".into());
        second.monthly_amount = "9007199254740993".into();
        save(&pool, second).await.unwrap();
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        heyday_core::database::MIGRATOR.run(&reopened).await.unwrap();
        assert_eq!(records(&reopened).await.len(), 2);
        assert!(records(&reopened)
            .await
            .iter()
            .any(|row| row.monthly_amount == "9007199254740993"
                && row.debt_account_name == "Card"
                && row.interest_rate_millis.as_deref() == Some("1195")));
        let mut edit = input();
        edit.id = Some(card.id.clone());
        edit.monthly_amount = "2500".into();
        edit.installment_count = 6;
        edit.interest_rate_millis = Some("12345".into());
        save(&reopened, edit).await.unwrap();
        assert!(records(&reopened).await.iter().any(|row| row.id == card.id
            && row.monthly_amount == "2500"
            && row.installment_count == 6
            && row.interest_rate_millis.as_deref() == Some("12345")));
        remove(&reopened, &card.id).await.unwrap();
        assert!(remove(&reopened, &card.id).await.is_err());
        assert_eq!(balances(&reopened).await, before);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&reopened)
            .await
            .unwrap();
        assert_eq!(count, 0);
        reopened.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
    #[tokio::test]
    async fn new_purchase_increases_debt_once_and_schedule_edits_never_rebook_it() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        sqlx::query("UPDATE accounts SET current_balance = 1000000, credit_limit = 10000000 WHERE id = 'card'").execute(&pool).await.unwrap();
        let mut purchase = input();
        purchase.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        save(&pool, purchase).await.unwrap();
        let row = records(&pool).await.remove(0);
        assert_eq!(row.purchase_kind, "new_purchase");
        let transaction_id = row.purchase_transaction_id.clone().unwrap();
        let card: (i64, i64) = sqlx::query_as("SELECT current_balance, credit_limit - current_balance FROM accounts WHERE id = 'card'").fetch_one(&pool).await.unwrap();
        assert_eq!(card, (3000000, 7000000));
        let expense: (String, String, i64) =
            sqlx::query_as("SELECT type, account_id, amount FROM transactions WHERE id = ?")
                .bind(&transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(expense, ("expense".into(), "card".into(), 2000000));
        let mut edit = input();
        edit.id = Some(row.id.clone());
        edit.monthly_amount = "5000".into();
        save(&pool, edit).await.unwrap();
        let mut rebook = input();
        rebook.id = Some(row.id.clone());
        rebook.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        assert!(save(&pool, rebook).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let updated: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(updated, 3000000);
        crate::transactions::remove(&pool, &transaction_id)
            .await
            .unwrap();
        let retained = records(&pool).await.remove(0);
        assert_eq!(retained.purchase_kind, "new_purchase");
        assert_eq!(retained.purchase_transaction_id, None);
        let reversed: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reversed, 1000000);
        remove(&pool, &row.id).await.unwrap();
        let mut another = input();
        another.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        save(&pool, another).await.unwrap();
        remove(&pool, &records(&pool).await[0].id).await.unwrap();
        let preserved: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(preserved, 3000000);
    }

    #[tokio::test]
    async fn purchase_and_schedule_fail_atomically() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        for (amount, date) in [
            ("0", "2024-01-01"),
            ("100", "2999-01-01"),
            ("100", "2024-02-30"),
            ("9223372036854775807", "2024-01-01"),
        ] {
            let mut value = input();
            value.purchase = Some(Purchase {
                amount: amount.into(),
                date: date.into(),
            });
            assert!(save(&pool, value).await.is_err());
        }
        sqlx::raw_sql("CREATE TRIGGER fail_schedule BEFORE INSERT ON installments BEGIN SELECT RAISE(ABORT, 'Schedule failed'); END;").execute(&pool).await.unwrap();
        let mut value = input();
        value.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-01".into(),
        });
        assert!(save(&pool, value)
            .await
            .unwrap_err()
            .contains("Schedule failed"));
        assert_eq!(balances(&pool).await, before);
        assert!(records(&pool).await.is_empty());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn legacy_loan_schedules_remain_readable_but_cannot_be_rewritten() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date) VALUES ('legacy', 'Car payments', 'bank', 'loan', 1000, 12, '2026-01-31')").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let legacy = records(&pool).await.remove(0);
        assert_eq!(legacy.debt_account_type, "loan");
        assert_eq!(legacy.monthly_amount, "1000");
        let mut edit = input();
        edit.id = Some(legacy.id.clone());
        assert!(save(&pool, edit)
            .await
            .unwrap_err()
            .contains("Existing loan schedules"));
        assert_eq!(records(&pool).await[0].debt_account_id, "loan");
        remove(&pool, &legacy.id).await.unwrap();
        assert_eq!(balances(&pool).await, before);
    }

    #[tokio::test]
    async fn interest_migration_preserves_existing_schedule_and_unknown_rate() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in [
            include_str!("../tests/fixtures/alpha-migrations/0001_initial.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0002_account_details.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0003_transactions.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0004_optional_transaction_description.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0008_installments.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Bank', 'bank', 10000), ('card', 'Card', 'credit_card', 5000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date) VALUES ('existing', 'Laptop', 'bank', 'card', 1000, 12, '2024-01-31')").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0009_installment_interest.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0010_installment_purchases.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET interest_rate_bps = 475 WHERE id = 'card'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date, interest_rate_bps) VALUES ('rated', 'Rated', 'bank', 'card', 1000, 12, '2025-01-31', 750), ('zero', 'Zero', 'bank', 'card', 1000, 12, '2025-01-31', 0)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0019_interest_precision.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let rate: i64 =
            sqlx::query_scalar("SELECT interest_rate_millis FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(rate, 4750);
        let rates: Vec<i64> = sqlx::query_scalar("SELECT interest_rate_millis FROM installments WHERE interest_rate_millis IS NOT NULL ORDER BY interest_rate_millis").fetch_all(&pool).await.unwrap();
        assert_eq!(rates, vec![0, 7500]);

        let row = records(&pool).await.remove(0);
        assert_eq!(row.interest_rate_millis, None);
        assert_eq!(row.monthly_amount, "1000");
        assert_eq!(row.installment_count, 12);
        assert_eq!(row.first_due_date, "2024-01-31");
        assert_eq!(row.debt_account_id, "card");
        assert!(
            sqlx::query("UPDATE installments SET interest_rate_millis = -1")
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("UPDATE installments SET interest_rate_millis = 1.5")
                .execute(&pool)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn invalid_and_stale_schedules_do_not_write() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let mut cases = vec![];
        for rate in ["-1", "1.5", "bad", "9223372036854775808"] {
            let mut value = input();
            value.interest_rate_millis = Some(rate.into());
            cases.push(value);
        }
        for amount in [
            "0",
            "-1",
            "1.2",
            "9223372036854775808",
            "9223372036854775807",
        ] {
            let mut value = input();
            value.monthly_amount = amount.into();
            cases.push(value);
        }
        for count in [0, -1, 601] {
            let mut value = input();
            value.installment_count = count;
            cases.push(value);
        }
        for date in [
            "2023-02-29",
            "2024-02-30",
            "2024-13-01",
            "0000-01-01",
            "9999-12-31",
            "bad",
        ] {
            let mut value = input();
            value.first_due_date = date.into();
            cases.push(value);
        }
        for id in ["bank", "loan", "missing", "old"] {
            let mut value = input();
            value.debt_account_id = id.into();
            cases.push(value);
        }
        for id in ["card", "missing", "old"] {
            let mut value = input();
            value.account_id = id.into();
            cases.push(value);
        }
        for name in ["".to_owned(), " ".into(), "x".repeat(101)] {
            let mut value = input();
            value.name = name;
            cases.push(value);
        }
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
        let mut value = input();
        value.first_due_date = "2024-02-29".into();
        value.interest_rate_millis = None;
        save(&pool, value).await.unwrap();
        let mut value = input();
        value.first_due_date = "9999-12-31".into();
        value.installment_count = 1;
        value.monthly_amount = "9223372036854775807".into();
        save(&pool, value).await.unwrap();
        assert_eq!(records(&pool).await.len(), 2);
    }
}
