use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Entry {
    transaction_id: String,
    date: String,
    description: String,
    #[serde(rename = "type")]
    kind: String,
    amount: String,
    account_id: String,
    destination_account_id: Option<String>,
    status: String,
    reconciliation_id: Option<String>,
    balance_change: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct History {
    id: String,
    completed_at: String,
    confirmed_balance: String,
    opening_balance: String,
    needs_review: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct HistoryEntry {
    reconciliation_id: String,
    transaction_id: String,
    date: String,
    description: String,
    #[serde(rename = "type")]
    kind: String,
    balance_change: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    account_id: String,
    name: String,
    account_type: String,
    is_archived: bool,
    currency: String,
    opening_balance: String,
    working_balance: String,
    cleared_balance: String,
    uncleared_changes: String,
    entries: Vec<Entry>,
    history: Vec<History>,
    history_entries: Vec<HistoryEntry>,
    token: String,
}

async fn snapshot(conn: &mut SqliteConnection, id: &str) -> Result<Snapshot, String> {
    let (name, kind, opening, working, archived): (String, String, i64, i64, bool) = sqlx::query_as(
        "SELECT name, type, opening_balance, current_balance, is_archived FROM accounts WHERE id = ?",
    ).bind(id).fetch_optional(&mut *conn).await.map_err(|e| e.to_string())?
        .ok_or("Account no longer exists.")?;
    if !["bank", "wallet", "credit_card"].contains(&kind.as_str()) {
        return Err(
            "Reconciliation is available for bank, digital wallet, and credit card accounts."
                .into(),
        );
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let mut entries: Vec<Entry> = sqlx::query_as("SELECT t.id AS transaction_id,t.date,t.description,t.type AS kind,CAST(t.amount AS TEXT) AS amount,t.account_id,t.destination_account_id,v.status,CAST(v.reconciliation_id AS TEXT) AS reconciliation_id,'' AS balance_change FROM transactions t JOIN transaction_verifications v ON v.transaction_id=t.id WHERE v.account_id=? ORDER BY t.date DESC,t.created_at DESC,t.id DESC")
        .bind(id).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let mut uncleared = 0i128;
    for entry in &mut entries {
        let amount = entry.amount.parse::<i64>().map_err(|e| e.to_string())?;
        let inflow =
            if entry.destination_account_id.as_deref() == Some(id) || entry.kind == "income" {
                amount
            } else {
                -amount
            };
        let change = if kind == "credit_card" {
            -inflow
        } else {
            inflow
        };
        entry.balance_change = change.to_string();
        if entry.status == "uncleared" {
            uncleared += i128::from(change);
        }
    }
    let history = sqlx::query_as("SELECT CAST(id AS TEXT) AS id,completed_at,CAST(confirmed_balance AS TEXT) AS confirmed_balance,CAST(opening_balance AS TEXT) AS opening_balance,needs_review FROM reconciliations WHERE account_id=? ORDER BY id DESC")
        .bind(id).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let history_entries = sqlx::query_as("SELECT CAST(e.reconciliation_id AS TEXT) AS reconciliation_id,e.transaction_id,e.date,e.description,e.type AS kind,CAST(e.balance_change AS TEXT) AS balance_change FROM reconciliation_entries e JOIN reconciliations r ON r.id=e.reconciliation_id WHERE r.account_id=? ORDER BY r.id DESC,e.transaction_id")
        .bind(id).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let mut result = Snapshot {
        account_id: id.into(),
        name,
        account_type: kind,
        is_archived: archived,
        currency: currency.ok_or("Choose a currency in Settings first.")?,
        opening_balance: opening.to_string(),
        working_balance: working.to_string(),
        cleared_balance: (i128::from(working) - uncleared).to_string(),
        uncleared_changes: uncleared.to_string(),
        entries,
        history,
        history_entries,
        token: String::new(),
    };
    // The comparison includes every participating entry, status and completed history.
    // It is opaque to the client, and is always reconstructed inside the save transaction.
    result.token = serde_json::to_string(&result).map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
pub async fn get_reconciliation(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<Snapshot, String> {
    read(pool.inner(), &account_id).await
}
async fn read(pool: &SqlitePool, account: &str) -> Result<Snapshot, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = snapshot(&mut tx, account).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}

#[derive(Deserialize)]
pub struct StatusChange {
    transaction_id: String,
    expected_status: String,
}
#[derive(Deserialize)]
pub struct VerificationUpdate {
    account_id: String,
    entries: Vec<StatusChange>,
    status: String,
    #[serde(default)]
    confirm_unlock: bool,
}

// A corrected early entry also invalidates later confirmations which carried it forward.
async fn mark_review(
    conn: &mut SqliteConnection,
    transaction: &str,
    account: Option<&str>,
) -> Result<(), String> {
    sqlx::query("UPDATE reconciliations AS r SET needs_review=1 WHERE (? IS NULL OR r.account_id=?) AND r.id >= (SELECT min(old.id) FROM reconciliations old JOIN reconciliation_entries e ON e.reconciliation_id=old.id WHERE old.account_id=r.account_id AND e.transaction_id=?)")
        .bind(account).bind(account).bind(transaction).execute(conn).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn set_status(pool: &SqlitePool, input: VerificationUpdate) -> Result<(), String> {
    if !["uncleared", "cleared"].contains(&input.status.as_str()) || input.entries.is_empty() {
        return Err("Select transactions and choose Uncleared or Cleared.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let available: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('bank','wallet','credit_card') AND is_archived=0)")
        .bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if !available {
        return Err("Choose an active bank, digital wallet, or credit card account.".into());
    }
    for entry in input.entries {
        let status: String = sqlx::query_scalar(
            "SELECT status FROM transaction_verifications WHERE transaction_id=? AND account_id=?",
        )
        .bind(&entry.transaction_id)
        .bind(&input.account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Transaction is no longer available for this account. Refresh and try again.")?;
        if status != entry.expected_status {
            return Err(
                "Verification changed. Refresh and review the transactions before trying again."
                    .into(),
            );
        }
        if status == "reconciled" {
            if !input.confirm_unlock {
                return Err("Confirm unlocking this reconciled transaction. Affected reconciliation history will need review.".into());
            }
            mark_review(&mut tx, &entry.transaction_id, Some(&input.account_id)).await?;
        }
        sqlx::query("UPDATE transaction_verifications SET status=?,reconciliation_id=NULL WHERE transaction_id=? AND account_id=?")
            .bind(&input.status).bind(&entry.transaction_id).bind(&input.account_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn set_transaction_verification(
    pool: tauri::State<'_, SqlitePool>,
    input: VerificationUpdate,
) -> Result<(), String> {
    set_status(pool.inner(), input).await
}

#[derive(Deserialize)]
pub struct Finish {
    account_id: String,
    posted_balance: String,
    expected_token: String,
    #[serde(default)]
    confirm_opening_balance: bool,
}
async fn finish(pool: &SqlitePool, input: Finish) -> Result<Snapshot, String> {
    let posted = input
        .posted_balance
        .parse::<i64>()
        .map_err(|_| "Enter a posted balance within the supported range.")?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let current = snapshot(&mut tx, &input.account_id).await?;
    if current.is_archived {
        return Err("Archived accounts cannot be reconciled.".into());
    }
    if current.token != input.expected_token {
        return Err("Account activity or verification changed. Refresh and review the new comparison before finishing.".into());
    }
    if current.history.is_empty() && !input.confirm_opening_balance {
        return Err("Confirm the opening balance before the first reconciliation.".into());
    }
    if i128::from(posted).to_string() != current.cleared_balance {
        return Err("The posted balance must match the cleared balance before finishing.".into());
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO reconciliations(account_id,confirmed_balance,opening_balance) VALUES (?,?,?) RETURNING id")
        .bind(&input.account_id).bind(posted).bind(current.opening_balance.parse::<i64>().map_err(|e| e.to_string())?)
        .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    for entry in current
        .entries
        .iter()
        .filter(|entry| entry.status == "cleared")
    {
        sqlx::query("INSERT INTO reconciliation_entries(reconciliation_id,transaction_id,date,description,type,balance_change) VALUES (?,?,?,?,?,?)")
            .bind(id).bind(&entry.transaction_id).bind(&entry.date).bind(&entry.description).bind(&entry.kind)
            .bind(entry.balance_change.parse::<i64>().map_err(|e| e.to_string())?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query("UPDATE transaction_verifications SET status='reconciled',reconciliation_id=? WHERE transaction_id=? AND account_id=? AND status='cleared'")
            .bind(id).bind(&entry.transaction_id).bind(&input.account_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    let result = snapshot(&mut tx, &input.account_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
#[tauri::command]
pub async fn finish_reconciliation(
    pool: tauri::State<'_, SqlitePool>,
    input: Finish,
) -> Result<Snapshot, String> {
    finish(pool.inner(), input).await
}

pub(crate) async fn before_delete(
    conn: &mut SqliteConnection,
    transaction: &str,
    confirmed: bool,
) -> Result<(), String> {
    let recorded: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reconciliation_entries WHERE transaction_id=?)",
    )
    .bind(transaction)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    if recorded && !confirmed {
        return Err("This transaction has reconciliation history. Confirm deletion and mark affected reconciliations as needing review.".into());
    }
    if recorded {
        mark_review(conn, transaction, None).await?;
    }
    Ok(())
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 22)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        seed(&pool).await;
        sqlx::raw_sql("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES ('legacy','transfer','bank','card',500,'2026-01-01','Old payment'); UPDATE accounts SET current_balance=current_balance-500 WHERE id IN ('bank','card');").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0022_reconciliation.sql"))
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
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
        std::fs::remove_file(path).unwrap();
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
}
