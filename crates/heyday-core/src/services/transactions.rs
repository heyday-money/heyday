use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    #[sqlx(default)]
    #[serde(default)]
    pub imported_linked_payment: bool,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub account_name: String,
    pub destination_account_id: Option<String>,
    pub destination_account_name: Option<String>,
    pub amount: String,
    pub date: String,
    pub description: String,
    pub payee_id: Option<String>,
    pub payee_name: Option<String>,
    pub loan_account_id: Option<String>,
    pub loan_account_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub category_icon: Option<String>,
    pub income_source_id: Option<String>,
    pub income_source_name: Option<String>,
    pub has_reconciliation_history: bool,
    pub salary_payment_id: Option<String>,
    pub salary_payment_role: Option<String>,
}
impl Transaction {
    pub fn order(&self) -> (&str, &str) {
        (&self.date, &self.id)
    }
}
pub const SELECT: &str = "SELECT sp.payment_id AS salary_payment_id,sp.role AS salary_payment_role,t.id, t.type AS kind, t.account_id, a.name AS account_name, t.destination_account_id, d.name AS destination_account_name, CAST(t.amount AS TEXT) AS amount, t.date, t.description, t.payee_id, p.name AS payee_name, linked_loan.id AS loan_account_id, linked_loan.name AS loan_account_name, t.category_id, c.name AS category_name, c.icon AS category_icon, t.income_source_id, (SELECT name FROM incomes WHERE id=t.income_source_id) AS income_source_name, EXISTS(SELECT 1 FROM reconciliation_entries re WHERE re.transaction_id=t.id) AS has_reconciliation_history FROM transactions t LEFT JOIN salary_payment_transactions sp ON sp.transaction_id=t.id JOIN accounts a ON a.id = t.account_id LEFT JOIN accounts d ON d.id = t.destination_account_id LEFT JOIN payees p ON p.id = t.payee_id LEFT JOIN categories c ON c.id = t.category_id LEFT JOIN loan_payment_parts lp ON lp.transaction_id=t.id LEFT JOIN loan_contracts lc ON lc.id=lp.contract_id LEFT JOIN accounts linked_loan ON linked_loan.id=COALESCE(lp.loan_account_id,lc.account_id)";

#[derive(Serialize, sqlx::FromRow)]
pub struct ExpenseAccount {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub is_archived: bool,
}
#[derive(Serialize)]
pub struct ExpenseReport {
    pub settings: crate::Settings,
    pub accounts: Vec<ExpenseAccount>,
    pub transactions: Vec<Transaction>,
}
pub async fn expense_snapshot(pool: &SqlitePool) -> Result<ExpenseReport, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let settings = sqlx::query_as("SELECT currency,period_start_day FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    // Retain account types for archived history so payment-method filters stay accurate.
    let accounts =
        sqlx::query_as("SELECT id,name,type AS kind,is_archived FROM accounts ORDER BY name,id")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let mut transactions: Vec<Transaction> = sqlx::query_as(&format!(
        "{SELECT} WHERE t.type='expense' ORDER BY t.date DESC,t.id"
    ))
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    decorate_imported(pool, &mut transactions).await?;
    Ok(ExpenseReport {
        settings,
        accounts,
        transactions,
    })
}
pub async fn get_expense_report(pool: &SqlitePool) -> Result<ExpenseReport, String> {
    expense_snapshot(pool).await
}

pub use crate::domain::dates::valid_date;
pub use crate::domain::transactions::NewTransaction;

