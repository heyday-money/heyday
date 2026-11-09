//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::transactions as service;
pub use heyday_core::services::transactions::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_expense_report(pool: tauri::State<'_, SqlitePool>) -> Result<ExpenseReport, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_expense_report(pool.inner()).await
}

#[tauri::command]
pub async fn update_transaction(pool: tauri::State<'_, SqlitePool>, input: UpdateTransaction) -> Result<Transaction, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::update_transaction(pool.inner(), input).await
}

#[tauri::command]
pub async fn create_transaction(
    pool: tauri::State<'_, SqlitePool>,
    input: NewTransaction,
) -> Result<Transaction, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::create_transaction(pool.inner(), input).await
}

#[tauri::command]
pub async fn delete_transaction(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
    confirm_reconciled: Option<bool>,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::delete_transaction(pool.inner(), id, confirm_reconciled).await
}

#[tauri::command]
pub async fn list_transactions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Transaction>, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_transactions(pool.inner()).await
}
#[cfg(test)]
pub async fn expense_snapshot(pool: &SqlitePool) -> Result<ExpenseReport, String> {
    service::expense_snapshot(pool).await
}
#[cfg(test)]
pub async fn update(pool: &SqlitePool, input: UpdateTransaction) -> Result<Transaction, String> {
    service::update(pool, input).await
}
#[cfg(test)]
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    service::remove(pool, id).await
}
#[cfg(test)]
pub async fn remove_confirmed(
    pool: &SqlitePool,
    id: &str,
    confirmed: bool,
) -> Result<(), String> {
    service::remove_confirmed(pool, id, confirmed).await
}
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn editable_database() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000),('card','Card','credit_card',2000,2000),('cash','Cash','cash',0,0);").execute(&pool).await.unwrap();
        pool
    }
    async fn saved(pool: &SqlitePool, id: &str) -> Transaction {
        sqlx::query_as(&format!("{SELECT} WHERE t.id=?")).bind(id).fetch_one(pool).await.unwrap()
    }
    #[tokio::test]
    async fn editing_keeps_identity_updates_both_balances_and_queues_sync() {
        let pool = editable_database().await;
        crate::cloud_sync_store::enable(&pool,"https://example.test","account","dataset","key").await.unwrap();
        sqlx::query("UPDATE cloud_sync_binding SET financial_enabled=1").execute(&pool).await.unwrap();
        let mut original = input("expense","card",None,"1000"); original.cleared_account_ids=vec!["card".into()];
        let old = insert(&pool, original).await.unwrap(); let id=old.id.clone();
        sqlx::query("UPDATE transactions SET created_at='2001-01-01' WHERE id=?").bind(&id).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM cloud_sync_dirty").execute(&pool).await.unwrap();
        let result = update(&pool, UpdateTransaction { expected: old, changes: input("transfer","bank",Some("cash"),"700") }).await.unwrap();
        assert_eq!(result.id,id);
        assert_eq!(balances(&pool).await,vec![("bank".into(),10000,9300),("card".into(),2000,2000),("cash".into(),0,700)]);
        let verification: Vec<(String,String)> = sqlx::query_as("SELECT account_id,status FROM transaction_verifications WHERE transaction_id=? ORDER BY account_id").bind(&id).fetch_all(&pool).await.unwrap();
        assert_eq!(verification,vec![("bank".into(),"uncleared".into()),("cash".into(),"uncleared".into())]);
        let stored:(i64,String)=sqlx::query_as("SELECT count(*),created_at FROM transactions").fetch_one(&pool).await.unwrap(); assert_eq!(stored,(1,"2001-01-01".into()));
        let revision:i64=sqlx::query_scalar("SELECT revision FROM cloud_sync_dirty WHERE record_id=?").bind(format!("transaction:{id}")).fetch_one(&pool).await.unwrap();assert_eq!(revision,1);
        let mut conn=pool.acquire().await.unwrap();
        let record=crate::cloud_sync_ledger::current(&mut conn,"transaction",&id).await.unwrap();
        let Some(crate::cloud_sync_store::Value::Transaction(group))=record.value else {panic!("missing transaction")};
        assert_eq!(group.entries[0].amount,"700");assert_eq!(group.entries[0].kind,"transfer");
    }
    #[tokio::test]
    async fn editing_rejects_reconciliation_on_either_side_and_retained_history() {
        let pool=editable_database().await;
        let old=insert(&pool,input("transfer","bank",Some("card"),"500")).await.unwrap();let id=old.id.clone();
        let before=balances(&pool).await;
        sqlx::raw_sql("INSERT INTO reconciliations(id,account_id,confirmed_balance,opening_balance) VALUES(1,'card',1500,2000)").execute(&pool).await.unwrap();
        sqlx::query("UPDATE transaction_verifications SET status='reconciled',reconciliation_id=1 WHERE transaction_id=? AND account_id='card'").bind(&id).execute(&pool).await.unwrap();
        assert!(update(&pool,UpdateTransaction{expected:old,changes:input("expense","bank",None,"200")}).await.unwrap_err().contains("reconciliation history"));
        sqlx::query("INSERT INTO reconciliation_entries(reconciliation_id,transaction_id,date,description,type,balance_change) VALUES(1,?,'2024-02-29','Test','transfer',-500)").bind(&id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE transaction_verifications SET status='cleared',reconciliation_id=NULL WHERE transaction_id=?").bind(&id).execute(&pool).await.unwrap();
        assert!(update(&pool,UpdateTransaction{expected:saved(&pool,&id).await,changes:input("income","bank",None,"200")}).await.is_err());
        assert_eq!(balances(&pool).await,before);assert_eq!(saved(&pool,&id).await.amount,"500");
    }
    #[tokio::test]
    async fn editing_invalid_and_stale_changes_roll_back_every_balance_and_keep_references() {
        let pool=editable_database().await;
        sqlx::raw_sql("INSERT INTO payees(id,name,name_key) VALUES('shop','Shop','shop'); INSERT INTO categories(id,name,name_key) VALUES('custom','Custom','custom');").execute(&pool).await.unwrap();
        let mut original=input("expense","bank",None,"1000");original.payee_id=Some("shop".into());original.category_id=Some("custom".into());
        let old=insert(&pool,original).await.unwrap();let id=old.id.clone();let before=balances(&pool).await;
        let mut invalid=input("repayment","bank",Some("cash"),"800");invalid.description="Invalid".into();
        assert!(update(&pool,UpdateTransaction{expected:old,changes:invalid}).await.is_err());
        assert_eq!(balances(&pool).await,before);assert_eq!(saved(&pool,&id).await.amount,"1000");
        let stale=saved(&pool,&id).await;
        sqlx::query("UPDATE transactions SET description='Changed elsewhere' WHERE id=?").bind(&id).execute(&pool).await.unwrap();
        assert!(update(&pool,UpdateTransaction{expected:stale,changes:input("expense","bank",None,"900")}).await.unwrap_err().contains("transaction changed"));
        sqlx::raw_sql("UPDATE payees SET is_archived=1 WHERE id='shop'; UPDATE categories SET is_archived=1 WHERE id='custom'").execute(&pool).await.unwrap();
        let mut changes=input("expense","bank",None,"900");changes.payee_id=Some("shop".into());changes.category_id=Some("custom".into());
        let row=update(&pool,UpdateTransaction{expected:saved(&pool,&id).await,changes}).await.unwrap();assert_eq!(row.payee_id.as_deref(),Some("shop"));assert_eq!(row.category_id.as_deref(),Some("custom"));
    }
    #[tokio::test]
    async fn editing_large_amounts_uses_the_final_balance_without_intermediate_overflow() {
        let pool=editable_database().await;
        sqlx::query("UPDATE accounts SET opening_balance=0,current_balance=0 WHERE id='bank'").execute(&pool).await.unwrap();
        let old=insert(&pool,input("expense","bank",None,"9223372036854775807")).await.unwrap();
        let row=update(&pool,UpdateTransaction{expected:old,changes:input("income","bank",None,"9223372036854775807")}).await.unwrap();
        assert_eq!(row.amount,"9223372036854775807");
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT current_balance FROM accounts WHERE id='bank'").fetch_one(&pool).await.unwrap(),i64::MAX);
    }
    #[tokio::test]
    async fn editing_card_payment_reviews_statement_and_removes_invalid_allocation() {
        let pool=editable_database().await;
        let old=insert(&pool,input("repayment","bank",Some("card"),"500")).await.unwrap();let id=old.id.clone();
        sqlx::raw_sql("INSERT INTO card_statements(id,account_id,start_date,end_date,due_date,amount,minimum) VALUES('bill','card','2024-02-01','2024-02-29','2024-03-15',2000,100)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO card_payment_allocations VALUES(?,'bill')").bind(&id).execute(&pool).await.unwrap();
        update(&pool,UpdateTransaction{expected:old,changes:input("expense","bank",None,"400")}).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT needs_review FROM card_statements WHERE id='bill'").fetch_one(&pool).await.unwrap(),1);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM card_payment_allocations").fetch_one(&pool).await.unwrap(),0);
        sqlx::query("UPDATE ledger_sync_members SET group_id='loan-group' WHERE transaction_id=?").bind(&id).execute(&pool).await.unwrap();
        assert!(update(&pool,UpdateTransaction{expected:saved(&pool,&id).await,changes:input("expense","bank",None,"100")}).await.unwrap_err().contains("Linked salary"));
    }
    async fn database() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0002_account_details.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Bank', 'bank', 10000), ('card', 'Card', 'credit_card', 2000), ('loan', 'Loan', 'loan', -100), ('cash', 'Cash', 'cash', 0)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0003_transactions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0004_optional_transaction_description.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0005_payees_and_categories.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0022_reconciliation.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0031_institutions.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0032_custom_logos.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0007_loan_types.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
        pool
    }
    #[tokio::test]
    async fn expense_report_preserves_archived_history_and_excludes_payments() {
        let pool = database().await;
        let card = insert(&pool, input("expense", "card", None, "9007199254740993")).await.unwrap();
        insert(&pool, input("expense", "bank", None, "100")).await.unwrap();
        insert(&pool, input("repayment", "bank", Some("card"), "200")).await.unwrap();
        insert(&pool, input("transfer", "bank", Some("cash"), "100")).await.unwrap();
        insert(&pool, input("income", "bank", None, "500")).await.unwrap();
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='card'").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let report = expense_snapshot(&pool).await.unwrap();
        assert_eq!(report.transactions.len(), 2);
        assert!(report.transactions.iter().all(|t|t.kind=="expense"));
        assert_eq!(report.transactions.iter().find(|t|t.id==card.id).unwrap().amount,"9007199254740993");
        let account=report.accounts.iter().find(|a|a.id=="card").unwrap();
        assert_eq!(account.kind,"credit_card");assert!(account.is_archived);
        assert_eq!(report.settings.currency.as_deref(),Some("THB"));
        assert_eq!(balances(&pool).await,before);
    }

    #[tokio::test]
    async fn income_sources_validate_and_preserve_actual_net_receipts() {
        let pool = database().await;
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Company salary','bank','salary',5000000,1)").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let mut value = input("income", "bank", None, "4500000");
        value.income_source_id = Some("missing".into());
        assert!(insert(&pool,value).await.is_err());
        let mut wrong = input("expense", "bank", None, "10");
        wrong.income_source_id = Some("salary".into());
        assert!(insert(&pool,wrong).await.is_err());
        assert_eq!(balances(&pool).await,before);
        let mut receipt = input("income", "bank", None, "4500000");
        receipt.income_source_id = Some("salary".into());
        let saved = insert(&pool,receipt).await.unwrap();
        assert_eq!(saved.income_source_name.as_deref(),Some("Company salary"));
        assert_eq!(saved.amount,"4500000");
        let balance: i64 = sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(balance,4510000);
        sqlx::query("UPDATE incomes SET is_active=0 WHERE id='salary'").execute(&pool).await.unwrap();
        let mut disabled = input("income", "bank", None, "1");
        disabled.income_source_id = Some("salary".into());
        assert!(insert(&pool,disabled).await.is_err());
        let history: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id=?")).bind(&saved.id).fetch_one(&pool).await.unwrap();
        assert_eq!(history.income_source_id.as_deref(),Some("salary"));
    }

    fn input(kind: &str, account: &str, destination: Option<&str>, amount: &str) -> NewTransaction {
        NewTransaction {
            kind: kind.into(),
            account_id: account.into(),
            destination_account_id: destination.map(str::to_owned),
            amount: amount.into(),
            date: "2024-02-29".into(),
            description: " Test ".into(),
            currency: "THB".into(),
            payee_id: None,
            category_id: None,
            income_source_id: None,
            cleared_account_ids: vec![],
        }
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id, opening_balance, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn balances_signs_and_reversal() {
        let pool = database().await;
        let initial = balances(&pool).await;
        assert!(initial
            .iter()
            .all(|(_, opening, current)| opening == current));
        let income = insert(&pool, input("income", "bank", None, "1000"))
            .await
            .unwrap();
        let expense = insert(&pool, input("expense", "card", None, "500"))
            .await
            .unwrap();
        let repayment = insert(&pool, input("repayment", "bank", Some("card"), "3000"))
            .await
            .unwrap();
        let transfer = insert(&pool, input("transfer", "bank", Some("cash"), "200"))
            .await
            .unwrap();
        let loan = insert(&pool, input("expense", "loan", None, "500"))
            .await
            .unwrap();
        assert_eq!(
            balances(&pool).await,
            vec![
                ("bank".into(), 10000, 7800),
                ("card".into(), 2000, -500),
                ("cash".into(), 0, 200),
                ("loan".into(), -100, 400)
            ]
        );
        for row in [income, expense, repayment, transfer, loan] {
            remove(&pool, &row.id).await.unwrap();
            assert!(remove(&pool, &row.id).await.is_err());
        }
        assert_eq!(balances(&pool).await, initial);
    }
    #[tokio::test]
    async fn invalid_inputs_and_overflow_roll_back_everything() {
        let pool = database().await;
        sqlx::query("UPDATE accounts SET current_balance = ? WHERE id = 'card'")
            .bind(i64::MIN)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET is_archived = 1 WHERE id = 'cash'")
            .execute(&pool)
            .await
            .unwrap();
        let initial = balances(&pool).await;
        let mut cases = vec![
            input("transfer", "bank", Some("card"), "1"),
            input("income", "missing", None, "1"),
            input("income", "cash", None, "1"),
            input("transfer", "bank", Some("bank"), "1"),
            input("transfer", "bank", None, "1"),
            input("income", "bank", Some("card"), "1"),
            input("repayment", "card", Some("bank"), "1"),
            input("unknown", "bank", None, "1"),
        ];
        for amount in ["0", "-1", "1.5", "9223372036854775808"] {
            cases.push(input("income", "bank", None, amount));
        }
        let mut stale = input("income", "bank", None, "1");
        stale.currency = "USD".into();
        cases.push(stale);
        let mut date = input("income", "bank", None, "1");
        date.date = "2025-02-29".into();
        cases.push(date);
        let mut future = input("income", "bank", None, "1");
        future.date = "9999-01-01".into();
        cases.push(future);
        let mut empty = input("income", "bank", None, "1");
        empty.description = "x".repeat(201);
        cases.push(empty);
        for case in cases {
            assert!(insert(&pool, case).await.is_err());
            assert_eq!(balances(&pool).await, initial);
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    #[tokio::test]
    async fn precise_amounts_and_failed_delete_are_atomic() {
        let pool = database().await;
        let row = insert(&pool, input("income", "bank", None, "9007199254740993"))
            .await
            .unwrap();
        assert_eq!(row.amount, "9007199254740993");
        let stored: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 9007199254750993);
        remove(&pool, &row.id).await.unwrap();
        let row = insert(&pool, input("transfer", "bank", Some("card"), "10"))
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET current_balance = ? WHERE id = 'card'")
            .bind(i64::MAX)
            .execute(&pool)
            .await
            .unwrap();
        let initial = balances(&pool).await;
        assert!(remove(&pool, &row.id).await.is_err());
        assert_eq!(balances(&pool).await, initial);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn transactions_survive_reopen() {
        let path = std::env::temp_dir().join(format!(
            "heyday-transactions-{}-{}.db",
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance) VALUES ('bank', 'Bank', 'bank', 100, 100)").execute(&pool).await.unwrap();
        let payee = option(
            &pool,
            crate::transaction_options::OptionKind::Payee,
            None,
            "Market",
            false,
        )
        .await;
        let category = option(
            &pool,
            crate::transaction_options::OptionKind::Category,
            None,
            "Groceries",
            false,
        )
        .await;
        let mut value = input("expense", "bank", None, "125");
        value.payee_id = Some(payee.id.clone());
        value.category_id = Some(category.id.clone());
        let row = insert(&pool, value).await.unwrap();
        pool.close().await;
        let pool = SqlitePool::connect_with(options).await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind(&row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.amount, "125");
        assert_eq!(stored.payee_name.as_deref(), Some("Market"));
        assert_eq!(stored.category_name.as_deref(), Some("Groceries"));
        assert_eq!(stored.payee_id.as_deref(), Some(payee.id.as_str()));
        assert_eq!(stored.category_id.as_deref(), Some(category.id.as_str()));
        assert_eq!(balances(&pool).await, vec![("bank".into(), 100, -25)]);
        remove(&pool, &row.id).await.unwrap();
        assert_eq!(balances(&pool).await, vec![("bank".into(), 100, 100)]);
        pool.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
    #[tokio::test]
    async fn optional_description_migration_preserves_history_and_balances() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in [
            include_str!("../tests/fixtures/alpha-migrations/0001_initial.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0002_account_details.sql"),
            include_str!("../tests/fixtures/alpha-migrations/0003_transactions.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance) VALUES ('bank', 'Bank', 'bank', 1000, 1000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transactions (id, type, account_id, amount, date, description) VALUES ('old', 'expense', 'bank', 125, '2024-02-29', 'Test')").execute(&pool).await.unwrap();
        sqlx::query("UPDATE accounts SET current_balance = 875 WHERE id = 'bank'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0004_optional_transaction_description.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0005_payees_and_categories.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0022_reconciliation.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0007_loan_types.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind("old")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.description, "Test");
        assert_eq!(stored.amount, "125");
        assert_eq!(balances(&pool).await, vec![("bank".into(), 1000, 875)]);
        for description in ["", "   "] {
            let mut value = input("expense", "bank", None, "25");
            value.description = description.into();
            let row = insert(&pool, value).await.unwrap();
            assert_eq!(row.description, "");
            remove(&pool, &row.id).await.unwrap();
        }
        remove(&pool, "old").await.unwrap();
        assert_eq!(balances(&pool).await, vec![("bank".into(), 1000, 1000)]);
    }
    async fn option(
        pool: &SqlitePool,
        kind: crate::transaction_options::OptionKind,
        id: Option<String>,
        name: &str,
        archived: bool,
    ) -> crate::transaction_options::TransactionOption {
        crate::transaction_options::save(
            pool,
            crate::transaction_options::SaveOption { logo_change: None,
                icon: None,
                kind,
                id,
                name: name.into(),
                is_archived: archived,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn payees_and_categories_validate_and_preserve_references() {
        use crate::transaction_options::{save, OptionKind, SaveOption};
        let pool = database().await;
        let payee = option(&pool, OptionKind::Payee, None, "  Corner   Shop  ", false).await;
        let category = option(&pool, OptionKind::Category, None, "Groceries", false).await;
        assert_eq!(payee.name, "Corner Shop");
        for kind in [OptionKind::Payee, OptionKind::Category] {
            for name in [" ".to_string(), "x".repeat(101)] {
                assert!(save(
                    &pool,
                    SaveOption { logo_change: None,
                icon: None,
                        kind,
                        id: None,
                        name,
                        is_archived: false
                    }
                )
                .await
                .is_err());
            }
        }
        assert!(save(
            &pool,
            SaveOption { logo_change: None,
                icon: None,
                kind: OptionKind::Payee,
                id: None,
                name: "corner shop".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveOption { logo_change: None,
                icon: None,
                kind: OptionKind::Category,
                id: Some("missing".into()),
                name: "New name".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        let initial = balances(&pool).await;
        for (kind, payee_id, category_id) in [
            ("expense", Some("missing".to_string()), None),
            ("expense", None, Some("missing".to_string())),
            ("income", Some(payee.id.clone()), None),
            ("transfer", None, Some(category.id.clone())),
        ] {
            let mut value = input(
                kind,
                "bank",
                if kind == "transfer" {
                    Some("card")
                } else {
                    None
                },
                "100",
            );
            value.payee_id = payee_id;
            value.category_id = category_id;
            assert!(insert(&pool, value).await.is_err());
            assert_eq!(balances(&pool).await, initial);
        }
        let mut value = input("expense", "bank", None, "100");
        value.payee_id = Some(payee.id.clone());
        value.category_id = Some(category.id.clone());
        let row = insert(&pool, value).await.unwrap();
        assert_eq!(row.payee_name.as_deref(), Some("Corner Shop"));
        assert_eq!(row.category_name.as_deref(), Some("Groceries"));
        option(
            &pool,
            OptionKind::Payee,
            Some(payee.id.clone()),
            "Local Market",
            true,
        )
        .await;
        option(
            &pool,
            OptionKind::Category,
            Some(category.id.clone()),
            "Food",
            true,
        )
        .await;
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind(&row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.payee_name.as_deref(), Some("Local Market"));
        assert_eq!(stored.category_name.as_deref(), Some("Food"));
        let archived_balances = balances(&pool).await;
        for (payee_id, category_id) in [
            (Some(payee.id.clone()), None),
            (None, Some(category.id.clone())),
        ] {
            let mut value = input("expense", "bank", None, "100");
            value.payee_id = payee_id;
            value.category_id = category_id;
            assert!(insert(&pool, value).await.is_err());
            assert_eq!(balances(&pool).await, archived_balances);
        }
        assert!(sqlx::query("DELETE FROM payees WHERE id = ?")
            .bind(&payee.id)
            .execute(&pool)
            .await
            .is_err());
        assert!(sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(&category.id)
            .execute(&pool)
            .await
            .is_err());
        option(
            &pool,
            OptionKind::Payee,
            Some(payee.id.clone()),
            "Local Market",
            false,
        )
        .await;
        option(
            &pool,
            OptionKind::Category,
            Some(category.id.clone()),
            "Food",
            false,
        )
        .await;
        let mut restored = input("expense", "bank", None, "100");
        restored.payee_id = Some(payee.id);
        restored.category_id = Some(category.id);
        let second = insert(&pool, restored).await.unwrap();
        remove(&pool, &row.id).await.unwrap();
        remove(&pool, &second.id).await.unwrap();
        assert_eq!(balances(&pool).await, initial);
    }
}
