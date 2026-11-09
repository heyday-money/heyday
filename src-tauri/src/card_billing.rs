//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::card_billing as service;
pub use heyday_core::services::card_billing::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_card_overview(pool: tauri::State<'_, SqlitePool>) -> Result<CardOverview, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_card_overview(pool.inner()).await
}

#[tauri::command]
pub async fn get_card_billing(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<AccountBilling, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_card_billing(pool.inner(), account_id).await
}

#[tauri::command]
pub async fn save_card_statement(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveStatement,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_card_statement(pool.inner(), input).await
}

#[tauri::command]
pub async fn save_card_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    input: SavePaymentPlan,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_card_payment_plan(pool.inner(), input).await
}

#[tauri::command]
pub async fn record_card_payment(
    pool: tauri::State<'_, SqlitePool>,
    input: PaymentInput,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::record_card_payment(pool.inner(), input).await
}

#[tauri::command]
pub async fn remove_card_billing_record(
    pool: tauri::State<'_, SqlitePool>,
    kind: String,
    id: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::remove_card_billing_record(pool.inner(), kind, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn pool() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        heyday_core::database::MIGRATOR.run(&p).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',100000,100000),('card','Card','credit_card',10000,10000),('other','Other','credit_card',0,0),('loan','Loan','loan',0,0);").execute(&p).await.unwrap();
        p
    }
    fn statement() -> SaveStatement {
        SaveStatement {
            id: None,
            account_id: "card".into(),
            start_date: "2024-01-01".into(),
            end_date: "2024-01-31".into(),
            due_date: "2024-02-10".into(),
            amount: "10000".into(),
            minimum: "1000".into(),
            currency: "THB".into(),
            installments: vec![],
        }
    }
    #[tokio::test]
    async fn overview_includes_archived_cards_and_both_transaction_sides_without_duplication() {
        let p=pool().await;
        sqlx::raw_sql("UPDATE accounts SET is_archived=1,current_balance=9007199254740993 WHERE id='other';
        INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('between','transfer','card','other',100,'2024-02-01','');
        INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('purchase','expense','card',250,'2024-01-10',''),('cash-only','expense','bank',123,'2024-01-10','');").execute(&p).await.unwrap();
        save_statement(&p,statement()).await.unwrap();
        let before=balances(&p).await;
        let data=overview_snapshot(&p).await.unwrap();
        assert_eq!(data.currency.as_deref(),Some("THB"));
        assert_eq!(data.archived_ids,vec!["other"]);
        assert_eq!(data.accounts.len(),2);
        assert_eq!(data.transactions.len(),2);
        assert_eq!(data.billing.statements.len(),1);
        let json=serde_json::to_value(&data).unwrap();
        assert!(json["accounts"].as_array().unwrap().iter().any(|a|a["id"]=="other"&&a["current_balance"]=="9007199254740993"));
        assert_eq!(balances(&p).await,before);
    }
    async fn id(p: &SqlitePool) -> String {
        sqlx::query_scalar("SELECT id FROM card_statements LIMIT 1")
            .fetch_one(p)
            .await
            .unwrap()
    }
    fn pay(s: String, n: &str) -> PaymentInput {
        PaymentInput {
            statement_id: s,
            transaction_id: None,
            account_id: "bank".into(),
            date: "2024-02-05".into(),
            amount: n.into(),
            currency: "THB".into(),
        }
    }
    async fn balances(p: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(p)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn billing_is_balance_neutral_and_payments_delete_atomically() {
        let p = pool().await;
        let before = balances(&p).await;
        save_statement(&p, statement()).await.unwrap();
        assert_eq!(balances(&p).await, before);
        // Read the complete account snapshot, including installment and transaction queries.
        assert_eq!(
            account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .statements
                .len(),
            1
        );
        let sid = id(&p).await;
        payment(&p, pay(sid.clone(), "1500")).await.unwrap();
        payment(&p, pay(sid.clone(), "2500")).await.unwrap();
        let mut conn = p.acquire().await.unwrap();
        let snap = snapshot(&mut conn).await.unwrap();
        assert_eq!(snap.allocations.len(), 2);
        assert_eq!(paid(&mut conn, &sid).await.unwrap(), 4000);
        drop(conn);
        let after = balances(&p).await;
        assert_eq!(after[0].1, 96000);
        assert_eq!(after[1].1, 6000);
        assert!(payment(&p, pay(sid.clone(), "7000")).await.is_err());
        assert_eq!(balances(&p).await, after);
        let transaction = snap.allocations[0].transaction_id.clone();
        crate::transactions::remove(&p, &transaction).await.unwrap();
        let mut conn = p.acquire().await.unwrap();
        let snap = snapshot(&mut conn).await.unwrap();
        assert_eq!(snap.allocations.len(), 1);
        assert!(!snap.statements[0].needs_review);
    }
    #[tokio::test]
    async fn validates_dates_currency_ownership_overlap_and_limits() {
        let p = pool().await;
        for field in ["date", "currency", "card", "amount", "minimum"] {
            let mut s = statement();
            match field {
                "date" => s.end_date = "2023-02-29".into(),
                "currency" => s.currency = "USD".into(),
                "card" => s.account_id = "bank".into(),
                "amount" => s.amount = "9223372036854775808".into(),
                _ => s.minimum = "10001".into(),
            };
            assert!(save_statement(&p, s).await.is_err())
        }
        save_statement(&p, statement()).await.unwrap();
        assert!(save_statement(&p, statement()).await.is_err());
        let sid = id(&p).await;
        let mut s = statement();
        s.id = Some(sid.clone());
        s.amount = "0".into();
        s.minimum = "0".into();
        save_statement(&p, s).await.unwrap();
        assert!(payment(&p, pay(sid, "1")).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn payment_links_are_unique_owned_and_neutral() {
        let p = pool().await;
        save_statement(&p, statement()).await.unwrap();
        let sid = id(&p).await;
        let mut wrong = pay(sid.clone(), "100");
        wrong.account_id = "loan".into();
        assert!(payment(&p, wrong).await.is_err());
        let mut early = pay(sid.clone(), "100");
        early.date = "2024-01-31".into();
        assert!(payment(&p, early).await.is_err());
        sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('existing','transfer','bank','card',1000,'2024-02-01','')").execute(&p).await.unwrap();
        let before = balances(&p).await;
        let mut link = pay(sid.clone(), "0");
        link.transaction_id = Some("existing".into());
        payment(&p, link).await.unwrap();
        assert_eq!(balances(&p).await, before);
        let mut link = pay(sid, "0");
        link.transaction_id = Some("existing".into());
        assert!(payment(&p, link).await.is_err());
    }
    #[tokio::test]
    async fn corrections_mark_history_review_and_keep_exact_large_amounts() {
        let p = pool().await;
        let mut s = statement();
        s.amount = "9007199254740993".into();
        save_statement(&p, s).await.unwrap();
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('purchase','expense','card',100,'2024-01-30','Groceries')").execute(&p).await.unwrap();
        let snap = account_snapshot(&p, "card").await.unwrap();
        assert!(snap.billing.statements[0].needs_review);
        assert_eq!(snap.billing.statements[0].amount, "9007199254740993");
        let mut s = statement();
        s.id = Some(id(&p).await);
        save_statement(&p, s).await.unwrap();
        assert!(
            !account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .statements[0]
                .needs_review
        );
        sqlx::query("DELETE FROM transactions WHERE id='purchase'")
            .execute(&p)
            .await
            .unwrap();
        assert!(
            account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .statements[0]
                .needs_review
        );
    }
    #[tokio::test]
    async fn plans_validate_and_retain_target_after_partial_payment() {
        let p = pool().await;
        save_statement(&p, statement()).await.unwrap();
        let sid = id(&p).await;
        let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
            .fetch_one(&p)
            .await
            .unwrap();
        save_plan(
            &p,
            SavePaymentPlan {
                statement_id: sid.clone(),
                account_id: "bank".into(),
                date: today,
                mode: "custom".into(),
                target: "4000".into(),
                currency: "THB".into(),
            },
        )
        .await
        .unwrap();
        payment(&p, pay(sid, "1500")).await.unwrap();
        let snap = account_snapshot(&p, "card").await.unwrap();
        assert_eq!(snap.billing.plans[0].target, "4000");
        assert_eq!(snap.billing.allocations[0].amount, "1500");
    }
    #[tokio::test]
    async fn installment_inclusions_validate_clamping_and_survive_reopening() {
        let p = pool().await;
        sqlx::query("INSERT INTO installments(id,name,account_id,debt_account_id,monthly_amount,installment_count,first_due_date) VALUES('i','Phone','bank','card',100,3,'2024-01-31')").execute(&p).await.unwrap();
        let mut s = statement();
        s.installments = vec![Coverage {
            installment_id: "i".into(),
            date: "2024-02-29".into(),
        }];
        save_statement(&p, s).await.unwrap();
        heyday_core::database::MIGRATOR.run(&p).await.unwrap();
        assert_eq!(
            account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .installments
                .len(),
            1
        );
        let mut s = statement();
        s.id = Some(id(&p).await);
        s.installments = vec![Coverage {
            installment_id: "i".into(),
            date: "2024-02-28".into(),
        }];
        assert!(save_statement(&p, s).await.is_err());
        assert_eq!(
            account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .installments[0]
                .date,
            "2024-02-29"
        );
    }
    #[tokio::test]
    async fn billing_persists_after_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "heyday-card-billing-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        heyday_core::database::MIGRATOR.run(&p).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',100000,100000),('card','Card','credit_card',10000,10000)").execute(&p).await.unwrap();
        save_statement(&p, statement()).await.unwrap();
        let sid = id(&p).await;
        payment(&p, pay(sid.clone(), "1000")).await.unwrap();
        p.close().await;
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        heyday_core::database::MIGRATOR.run(&p).await.unwrap();
        let snapshot = account_snapshot(&p, "card").await.unwrap();
        assert_eq!(snapshot.billing.statements[0].id, sid);
        assert_eq!(snapshot.billing.allocations[0].amount, "1000");
        assert_eq!(balances(&p).await[1].1, 9000);
        p.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
    #[tokio::test]
    async fn original_billing_migration_upgrades_without_rewriting_history() {
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let all = sqlx::migrate!("./tests/fixtures/alpha-migrations");
        let original = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                all.iter().filter(|m| m.version <= 27).cloned().collect(),
            ),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        original.run(&p).await.unwrap();
        let checksum: String = sqlx::query_scalar(
            "SELECT lower(hex(checksum)) FROM _sqlx_migrations WHERE version=27",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(checksum,"924ea9a26e98ebaddce033a3da103a689f32fae671ae13449f4f751db65ea20339fa8e846bcd0bad87d4fa1974f5439d");
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',100000,100000),('card','Card','credit_card',10000,10000)").execute(&p).await.unwrap();
        save_statement(&p, statement()).await.unwrap();
        let sid = id(&p).await;
        // Seed the legacy schema directly: current transaction APIs require migration 29.
        sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('legacy-payment','repayment','bank','card',1000,'2024-02-01','')").execute(&p).await.unwrap();
        sqlx::query("UPDATE accounts SET current_balance=current_balance-1000 WHERE id IN ('bank','card')").execute(&p).await.unwrap();
        sqlx::query("INSERT INTO card_payment_allocations(transaction_id,statement_id) VALUES('legacy-payment',?)").bind(&sid).execute(&p).await.unwrap();
        let before = balances(&p).await;
        all.run(&p).await.unwrap();
        all.run(&p).await.unwrap();
        assert_eq!(balances(&p).await, before);
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0002_salary_payments.sql")).execute(&p).await.unwrap();
        let saved = account_snapshot(&p, "card").await.unwrap();
        assert_eq!(saved.billing.statements[0].id, sid);
        assert_eq!(saved.billing.allocations[0].amount, "1000");
        let after: String = sqlx::query_scalar(
            "SELECT lower(hex(checksum)) FROM _sqlx_migrations WHERE version=27",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(checksum, after);
        // A change preceding the statement start must now mark its carried balance for review.
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('older','expense','card',50,'2023-12-01','')").execute(&p).await.unwrap();
        assert!(
            account_snapshot(&p, "card")
                .await
                .unwrap()
                .billing
                .statements[0]
                .needs_review
        );
        let trigger_count:i64=sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name LIKE 'card_statement_%'").fetch_one(&p).await.unwrap();
        assert_eq!(trigger_count, 4);
    }

}