// Economic inflows increase assets and reduce amounts owed; outflows do the reverse.
pub async fn adjust(
    conn: &mut SqliteConnection,
    id: &str,
    inflow: i64,
    active: bool,
) -> Result<String, String> {
    adjust_wide(conn, id, i128::from(inflow), active).await
}
pub async fn adjust_wide(
    conn: &mut SqliteConnection,
    id: &str,
    inflow: i128,
    active: bool,
) -> Result<String, String> {
    let (kind, balance, archived): (String, i64, bool) =
        sqlx::query_as("SELECT type, current_balance, is_archived FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Choose an existing account.")?;
    if active && archived {
        return Err("Choose an active account.".into());
    }
    let next = crate::domain::money::adjust_balance(&kind, balance, inflow)?;
    if next != balance {
        sqlx::query(
            "UPDATE accounts SET current_balance = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(next)
        .bind(id)
        .execute(conn)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(kind)
}

pub async fn insert_in_connection(
    conn: &mut SqliteConnection,
    input: NewTransaction,
) -> Result<Transaction, String> {
    write_in_connection(conn, input, None).await
}
pub async fn write_in_connection(
    conn: &mut SqliteConnection,
    input: NewTransaction,
    existing_id: Option<&str>,
) -> Result<Transaction, String> {
    let amount = crate::domain::transactions::validate(&input)?;
    let description = input.description.trim();
    let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if input.date > today {
        return Err("Transactions cannot be dated in the future.".into());
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload transactions before saving.".into());
    }
    if input.kind != "expense" && (input.payee_id.is_some() || input.category_id.is_some()) {
        return Err("Payee and spending category apply only to expenses.".into());
    }
    let retained: Option<(Option<String>, Option<String>, Option<String>)> = if let Some(id) =
        existing_id
    {
        sqlx::query_as("SELECT payee_id,category_id,income_source_id FROM transactions WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    if let Some(id) = &input.income_source_id {
        if input.kind != "income" {
            return Err("Income source applies only to money received.".into());
        }
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM incomes i JOIN accounts a ON a.id=i.destination_account_id WHERE i.id=? AND i.is_active=1 AND a.is_archived=0)")
            .bind(id).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        if !active && retained.as_ref().and_then(|r| r.2.as_ref()) != Some(id) {
            return Err("Choose an existing, active income source.".into());
        }
    }
    for (table, id, label) in [
        ("payees", &input.payee_id, "payee"),
        ("categories", &input.category_id, "category"),
    ] {
        if let Some(id) = id {
            let active: bool = sqlx::query_scalar(&format!(
                "SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ? AND is_archived = 0)"
            ))
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            let unchanged = retained.as_ref().and_then(|r| {
                if table == "payees" {
                    r.0.as_ref()
                } else {
                    r.1.as_ref()
                }
            }) == Some(id);
            if !active && !unchanged {
                return Err(format!("Choose an existing, active {label}."));
            }
        }
    }
    for account in &input.cleared_account_ids {
        if account != &input.account_id && Some(account) != input.destination_account_id.as_ref() {
            return Err("Only participating accounts can be cleared.".into());
        }
        let supported: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('bank','wallet','credit_card'))",
        )
        .bind(account)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        if !supported {
            return Err(
                "Only bank, digital wallet, and credit card accounts support clearing.".into(),
            );
        }
    }
    if let Some(id) = existing_id {
        let old: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id=?"))
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        let old_amount = old.amount.parse::<i128>().map_err(|e| e.to_string())?;
        let mut changes = std::collections::BTreeMap::<String, i128>::new();
        *changes.entry(old.account_id).or_default() += if old.kind == "income" {
            -old_amount
        } else {
            old_amount
        };
        if let Some(destination) = old.destination_account_id {
            *changes.entry(destination).or_default() -= old_amount;
        }
        *changes.entry(input.account_id.clone()).or_default() += if input.kind == "income" {
            i128::from(amount)
        } else {
            -i128::from(amount)
        };
        if let Some(destination) = &input.destination_account_id {
            *changes.entry(destination.clone()).or_default() += i128::from(amount);
        }
        for (account, change) in changes {
            let active = account == input.account_id
                || input.destination_account_id.as_ref() == Some(&account);
            let kind = adjust_wide(conn, &account, change, active).await?;
            if input.kind == "repayment"
                && input.destination_account_id.as_ref() == Some(&account)
                && !["credit_card", "loan"].contains(&kind.as_str())
            {
                return Err("Repayments must go to a credit card or loan.".into());
            }
        }
    } else {
        adjust(
            conn,
            &input.account_id,
            if input.kind == "income" {
                amount
            } else {
                -amount
            },
            true,
        )
        .await?;
        if let Some(id) = &input.destination_account_id {
            let kind = adjust(conn, id, amount, true).await?;
            if input.kind == "repayment" && !["credit_card", "loan"].contains(&kind.as_str()) {
                return Err("Repayments must go to a credit card or loan.".into());
            }
        }
    }
    let id: String = if let Some(id) = existing_id {
        sqlx::query("UPDATE transactions SET type=?,account_id=?,destination_account_id=?,amount=?,date=?,description=?,payee_id=?,category_id=?,income_source_id=? WHERE id=?")
            .bind(&input.kind).bind(&input.account_id).bind(&input.destination_account_id).bind(amount).bind(&input.date).bind(description).bind(&input.payee_id).bind(&input.category_id).bind(&input.income_source_id).bind(id)
            .execute(&mut *conn).await.map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM transaction_verifications WHERE transaction_id=?")
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO transaction_verifications(transaction_id,account_id) SELECT id,account_id FROM transactions WHERE id=? UNION ALL SELECT id,destination_account_id FROM transactions WHERE id=? AND destination_account_id IS NOT NULL")
            .bind(id).bind(id).execute(&mut *conn).await.map_err(|e| e.to_string())?;
        id.to_owned()
    } else {
        sqlx::query_scalar("INSERT INTO transactions (id, type, account_id, destination_account_id, amount, date, description, payee_id, category_id, income_source_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(input.kind).bind(input.account_id).bind(input.destination_account_id).bind(amount).bind(input.date).bind(description).bind(input.payee_id).bind(input.category_id).bind(input.income_source_id)
        .fetch_one(&mut *conn).await.map_err(|e| e.to_string())?
    };
    for account in &input.cleared_account_ids {
        sqlx::query("UPDATE transaction_verifications SET status='cleared' WHERE transaction_id=? AND account_id=?")
            .bind(&id).bind(account).execute(&mut *conn).await.map_err(|e| e.to_string())?;
    }
    let row = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
        .bind(id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row)
}

pub async fn insert(pool: &SqlitePool, input: NewTransaction) -> Result<Transaction, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    validate_manual(&mut tx, &input).await?;
    let row = insert_in_connection(&mut tx, input).await?;
    crate::loans::validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(row)
}
pub async fn validate_manual(
    tx: &mut SqliteConnection,
    input: &NewTransaction,
) -> Result<(), String> {
    if matches!(input.kind.as_str(), "repayment" | "transfer") {
        if let Some(destination) = &input.destination_account_id {
            let has_contracts: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=?)",
            )
            .bind(destination)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            if has_contracts {
                return Err("This loan has contracts. Choose Repayment and select a contract to separate principal, interest and fees.".into());
            }
        }
    }
    if let Some(source) = &input.income_source_id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE income_id=? AND (occurrence=substr(?,1,7) OR substr(date,1,7)=substr(?,1,7)))")
            .bind(source).bind(&input.date).bind(&input.date).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
        if exists {
            return Err("A salary payment already exists for this month. Review it in Income before adding another receipt.".into());
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct UpdateTransaction {
    pub expected: Transaction,
    #[serde(flatten)]
    pub changes: NewTransaction,
}
pub async fn update(pool: &SqlitePool, input: UpdateTransaction) -> Result<Transaction, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let id = &input.expected.id;
    let old: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id=?"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Transaction no longer exists. Reload the page.")?;
    let reconciled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transaction_verifications WHERE transaction_id=? AND status='reconciled') OR EXISTS(SELECT 1 FROM reconciliation_entries WHERE transaction_id=?)")
        .bind(id).bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if reconciled {
        return Err("Transactions with reconciliation history cannot be edited.".into());
    }
    let expected = &input.expected;
    if old.kind != expected.kind
        || old.account_id != expected.account_id
        || old.destination_account_id != expected.destination_account_id
        || old.amount != expected.amount
        || old.date != expected.date
        || old.description != expected.description
        || old.payee_id != expected.payee_id
        || old.category_id != expected.category_id
        || old.income_source_id != expected.income_source_id
    {
        return Err(
            "This transaction changed. Close the editor and reload before trying again.".into(),
        );
    }
    crate::cloud_sync_ledger::protect_imported_deletion(&mut tx, id).await?;
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payment_transactions WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM loan_payment_parts WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM loan_contracts WHERE borrowing_transaction_id=?) OR EXISTS(SELECT 1 FROM installments WHERE purchase_transaction_id=?) OR EXISTS(SELECT 1 FROM ledger_sync_members WHERE transaction_id=? AND group_id != transaction_id)")
        .bind(id).bind(id).bind(id).bind(id).bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if linked {
        return Err("Linked salary, loan, and installment entries cannot be edited here. Open their source record to review them.".into());
    }
    validate_manual(&mut tx, &input.changes).await?;
    let row = write_in_connection(&mut tx, input.changes, Some(id)).await?;
    // Imported labels are fallbacks, not authoritative after an explicit edit.
    sqlx::query("DELETE FROM ledger_sync_imports WHERE group_id=(SELECT group_id FROM ledger_sync_members WHERE transaction_id=?)")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    // Any statement touched by either version must be reviewed.
    for version in [&old, &row] {
        sqlx::query("UPDATE card_statements SET needs_review=1 WHERE (account_id=? OR account_id=?) AND end_date>=?")
            .bind(&version.account_id).bind(&version.destination_account_id).bind(&version.date).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    sqlx::query("DELETE FROM card_payment_allocations WHERE transaction_id=? AND NOT EXISTS(SELECT 1 FROM transactions t JOIN accounts a ON a.id=t.account_id JOIN card_statements s ON s.id=card_payment_allocations.statement_id WHERE t.id=card_payment_allocations.transaction_id AND t.type IN ('repayment','transfer') AND a.type IN ('cash','bank','wallet') AND t.destination_account_id=s.account_id)")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    crate::loans::validate_allocations(&mut tx).await?;
    sqlx::query("INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=? AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(row)
}
pub async fn update_transaction(
    pool: &SqlitePool,
    input: UpdateTransaction,
) -> Result<Transaction, String> {
    update(pool, input).await
}

pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    remove_confirmed(pool, id, false).await
}

