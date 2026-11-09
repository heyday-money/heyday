//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::loans as service;
pub use heyday_core::services::loans::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_loan_account(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<Snapshot, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_loan_account(pool.inner(), account_id).await
}

#[tauri::command]
pub async fn save_loan_facility(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
    credit_limit: String,
    currency_code: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_loan_facility(pool.inner(), account_id, credit_limit, currency_code).await
}

#[tauri::command]
pub async fn save_loan_contract(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveContract,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_loan_contract(pool.inner(), input).await
}

#[tauri::command]
pub async fn record_loan_repayment(
    pool: tauri::State<'_, SqlitePool>,
    input: Repayment,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::record_loan_repayment(pool.inner(), input).await
}

#[tauri::command]
pub async fn delete_loan_contract(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_loan_contract(pool.inner(), id).await
}

#[cfg(test)]
mod tests {
    use crate::transactions::NewTransaction;
    use super::*;
    use serde_json::json;
    async fn database() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Cash card','loan','personal_loan',0,0),('bank','Bank','bank',NULL,5000000,5000000); INSERT INTO loan_facilities VALUES('loan',10000000);").execute(&pool).await.unwrap();
        pool
    }
    fn input() -> SaveContract {
        serde_json::from_value(json!({"id":null,"account_id":"loan","name":"Borrowing 1","principal":"2000000","remaining_principal":"2000000","borrowing_date":"2024-01-31","receiving_account_id":"bank","payment_account_id":"bank","interest_rate":75000,"monthly_amount":"210000","installment_count":10,"first_due_date":"2024-02-29","schedule_end":null,"notes":"","borrowing_kind":"new","currency":"THB"})).unwrap()
    }
    fn payment(id: &str) -> Repayment {
        serde_json::from_value(json!({"contract_id":id,"account_id":"bank","date":"2024-03-01","total":"210000","principal":"200000","interest":"9000","fee":"1000","currency":"THB"})).unwrap()
    }
    fn standalone_payment() -> Repayment {
        serde_json::from_value(json!({"contract_id":null,"loan_account_id":"loan","account_id":"bank","date":"2024-03-01","total":"500000","principal":"400000","interest":"80000","fee":"20000","currency":"THB","description":"Monthly payment","cleared":true})).unwrap()
    }
    #[tokio::test]
    async fn repayment_migration_preserves_existing_contract_parts() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations").iter().filter(|m| m.version < 38) {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,current_balance) VALUES('loan','Loan','loan','personal_loan',2000000),('bank','Bank','bank',NULL,5000000); INSERT INTO loan_facilities VALUES('loan',3000000);").execute(&pool).await.unwrap();
        let mut draft = input();
        draft.borrowing_kind = "existing".into();
        save(&pool,draft).await.unwrap();
        let contract = rows(&pool).await.remove(0);
        sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('old','repayment','bank','loan',200000,'2024-03-01','Existing payment')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO loan_payment_parts VALUES('old','payment',?,'principal',200000)").bind(&contract.id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE loan_contracts SET remaining_principal=1800000").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
        assert_eq!(balances(&pool).await,before);
        let part: (String,Option<String>,Option<String>) = sqlx::query_as("SELECT transaction_id,contract_id,loan_account_id FROM loan_payment_parts").fetch_one(&pool).await.unwrap();
        assert_eq!(part,("old".into(),Some(contract.id.clone()),None));
        assert_eq!(rows(&pool).await[0].remaining_principal,"1800000");
        crate::transactions::remove(&pool,"old").await.unwrap();
        assert_eq!(rows(&pool).await[0].remaining_principal,"2000000");
    }
    #[tokio::test]
    async fn repayment_history_resolves_the_same_loan_for_every_component() {
        for standalone in [true, false] {
            let pool = database().await;
            if standalone {
                sqlx::query("UPDATE accounts SET current_balance=1000000 WHERE id='loan'").execute(&pool).await.unwrap();
                repay(&pool, standalone_payment()).await.unwrap();
            } else {
                save(&pool, input()).await.unwrap();
                let contract = rows(&pool).await.remove(0);
                repay(&pool, payment(&contract.id)).await.unwrap();
            }
            // Resolve existing records from their stable loan links, including archived/renamed loans.
            sqlx::query("UPDATE accounts SET name='Sample',last_four='2342',is_archived=1 WHERE id='loan'").execute(&pool).await.unwrap();
            let before = balances(&pool).await;
            let history: Vec<crate::transactions::Transaction> = sqlx::query_as(&format!("{} WHERE lp.transaction_id IS NOT NULL", crate::transactions::SELECT)).fetch_all(&pool).await.unwrap();
            assert_eq!(history.len(), 3);
            for row in history {
                let row = serde_json::to_value(row).unwrap();
                assert_eq!(row["loan_account_id"], "loan");
                assert_eq!(row["loan_account_name"], "Sample");
                assert_eq!(row["account_id"], "bank");
                assert!(row["payee_id"].is_null());
                if row["type"] == "expense" {
                    assert_eq!(row["category_name"], "Fee/Interest");
                    assert!(row["destination_account_id"].is_null());
                } else {
                    assert_eq!(row["destination_account_id"], "loan");
                }
            }
            assert_eq!(balances(&pool).await, before);
        }
    }
    #[tokio::test]
    async fn standalone_split_reduces_only_principal_and_reverses_as_a_group() {
        let pool = database().await;
        sqlx::raw_sql("DELETE FROM loan_facilities; UPDATE accounts SET loan_type='mortgage',current_balance=1000000 WHERE id='loan';").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        repay(&pool, standalone_payment()).await.unwrap();
        assert_eq!(balances(&pool).await, vec![("bank".into(),4500000),("loan".into(),600000)]);
        let parts: Vec<(String, Option<String>, String)> = sqlx::query_as("SELECT component,contract_id,loan_account_id FROM loan_payment_parts ORDER BY component").fetch_all(&pool).await.unwrap();
        assert_eq!(parts, vec![("fee".into(),None,"loan".into()),("interest".into(),None,"loan".into()),("principal".into(),None,"loan".into())]);
        let cleared: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transaction_verifications WHERE account_id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(cleared,3);
        let id: String = sqlx::query_scalar("SELECT transaction_id FROM loan_payment_parts WHERE component='interest'").fetch_one(&pool).await.unwrap();
        crate::transactions::remove_confirmed(&pool,&id,true).await.unwrap();
        assert_eq!(balances(&pool).await,before);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loan_payment_parts").fetch_one(&pool).await.unwrap();
        assert_eq!(count,0);
    }
    #[tokio::test]
    async fn standalone_split_validates_and_rolls_back_every_component() {
        let pool = database().await;
        sqlx::query("UPDATE accounts SET current_balance=1000000 WHERE id='loan'").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        for (field,value) in [("interest","-1"),("total","0"),("total","499999"),("total","9223372036854775808"),("principal","1000001"),("currency","USD"),("date","9999-01-01"),("account_id","loan")] {
            let mut input = json!({"contract_id":null,"loan_account_id":"loan","account_id":"bank","date":"2024-03-01","total":"500000","principal":"400000","interest":"80000","fee":"20000","currency":"THB"});
            input[field]=json!(value);
            assert!(repay(&pool,serde_json::from_value(input).unwrap()).await.is_err(),"{field}");
            assert_eq!(balances(&pool).await,before);
        }
        sqlx::query("CREATE TRIGGER reject_fee BEFORE INSERT ON loan_payment_parts WHEN NEW.component='fee' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();
        assert!(repay(&pool,standalone_payment()).await.is_err());
        assert_eq!(balances(&pool).await,before);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions").fetch_one(&pool).await.unwrap();
        assert_eq!(count,0);
        sqlx::query("DROP TRIGGER reject_fee").execute(&pool).await.unwrap();
        save(&pool,input()).await.unwrap();
        assert!(repay(&pool,standalone_payment()).await.err().unwrap().contains("Choose a contract"));
        let mut wrong = payment(&rows(&pool).await[0].id);
        wrong.loan_account_id=Some("bank".into());
        assert!(repay(&pool,wrong).await.err().unwrap().contains("belonging"));
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    async fn rows(pool: &SqlitePool) -> Vec<Contract> {
        contracts(&mut *pool.acquire().await.unwrap())
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn fee_interest_backfill_preserves_ledger_and_user_categories() {
        let pool = database().await;
        // Repayment must also recreate the category after Clear all data.
        sqlx::query("DELETE FROM categories WHERE name_key='fee/interest'")
            .execute(&pool).await.unwrap();
        save(&pool, input()).await.unwrap();
        let contract = rows(&pool).await.remove(0);
        repay(&pool, payment(&contract.id)).await.unwrap();
        let before = balances(&pool).await;
        sqlx::raw_sql("INSERT INTO categories(id,name,name_key,icon) VALUES('custom','Custom','custom','tag'); UPDATE transactions SET category_id='custom' WHERE id IN (SELECT transaction_id FROM loan_payment_parts WHERE component='fee'); UPDATE transactions SET category_id=NULL WHERE id IN (SELECT transaction_id FROM loan_payment_parts WHERE component='interest'); UPDATE categories SET name='Fee/Interest',icon='percent' WHERE name_key='fee/interest'; INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('unrelated','expense','bank',123,'2024-03-01','interest');")
            .execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0037_loan_fee_interest_category.sql"))
            .execute(&pool).await.unwrap();
        let categories: Vec<(String, Option<String>)> = sqlx::query_as("SELECT p.component,c.name FROM loan_payment_parts p JOIN transactions t ON t.id=p.transaction_id LEFT JOIN categories c ON c.id=t.category_id ORDER BY p.component")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(categories, vec![("fee".into(),Some("Custom".into())),("interest".into(),Some("Fee/Interest".into())),("principal".into(),None)]);
        let unrelated: Option<String> = sqlx::query_scalar("SELECT category_id FROM transactions WHERE id='unrelated'").fetch_one(&pool).await.unwrap();
        assert_eq!(unrelated, None);
        let icon: String = sqlx::query_scalar("SELECT icon FROM categories WHERE name_key='fee/interest'").fetch_one(&pool).await.unwrap();
        assert_eq!(icon, "percent");
        assert_eq!(balances(&pool).await, before);
        assert_eq!(rows(&pool).await[0].remaining_principal, "1800000");
    }

    #[tokio::test]
    async fn multiple_draws_split_repayment_and_group_reversal() {
        let pool = database().await;
        save(&pool, input()).await.unwrap();
        let mut second = input();
        second.name = "Borrowing 2".into();
        second.principal = "1000000".into();
        second.remaining_principal = "1000000".into();
        second.interest_rate = 50000;
        second.installment_count = 6;
        save(&pool, second).await.unwrap();
        assert_eq!(
            balances(&pool).await,
            vec![("bank".into(), 8000000), ("loan".into(), 3000000)]
        );
        let c = rows(&pool).await.remove(0);
        repay(&pool, payment(&c.id)).await.unwrap();
        assert_eq!(
            balances(&pool).await,
            vec![("bank".into(), 7790000), ("loan".into(), 2800000)]
        );
        assert!(
            crate::transactions::remove(&pool, c.borrowing_transaction_id.as_ref().unwrap())
                .await
                .is_err()
        );
        let parts: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT p.component,c.name FROM loan_payment_parts p JOIN transactions t ON t.id=p.transaction_id LEFT JOIN categories c ON c.id=t.category_id ORDER BY p.component",
        ).fetch_all(&pool).await.unwrap();
        assert_eq!(parts, vec![
            ("fee".into(), Some("Fee/Interest".into())),
            ("interest".into(), Some("Fee/Interest".into())),
            ("principal".into(), None),
        ]);
        let fee: String = sqlx::query_scalar(
            "SELECT transaction_id FROM loan_payment_parts WHERE component='fee'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        crate::transactions::remove(&pool, &fee).await.unwrap();
        assert_eq!(
            balances(&pool).await,
            vec![("bank".into(), 8000000), ("loan".into(), 3000000)]
        );
        assert_eq!(
            rows(&pool)
                .await
                .into_iter()
                .find(|x| x.id == c.id)
                .unwrap()
                .remaining_principal,
            c.principal
        );
        crate::transactions::remove(&pool, c.borrowing_transaction_id.as_ref().unwrap())
            .await
            .unwrap();
        let c = rows(&pool)
            .await
            .into_iter()
            .find(|x| x.id == c.id)
            .unwrap();
        assert!(c.needs_review);
        assert_eq!(c.remaining_principal, "0");
        assert!(c.borrowing_transaction_id.is_none());
    }
    #[tokio::test]
    async fn existing_allocation_is_exact_and_never_duplicates_debt() {
        let pool = database().await;
        sqlx::query("UPDATE accounts SET current_balance=9007199254740993 WHERE id='loan'")
            .execute(&pool)
            .await
            .unwrap();
        let mut i = input();
        i.borrowing_kind = "existing".into();
        i.principal = "9007199254740993".into();
        i.remaining_principal = i.principal.clone();
        save(&pool, i).await.unwrap();
        let before = balances(&pool).await;
        let mut duplicate = input();
        duplicate.borrowing_kind = "existing".into();
        assert!(save(&pool, duplicate).await.is_err());
        assert_eq!(rows(&pool).await[0].remaining_principal, "9007199254740993");
        assert_eq!(balances(&pool).await, before);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let c = rows(&pool).await.remove(0);
        let mut edit = input();
        edit.id = Some(c.id);
        edit.borrowing_kind = "existing".into();
        edit.principal = c.principal;
        edit.remaining_principal = c.remaining_principal;
        edit.monthly_amount = "12345".into();
        edit.interest_rate = 11957;
        save(&pool, edit).await.unwrap();
        assert_eq!(balances(&pool).await, before);
        assert_eq!(rows(&pool).await[0].interest_rate, 11957);
    }
    #[tokio::test]
    async fn failures_roll_back_draws_payments_and_deletions() {
        let pool = database().await;
        let initial = balances(&pool).await;
        sqlx::query("CREATE TRIGGER fail_contract BEFORE INSERT ON loan_contracts BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();
        assert!(save(&pool, input()).await.is_err());
        assert_eq!(balances(&pool).await, initial);
        sqlx::query("DROP TRIGGER fail_contract")
            .execute(&pool)
            .await
            .unwrap();
        save(&pool, input()).await.unwrap();
        let c = rows(&pool).await.remove(0);
        let before = balances(&pool).await;
        sqlx::query("CREATE TRIGGER fail_expense BEFORE INSERT ON transactions WHEN NEW.type='expense' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();
        assert!(repay(&pool, payment(&c.id)).await.is_err());
        assert_eq!(balances(&pool).await, before);
        assert_eq!(rows(&pool).await[0].remaining_principal, "2000000");
        sqlx::query("DROP TRIGGER fail_expense")
            .execute(&pool)
            .await
            .unwrap();
        repay(&pool, payment(&c.id)).await.unwrap();
        let paid = balances(&pool).await;
        let id: String = sqlx::query_scalar(
            "SELECT transaction_id FROM loan_payment_parts WHERE component='principal'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TRIGGER fail_delete BEFORE DELETE ON transactions WHEN OLD.type='expense' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();
        assert!(crate::transactions::remove(&pool, &id).await.is_err());
        assert_eq!(balances(&pool).await, paid);
        assert_eq!(rows(&pool).await[0].remaining_principal, "1800000");
    }
    #[tokio::test]
    async fn validation_and_overlimit_acknowledgement() {
        let pool = database().await;
        for field in [
            "date",
            "rate",
            "term",
            "currency",
            "account",
            "amount",
            "remaining",
        ] {
            let mut i = input();
            match field {
                "date" => i.first_due_date = "2024-02-30".into(),
                "rate" => i.interest_rate = 1000001,
                "term" => {
                    i.first_due_date = "9999-12-31".into();
                    i.installment_count = 2
                }
                "currency" => i.currency = "USD".into(),
                "account" => i.receiving_account_id = "loan".into(),
                "amount" => i.principal = "9223372036854775808".into(),
                _ => i.remaining_principal = "1".into(),
            };
            assert!(save(&pool, i).await.is_err(), "{field}");
        }
        let mut i = input();
        i.principal = "11000000".into();
        i.remaining_principal = i.principal.clone();
        assert!(save(&pool, i).await.is_err());
        let mut i = input();
        i.principal = "11000000".into();
        i.remaining_principal = i.principal.clone();
        i.acknowledge_over_limit = true;
        save(&pool, i).await.unwrap();
        let c = rows(&pool).await.remove(0);
        let mut p = payment(&c.id);
        p.total = "1".into();
        assert!(repay(&pool, p).await.is_err());
        let mut p = payment(&c.id);
        p.principal = "12000000".into();
        p.total = "12010000".into();
        assert!(repay(&pool, p).await.is_err());
        assert!(
            sqlx::query("UPDATE accounts SET loan_type='mortgage' WHERE id='loan'")
                .execute(&pool)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn migration_and_restart_preserve_records_and_overrides() {
        let path = std::env::temp_dir().join(format!(
            "heyday-loans-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
        for m in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 25)
        {
            sqlx::raw_sql(&m.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Loan','loan','personal_loan',3000000,3000000),('bank','Bank','bank',NULL,0,0); INSERT INTO planner_debt_amounts VALUES('loan','2024-02',0);").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO loan_facilities VALUES('loan',10000000)")
            .execute(&pool)
            .await
            .unwrap();
        let mut i = input();
        i.borrowing_kind = "existing".into();
        save(&pool, i).await.unwrap();
        pool.close().await;
        let pool = SqlitePool::connect_with(options).await.unwrap();
        assert_eq!(rows(&pool).await[0].remaining_principal, "2000000");
        assert_eq!(
            balances(&pool).await,
            vec![("bank".into(), 0), ("loan".into(), 3000000)]
        );
        let amount: i64 =
            sqlx::query_scalar("SELECT amount FROM planner_debt_amounts WHERE account_id='loan'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(amount, 0);
        pool.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
    #[tokio::test]
    async fn generic_repayments_cannot_consume_allocated_principal_and_group_history_is_protected()
    {
        let pool = database().await;
        save(&pool, input()).await.unwrap();
        let c = rows(&pool).await.remove(0);
        let before = balances(&pool).await;
        let generic = NewTransaction {
            kind: "repayment".into(),
            account_id: "bank".into(),
            destination_account_id: Some("loan".into()),
            amount: "1".into(),
            date: "2024-03-01".into(),
            description: "Unallocated repayment".into(),
            currency: "THB".into(),
            payee_id: None,
            category_id: None,
            income_source_id: None,
            cleared_account_ids: vec![],
        };
        assert!(crate::transactions::insert(&pool, generic).await.is_err());
        assert_eq!(balances(&pool).await, before);
        repay(&pool, payment(&c.id)).await.unwrap();
        let paid = balances(&pool).await;
        let fee: String = sqlx::query_scalar(
            "SELECT transaction_id FROM loan_payment_parts WHERE component='fee'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let principal: String = sqlx::query_scalar(
            "SELECT transaction_id FROM loan_payment_parts WHERE component='principal'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO reconciliations(id,account_id,confirmed_balance,opening_balance) VALUES(1,'bank',6790000,5000000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO reconciliation_entries(reconciliation_id,transaction_id,date,description,type,balance_change) VALUES(1,?,'2024-03-01','Fee','expense',-1000)").bind(&fee).execute(&pool).await.unwrap();
        assert!(crate::transactions::remove(&pool, &principal)
            .await
            .unwrap_err()
            .contains("reconciliation history"));
        assert_eq!(balances(&pool).await, paid);
        assert_eq!(rows(&pool).await[0].remaining_principal, "1800000");
        crate::transactions::remove_confirmed(&pool, &principal, true)
            .await
            .unwrap();
        assert_eq!(balances(&pool).await, before);
        let review: bool =
            sqlx::query_scalar("SELECT needs_review FROM reconciliations WHERE id=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(review);
    }
    #[tokio::test]
    async fn general_payments_cannot_bypass_contract_split_with_unassigned_debt() {
        let pool = database().await;
        save(&pool, input()).await.unwrap();
        sqlx::query("UPDATE accounts SET current_balance=current_balance+100000 WHERE id='loan'").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        for kind in ["repayment", "transfer"] {
            let input = NewTransaction { kind:kind.into(),account_id:"bank".into(),destination_account_id:Some("loan".into()),amount:"100".into(),date:"2024-03-01".into(),description:"Payment".into(),currency:"THB".into(),payee_id:None,category_id:None,income_source_id:None,cleared_account_ids:vec![] };
            let error = crate::transactions::insert(&pool,input).await.err().unwrap();
            assert!(error.contains("This loan has contracts"));
            assert_eq!(balances(&pool).await,before);
        }
        assert_eq!(rows(&pool).await[0].remaining_principal,"2000000");
    }

    #[tokio::test]
    async fn account_creation_can_enable_revolving_credit_atomically() {
        let pool = database().await;
        let input = json!({"name":"New facility","type":"loan","loan_type":"personal_loan","opening_balance":"0","currency":"THB","revolving_credit_limit":"10000000"});
        crate::accounts::insert_account(&pool, serde_json::from_value(input.clone()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            facilities(&mut *pool.acquire().await.unwrap())
                .await
                .unwrap()
                .len(),
            2
        );
        let mut invalid = input;
        invalid["loan_type"] = json!("mortgage");
        assert!(
            crate::accounts::insert_account(&pool, serde_json::from_value(invalid).unwrap())
                .await
                .is_err()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }
}
