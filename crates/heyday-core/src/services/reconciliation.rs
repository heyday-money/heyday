use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Entry {
    #[sqlx(default)]
    pub imported_linked_payment: bool,
    pub salary_payment_id: Option<String>,
    pub salary_payment_role: Option<String>,
    pub transaction_id: String,
    pub date: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub amount: String,
    pub account_id: String,
    pub destination_account_id: Option<String>,
    pub status: String,
    pub reconciliation_id: Option<String>,
    pub balance_change: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct History {
    pub id: String,
    pub completed_at: String,
    pub confirmed_balance: String,
    pub opening_balance: String,
    pub needs_review: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct HistoryEntry {
    pub reconciliation_id: String,
    pub transaction_id: String,
    pub date: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub balance_change: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub account_id: String,
    pub name: String,
    pub account_type: String,
    pub is_archived: bool,
    pub currency: String,
    pub opening_balance: String,
    pub working_balance: String,
    pub cleared_balance: String,
    pub uncleared_changes: String,
    pub entries: Vec<Entry>,
    pub history: Vec<History>,
    pub history_entries: Vec<HistoryEntry>,
    pub token: String,
}

pub async fn snapshot(conn: &mut SqliteConnection, id: &str) -> Result<Snapshot, String> {
    let (name, kind, opening, working, archived): (String, String, i64, i64, bool) = sqlx::query_as(
        "SELECT name, type, opening_balance, current_balance, is_archived FROM accounts WHERE id = ?",
    ).bind(id).fetch_optional(&mut *conn).await.map_err(|e| e.to_string())?
        .ok_or("Account no longer exists.")?;
    if !["bank", "wallet", "credit_card", "loan"].contains(&kind.as_str()) {
        return Err(
            "Reconciliation is available for bank, digital wallet, and credit card accounts."
                .into(),
        );
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let mut entries: Vec<Entry> = sqlx::query_as("SELECT (SELECT payment_id FROM salary_payment_transactions WHERE transaction_id=t.id) salary_payment_id,(SELECT role FROM salary_payment_transactions WHERE transaction_id=t.id) salary_payment_role,t.id AS transaction_id,t.date,t.description,t.type AS kind,CAST(t.amount AS TEXT) AS amount,t.account_id,t.destination_account_id,v.status,CAST(v.reconciliation_id AS TEXT) AS reconciliation_id,'' AS balance_change FROM transactions t JOIN transaction_verifications v ON v.transaction_id=t.id WHERE v.account_id=? ORDER BY t.date DESC,t.created_at DESC,t.id DESC")
        .bind(id).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let mut uncleared = 0i128;
    let imported = crate::cloud_sync_ledger::imported_entries(&mut *conn).await?;
    for entry in &mut entries {
        if let Some((protected, source)) = imported.get(&entry.transaction_id) {
            entry.imported_linked_payment = *protected;
            if entry.salary_payment_role.is_none() {
                entry.salary_payment_role = source.salary_role.clone();
            }
        }
        let amount = entry.amount.parse::<i64>().map_err(|e| e.to_string())?;
        let inflow =
            if entry.destination_account_id.as_deref() == Some(id) || entry.kind == "income" {
                amount
            } else {
                -amount
            };
        let change = if kind == "credit_card" || kind == "loan" {
            -inflow
        } else {
            inflow
        };
        entry.balance_change = change.to_string();
        if entry.status == "uncleared" {
            uncleared += i128::from(change);
        }
    }
    let history = sqlx::query_as("SELECT CAST(id AS TEXT) AS id,completed_at,CAST(confirmed_balance AS TEXT) AS confirmed_balance,CAST(opening_balance AS TEXT) AS opening_balance,needs_review FROM reconciliations WHERE account_id=? ORDER BY completed_at DESC,id DESC")
        .bind(id).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let history_entries = sqlx::query_as("SELECT CAST(e.reconciliation_id AS TEXT) AS reconciliation_id,e.transaction_id,e.date,e.description,e.type AS kind,CAST(e.balance_change AS TEXT) AS balance_change FROM reconciliation_entries e JOIN reconciliations r ON r.id=e.reconciliation_id WHERE r.account_id=? ORDER BY r.completed_at DESC,r.id DESC,e.transaction_id")
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

pub async fn get_reconciliation(pool: &SqlitePool, account_id: String) -> Result<Snapshot, String> {
    read(pool, &account_id).await
}
pub async fn read(pool: &SqlitePool, account: &str) -> Result<Snapshot, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = snapshot(&mut tx, account).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}

#[derive(Deserialize)]
pub struct StatusChange {
    pub transaction_id: String,
    pub expected_status: String,
}
#[derive(Deserialize)]
pub struct VerificationUpdate {
    pub account_id: String,
    pub entries: Vec<StatusChange>,
    pub status: String,
    #[serde(default)]
    pub confirm_unlock: bool,
}

// A corrected early entry also invalidates later confirmations which carried it forward.
pub async fn mark_review(
    conn: &mut SqliteConnection,
    transaction: &str,
    account: Option<&str>,
) -> Result<(), String> {
    sqlx::query("UPDATE reconciliations AS r SET needs_review=1 WHERE (? IS NULL OR r.account_id=?) AND r.completed_at >= (SELECT min(old.completed_at) FROM reconciliations old JOIN reconciliation_entries e ON e.reconciliation_id=old.id WHERE old.account_id=r.account_id AND e.transaction_id=?)")
        .bind(account).bind(account).bind(transaction).execute(conn).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn set_status(pool: &SqlitePool, input: VerificationUpdate) -> Result<(), String> {
    if !["uncleared", "cleared"].contains(&input.status.as_str()) || input.entries.is_empty() {
        return Err("Select transactions and choose Uncleared or Cleared.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let available: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('bank','wallet','credit_card','loan') AND is_archived=0)")
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
pub async fn set_transaction_verification(
    pool: &SqlitePool,
    input: VerificationUpdate,
) -> Result<(), String> {
    set_status(pool, input).await
}

#[derive(Deserialize)]
pub struct Finish {
    pub account_id: String,
    pub posted_balance: String,
    pub expected_token: String,
    #[serde(default)]
    pub confirm_opening_balance: bool,
}
pub async fn finish(pool: &SqlitePool, input: Finish) -> Result<Snapshot, String> {
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
pub async fn finish_reconciliation(pool: &SqlitePool, input: Finish) -> Result<Snapshot, String> {
    finish(pool, input).await
}

pub async fn before_delete(
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