pub async fn remove_confirmed(pool: &SqlitePool, id: &str, confirmed: bool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    remove_in_connection(&mut tx, id, confirmed).await?;
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn remove_in_connection(
    conn: &mut SqliteConnection,
    id: &str,
    confirmed: bool,
) -> Result<(), String> {
    crate::cloud_sync_ledger::protect_imported_deletion(conn, id).await?;
    let group = crate::salary_payments::group(conn, id).await?;
    let ids = if let Some(group) = &group {
        sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE payment_id=?",
        )
        .bind(group)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?
    } else {
        crate::loans::deletion_ids(conn, id).await?
    };
    for id in &ids {
        crate::loans::before_delete(conn, id).await?;
        let row: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Transaction no longer exists. Reload the page.")?;
        crate::reconciliation::before_delete(conn, id, confirmed).await?;
        let amount = row.amount.parse::<i64>().map_err(|e| e.to_string())?;
        adjust(
            conn,
            &row.account_id,
            if row.kind == "income" {
                -amount
            } else {
                amount
            },
            false,
        )
        .await?;
        if let Some(destination) = row.destination_account_id {
            adjust(conn, &destination, -amount, false).await?;
        }
        sqlx::query("DELETE FROM transactions WHERE id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(group) = group {
        sqlx::query("DELETE FROM salary_payments WHERE id=?")
            .bind(group)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    crate::loans::validate_allocations(conn).await?;
    Ok(())
}

pub async fn create_transaction(
    pool: &SqlitePool,
    input: NewTransaction,
) -> Result<Transaction, String> {
    insert(pool, input).await
}
pub async fn delete_transaction(
    pool: &SqlitePool,
    id: String,
    confirm_reconciled: Option<bool>,
) -> Result<(), String> {
    remove_confirmed(pool, &id, confirm_reconciled.unwrap_or(false)).await
}
pub async fn list_transactions(pool: &SqlitePool) -> Result<Vec<Transaction>, String> {
    let mut rows: Vec<Transaction> = sqlx::query_as(&format!(
        "{SELECT} ORDER BY t.date DESC,t.created_at DESC,t.id DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    decorate_imported(pool, &mut rows).await?;
    Ok(rows)
}

pub async fn decorate_imported(pool: &SqlitePool, rows: &mut [Transaction]) -> Result<(), String> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(|_| crate::database::ledger::DATABASE)?;
    decorate_in_connection(&mut conn, rows).await
}
pub async fn decorate_in_connection(
    conn: &mut SqliteConnection,
    rows: &mut [Transaction],
) -> Result<(), String> {
    let entries = crate::cloud_sync_ledger::imported_entries(&mut *conn).await?;
    let accounts: Vec<(String, String)> = sqlx::query_as("SELECT id,name FROM accounts")
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| crate::database::ledger::DATABASE)?;
    let names = accounts
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    for row in rows {
        if let Some((protected, entry)) = entries.get(&row.id) {
            row.imported_linked_payment = *protected;
            if row.income_source_name.is_none() {
                if let Some(source) = &entry.income_source {
                    row.income_source_id = Some(source.id.clone());
                    row.income_source_name = Some(source.name.clone());
                }
            }
            if row.loan_account_id.is_none() {
                row.loan_account_id = entry.loan_account_id.clone();
                row.loan_account_name = entry
                    .loan_account_id
                    .as_ref()
                    .and_then(|id| names.get(id).cloned());
            }
            if row.salary_payment_role.is_none() {
                row.salary_payment_role = entry.salary_role.clone();
            }
        }
    }
    Ok(())
}
