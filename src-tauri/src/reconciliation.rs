//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::reconciliation as service;
pub use heyday_core::services::reconciliation::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_reconciliation(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<Snapshot, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_reconciliation(pool.inner(), account_id).await
}

#[tauri::command]
pub async fn set_transaction_verification(
    pool: tauri::State<'_, SqlitePool>,
    input: VerificationUpdate,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::set_transaction_verification(pool.inner(), input).await
}

#[tauri::command]
pub async fn finish_reconciliation(
    pool: tauri::State<'_, SqlitePool>,
    input: Finish,
) -> Result<Snapshot, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::finish_reconciliation(pool.inner(), input).await
}

#[cfg(test)]
pub async fn read(pool: &SqlitePool, account: &str) -> Result<Snapshot, String> {
    service::read(pool, account).await
}
#[cfg(test)]
pub async fn finish(pool: &SqlitePool, input: Finish) -> Result<Snapshot, String> {
    service::finish(pool, input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::{insert_in_connection, NewTransaction};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn pool() -> SqlitePool {
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
        seed(&pool).await;
        pool
    }
    async fn seed(pool: &SqlitePool) {
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES ('bank','Bank','bank',10000,10000),('card','Card','credit_card',2000,2000),('cash','Cash','cash',0,0);")
            .execute(pool).await.unwrap();
    }
    async fn record(
        pool: &SqlitePool,
        kind: &str,
        account: &str,
        to: Option<&str>,
        amount: &str,
        cleared: &[&str],
    ) -> String {
        let mut tx = pool.begin().await.unwrap();
        let row = insert_in_connection(
            &mut tx,
            NewTransaction {
                kind: kind.into(),
                account_id: account.into(),
                destination_account_id: to.map(str::to_owned),
                amount: amount.into(),
                date: "2026-01-01".into(),
                description: "Entry".into(),
                currency: "THB".into(),
                payee_id: None,
                category_id: None,
                income_source_id: None,
                cleared_account_ids: cleared.iter().map(|id| (*id).into()).collect(),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        row.id
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id,opening_balance,current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    async fn status(
        pool: &SqlitePool,
        account: &str,
        transaction: &str,
        expected: &str,
        next: &str,
        confirmed: bool,
    ) -> Result<(), String> {
        set_status(
            pool,
            VerificationUpdate {
                account_id: account.into(),
                entries: vec![StatusChange {
                    transaction_id: transaction.into(),
                    expected_status: expected.into(),
                }],
                status: next.into(),
                confirm_unlock: confirmed,
            },
        )
        .await
    }
    fn completion(snapshot: &Snapshot, posted: &str, confirmed: bool) -> Finish {
        Finish {
            account_id: snapshot.account_id.clone(),
            posted_balance: posted.into(),
            expected_token: snapshot.token.clone(),
            confirm_opening_balance: confirmed,
        }
    }

    #[tokio::test]
    async fn pending_entries_opening_confirmation_and_exact_completion() {
        let pool = pool().await;
        let posted = record(&pool, "expense", "bank", None, "1000", &[]).await;
        let pending = record(&pool, "expense", "bank", None, "2000", &[]).await;
        let before = balances(&pool).await;
        let snap = read(&pool, "bank").await.unwrap();
        assert_eq!(
            (
                &snap.working_balance,
                &snap.cleared_balance,
                &snap.uncleared_changes
            ),
            (&"7000".into(), &"10000".into(), &"-3000".into())
        );
        assert!(snap.entries.iter().all(|entry| entry.status == "uncleared"));
        status(&pool, "bank", &posted, "uncleared", "cleared", false)
            .await
            .unwrap();
        let snap = read(&pool, "bank").await.unwrap();
        assert_eq!(snap.cleared_balance, "9000");
        assert!(finish(&pool, completion(&snap, "9000", false))
            .await
            .is_err());
        assert!(finish(&pool, completion(&snap, "7000", true))
            .await
            .is_err());
        let done = finish(&pool, completion(&snap, "9000", true))
            .await
            .unwrap();
        assert_eq!(done.history.len(), 1);
        assert_eq!(done.history[0].opening_balance, "10000");
        assert!(!done.history[0].completed_at.is_empty());
        assert_eq!(done.history_entries.len(), 1);
        assert_eq!(done.history_entries[0].transaction_id, posted);
        assert_eq!(
            done.entries
                .iter()
                .find(|e| e.transaction_id == pending)
                .unwrap()
                .status,
            "uncleared"
        );
        assert_eq!(
            done.entries
                .iter()
                .find(|e| e.transaction_id == posted)
                .unwrap()
                .status,
            "reconciled"
        );
        assert_eq!(balances(&pool).await, before);
        assert!(
            status(&pool, "bank", &posted, "reconciled", "uncleared", false)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn transfer_sides_card_debt_credits_and_zero_balance_are_independent() {
        let pool = pool().await;
        let transfer = record(&pool, "repayment", "bank", Some("card"), "3000", &["bank"]).await;
        let before = balances(&pool).await;
        let bank = read(&pool, "bank").await.unwrap();
        let card = read(&pool, "card").await.unwrap();
        assert_eq!(bank.entries[0].status, "cleared");
        assert_eq!(card.entries[0].status, "uncleared");
        assert_eq!(bank.cleared_balance, "7000");
        assert_eq!(card.working_balance, "-1000");
        assert_eq!(card.cleared_balance, "2000");
        finish(&pool, completion(&bank, "7000", true))
            .await
            .unwrap();
        status(&pool, "card", &transfer, "uncleared", "cleared", false)
            .await
            .unwrap();
        let card = read(&pool, "card").await.unwrap();
        assert_eq!(card.cleared_balance, "-1000");
        finish(&pool, completion(&card, "-1000", true))
            .await
            .unwrap();
        assert_eq!(balances(&pool).await, before);
        record(&pool, "expense", "card", None, "1000", &["card"]).await;
        let card = read(&pool, "card").await.unwrap();
        assert_eq!(card.cleared_balance, "0");
        assert!(finish(&pool, completion(&card, "0", false)).await.is_ok());
        assert_eq!(
            read(&pool, "bank").await.unwrap().entries[0].status,
            "reconciled"
        );
        assert!(read(&pool, "cash").await.is_err());
    }

    #[tokio::test]
    async fn unlocking_and_deletion_require_confirmation_preserve_evidence_and_review_later_history(
    ) {
        let pool = pool().await;
        let transfer = record(
            &pool,
            "transfer",
            "bank",
            Some("card"),
            "1000",
            &["bank", "card"],
        )
        .await;
        for id in ["bank", "card"] {
            let snap = read(&pool, id).await.unwrap();
            finish(&pool, completion(&snap, &snap.cleared_balance, true))
                .await
                .unwrap();
            let snap = read(&pool, id).await.unwrap();
            finish(&pool, completion(&snap, &snap.cleared_balance, false))
                .await
                .unwrap();
        }
        let before = balances(&pool).await;
        assert!(
            crate::transactions::remove_confirmed(&pool, &transfer, false)
                .await
                .is_err()
        );
        assert_eq!(balances(&pool).await, before);
        status(&pool, "bank", &transfer, "reconciled", "cleared", true)
            .await
            .unwrap();
        assert_eq!(balances(&pool).await, before);
        assert!(read(&pool, "bank")
            .await
            .unwrap()
            .history
            .iter()
            .all(|h| h.needs_review));
        assert!(read(&pool, "card")
            .await
            .unwrap()
            .history
            .iter()
            .all(|h| !h.needs_review));
        assert_eq!(
            read(&pool, "card").await.unwrap().entries[0].status,
            "reconciled"
        );
        // Even after unlocking, deleting an entry with historical evidence needs confirmation.
        assert!(
            crate::transactions::remove_confirmed(&pool, &transfer, false)
                .await
                .is_err()
        );
        sqlx::query("CREATE TRIGGER fail_delete BEFORE DELETE ON transactions BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&pool).await.unwrap();
        assert!(
            crate::transactions::remove_confirmed(&pool, &transfer, true)
                .await
                .is_err()
        );
        assert_eq!(balances(&pool).await, before);
        assert!(read(&pool, "card")
            .await
            .unwrap()
            .history
            .iter()
            .all(|h| !h.needs_review));
        sqlx::query("DROP TRIGGER fail_delete")
            .execute(&pool)
            .await
            .unwrap();
        crate::transactions::remove_confirmed(&pool, &transfer, true)
            .await
            .unwrap();
        for id in ["bank", "card"] {
            let snap = read(&pool, id).await.unwrap();
            assert!(snap.entries.is_empty());
            assert!(snap.history.iter().all(|h| h.needs_review));
            assert_eq!(snap.history_entries.len(), 1);
            assert_eq!(snap.history_entries[0].transaction_id, transfer);
        }
        assert_eq!(read(&pool, "bank").await.unwrap().working_balance, "10000");
        assert_eq!(read(&pool, "card").await.unwrap().working_balance, "2000");
    }

    #[tokio::test]
    async fn stale_comparisons_bulk_conflicts_and_failed_finish_roll_back() {
        let pool = pool().await;
        let first = record(&pool, "expense", "bank", None, "1000", &[]).await;
        let second = record(&pool, "income", "bank", None, "1000", &[]).await;
        let old = read(&pool, "bank").await.unwrap();
        // Net cleared balance stays identical, but changed verification must still invalidate the comparison.
        for id in [&first, &second] {
            status(&pool, "bank", id, "uncleared", "cleared", false)
                .await
                .unwrap();
        }
        assert!(finish(&pool, completion(&old, "10000", true))
            .await
            .is_err());
        assert!(set_status(
            &pool,
            VerificationUpdate {
                account_id: "bank".into(),
                status: "uncleared".into(),
                confirm_unlock: false,
                entries: vec![
                    StatusChange {
                        transaction_id: first.clone(),
                        expected_status: "cleared".into()
                    },
                    StatusChange {
                        transaction_id: second.clone(),
                        expected_status: "uncleared".into()
                    }
                ]
            }
        )
        .await
        .is_err());
        let current = read(&pool, "bank").await.unwrap();
        assert!(current.entries.iter().all(|e| e.status == "cleared"));
        let before = balances(&pool).await;
        sqlx::query("CREATE TRIGGER fail_reconciliation BEFORE UPDATE ON transaction_verifications WHEN NEW.status='reconciled' AND (SELECT count(*) FROM transaction_verifications WHERE status='reconciled') > 0 BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&pool).await.unwrap();
        assert!(finish(&pool, completion(&current, "10000", true))
            .await
            .is_err());
        let unchanged = read(&pool, "bank").await.unwrap();
        assert_eq!(unchanged.token, current.token);
        assert!(unchanged.history.is_empty());
        assert!(unchanged.history_entries.is_empty());
        assert_eq!(balances(&pool).await, before);
        sqlx::query("DROP TRIGGER fail_reconciliation")
            .execute(&pool)
            .await
            .unwrap();
        record(&pool, "expense", "bank", None, "1", &[]).await;
        // A new pending transaction doesn't alter the cleared balance, but is still a stale snapshot.
        assert!(finish(&pool, completion(&current, "10000", true))
            .await
            .is_err());
        let latest = read(&pool, "bank").await.unwrap();
        assert!(finish(&pool, completion(&latest, "10000", true))
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn migration_never_infers_verification_and_reconciliation_survives_restart() {
        let path = std::env::temp_dir().join(format!(
            "heyday-reconciliation-{}-{}.db",
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 22)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        seed(&pool).await;
        sqlx::raw_sql("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES ('legacy','transfer','bank','card',500,'2026-01-01','Old payment'); UPDATE accounts SET current_balance=current_balance-500 WHERE id IN ('bank','card');").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0022_reconciliation.sql"))
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
        for id in ["bank", "card"] {
            assert_eq!(
                read(&pool, id).await.unwrap().entries[0].status,
                "uncleared"
            );
        }
        assert_eq!(balances(&pool).await, before);
        status(&pool, "bank", "legacy", "uncleared", "cleared", false)
            .await
            .unwrap();
        let snap = read(&pool, "bank").await.unwrap();
        let finished = finish(&pool, completion(&snap, "9500", true))
            .await
            .unwrap();
        pool.close().await;
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        assert_eq!(read(&reopened, "bank").await.unwrap().token, finished.token);
        assert_eq!(
            read(&reopened, "card").await.unwrap().entries[0].status,
            "uncleared"
        );
        assert_eq!(balances(&reopened).await, before);
        reopened.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }

    #[tokio::test]
    async fn large_pending_totals_remain_exact_and_wrong_account_clearing_is_rejected() {
        let pool = pool().await;
        sqlx::query("UPDATE accounts SET opening_balance=9007199254740993,current_balance=9007199254740993 WHERE id='bank'").execute(&pool).await.unwrap();
        let id = record(&pool, "expense", "bank", None, "1", &[]).await;
        let snap = read(&pool, "bank").await.unwrap();
        assert_eq!(snap.cleared_balance, "9007199254740993");
        assert_eq!(snap.working_balance, "9007199254740992");
        assert!(status(&pool, "card", &id, "uncleared", "cleared", false)
            .await
            .is_err());
        let before = balances(&pool).await;
        finish(&pool, completion(&snap, "9007199254740993", true))
            .await
            .unwrap();
        assert_eq!(balances(&pool).await, before);
    }
    #[tokio::test]
    async fn cleared_and_pending_aggregates_can_exceed_i64_without_rounding() {
        let pool = pool().await;
        sqlx::query("UPDATE accounts SET opening_balance=0,current_balance=0 WHERE id='bank'")
            .execute(&pool)
            .await
            .unwrap();
        record(&pool, "expense", "bank", None, "9223372036854775807", &[]).await;
        record(
            &pool,
            "income",
            "bank",
            None,
            "9223372036854775807",
            &["bank"],
        )
        .await;
        record(
            &pool,
            "income",
            "bank",
            None,
            "9223372036854775807",
            &["bank"],
        )
        .await;
        let snap = read(&pool, "bank").await.unwrap();
        assert_eq!(snap.working_balance, "9223372036854775807");
        assert_eq!(snap.uncleared_changes, "-9223372036854775807");
        assert_eq!(snap.cleared_balance, "18446744073709551614");
        assert!(
            finish(&pool, completion(&snap, "9223372036854775807", true))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn wallet_reconciliation_preserves_cash_balances_and_independent_top_up_status() {
        let pool = pool().await;
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES ('wallet','TrueMoney','wallet',5000,5000)").execute(&pool).await.unwrap();
        let top_up = record(&pool, "transfer", "bank", Some("wallet"), "1000", &["bank"]).await;
        let purchase = record(&pool, "expense", "wallet", None, "2000", &[]).await;
        let before = balances(&pool).await;
        let wallet = read(&pool, "wallet").await.unwrap();
        assert_eq!(wallet.working_balance, "4000");
        assert_eq!(wallet.cleared_balance, "5000");
        status(&pool, "wallet", &purchase, "uncleared", "cleared", false)
            .await
            .unwrap();
        let wallet = read(&pool, "wallet").await.unwrap();
        assert_eq!(wallet.cleared_balance, "3000");
        assert_eq!(wallet.uncleared_changes, "1000");
        let done = finish(&pool, completion(&wallet, "3000", true))
            .await
            .unwrap();
        assert_eq!(
            done.entries
                .iter()
                .find(|entry| entry.transaction_id == top_up)
                .unwrap()
                .status,
            "uncleared"
        );
        assert_eq!(
            done.entries
                .iter()
                .find(|entry| entry.transaction_id == purchase)
                .unwrap()
                .status,
            "reconciled"
        );
        assert_eq!(
            read(&pool, "bank").await.unwrap().entries[0].status,
            "cleared"
        );
        assert_eq!(balances(&pool).await, before);
        record(&pool, "income", "wallet", None, "100", &["wallet"]).await;
        assert_eq!(read(&pool, "wallet").await.unwrap().cleared_balance, "3100");
    }

    #[tokio::test]
    async fn payroll_loan_principal_reconciles_and_group_reversal_preserves_evidence() {
        let pool=crate::salary_payments::tests::database().await;
        crate::salary_payments::record(&pool,crate::salary_payments::tests::input()).await.unwrap();
        let state=read(&pool,"home").await.unwrap();
        assert_eq!(state.working_balance,"1300000");
        assert_eq!(state.cleared_balance,"2000000");
        assert_eq!(state.entries[0].balance_change,"-700000");
        let id=state.entries[0].transaction_id.clone();
        set_status(&pool,VerificationUpdate {account_id:"home".into(),entries:vec![StatusChange{transaction_id:id.clone(),expected_status:"uncleared".into()}],status:"cleared".into(),confirm_unlock:false}).await.unwrap();
        let state=read(&pool,"home").await.unwrap();
        finish(&pool,Finish{account_id:"home".into(),posted_balance:"1300000".into(),expected_token:state.token,confirm_opening_balance:true}).await.unwrap();
        let receipt:String=sqlx::query_scalar("SELECT transaction_id FROM salary_payment_transactions WHERE role='net'").fetch_one(&pool).await.unwrap();
        assert!(crate::transactions::remove(&pool,&receipt).await.is_err());
        assert_eq!(read(&pool,"home").await.unwrap().working_balance,"1300000");
        crate::transactions::remove_confirmed(&pool,&receipt,true).await.unwrap();
        let state=read(&pool,"home").await.unwrap();
        assert_eq!(state.working_balance,"2000000");assert!(state.history[0].needs_review);
        assert_eq!(state.history_entries[0].balance_change,"-700000");
    }
}
