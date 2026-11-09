use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    #[sqlx(default)]
    #[serde(default)]
    imported_linked_payment: bool,
    pub(crate) id: String,
    #[serde(rename = "type")]
    kind: String,
    account_id: String,
    account_name: String,
    destination_account_id: Option<String>,
    destination_account_name: Option<String>,
    amount: String,
    date: String,
    description: String,
    payee_id: Option<String>,
    payee_name: Option<String>,
    loan_account_id: Option<String>,
    loan_account_name: Option<String>,
    category_id: Option<String>,
    category_name: Option<String>,
    category_icon: Option<String>,
    income_source_id: Option<String>,
    income_source_name: Option<String>,
    has_reconciliation_history: bool,
    salary_payment_id: Option<String>,
    salary_payment_role: Option<String>,
}
impl Transaction {
    pub(crate) fn order(&self)->(&str,&str){(&self.date,&self.id)}
}
pub(crate) const SELECT: &str = "SELECT sp.payment_id AS salary_payment_id,sp.role AS salary_payment_role,t.id, t.type AS kind, t.account_id, a.name AS account_name, t.destination_account_id, d.name AS destination_account_name, CAST(t.amount AS TEXT) AS amount, t.date, t.description, t.payee_id, p.name AS payee_name, linked_loan.id AS loan_account_id, linked_loan.name AS loan_account_name, t.category_id, c.name AS category_name, c.icon AS category_icon, t.income_source_id, (SELECT name FROM incomes WHERE id=t.income_source_id) AS income_source_name, EXISTS(SELECT 1 FROM reconciliation_entries re WHERE re.transaction_id=t.id) AS has_reconciliation_history FROM transactions t LEFT JOIN salary_payment_transactions sp ON sp.transaction_id=t.id JOIN accounts a ON a.id = t.account_id LEFT JOIN accounts d ON d.id = t.destination_account_id LEFT JOIN payees p ON p.id = t.payee_id LEFT JOIN categories c ON c.id = t.category_id LEFT JOIN loan_payment_parts lp ON lp.transaction_id=t.id LEFT JOIN loan_contracts lc ON lc.id=lp.contract_id LEFT JOIN accounts linked_loan ON linked_loan.id=COALESCE(lp.loan_account_id,lc.account_id)";

#[derive(Serialize, sqlx::FromRow)]
pub struct ExpenseAccount {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    is_archived: bool,
}
#[derive(Serialize)]
pub struct ExpenseReport {
    settings: crate::Settings,
    accounts: Vec<ExpenseAccount>,
    transactions: Vec<Transaction>,
}
async fn expense_snapshot(pool: &SqlitePool) -> Result<ExpenseReport, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let settings = sqlx::query_as("SELECT currency,period_start_day FROM settings WHERE id=1")
        .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    // Retain account types for archived history so payment-method filters stay accurate.
    let accounts = sqlx::query_as("SELECT id,name,type AS kind,is_archived FROM accounts ORDER BY name,id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    let mut transactions:Vec<Transaction> = sqlx::query_as(&format!("{SELECT} WHERE t.type='expense' ORDER BY t.date DESC,t.id"))
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    decorate_imported(pool,&mut transactions).await?;
    Ok(ExpenseReport { settings, accounts, transactions })
}
#[tauri::command]
pub async fn get_expense_report(pool: tauri::State<'_, SqlitePool>) -> Result<ExpenseReport, String> {
    let _database_operation = crate::backups::operation()?;
    expense_snapshot(pool.inner()).await
}

#[derive(Deserialize)]
pub struct NewTransaction {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) account_id: String,
    pub(crate) destination_account_id: Option<String>,
    pub(crate) amount: String,
    pub(crate) date: String,
    pub(crate) description: String,
    pub(crate) currency: String,
    pub(crate) payee_id: Option<String>,
    pub(crate) category_id: Option<String>,
    pub(crate) income_source_id: Option<String>,
    #[serde(default)]
    pub(crate) cleared_account_ids: Vec<String>,
}

pub(crate) fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap();
    let month: usize = date[5..7].parse().unwrap();
    let day: u32 = date[8..].parse().unwrap();
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day >= 1 && day <= days[month - 1]
}

// Economic inflows increase assets and reduce amounts owed; outflows do the reverse.
async fn adjust(
    conn: &mut SqliteConnection,
    id: &str,
    inflow: i64,
    active: bool,
) -> Result<String, String> {
    adjust_wide(conn, id, i128::from(inflow), active).await
}
async fn adjust_wide(conn: &mut SqliteConnection, id: &str, inflow: i128, active: bool) -> Result<String, String> {
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
    let debt = ["credit_card", "loan"].contains(&kind.as_str());
    let next = i64::try_from(if debt { i128::from(balance) - inflow } else { i128::from(balance) + inflow })
        .map_err(|_| "This transaction would put an account balance outside the supported range.")?;
    if next != balance { sqlx::query(
        "UPDATE accounts SET current_balance = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(next)
    .bind(id)
    .execute(conn)
    .await
    .map_err(|e| e.to_string())?; }
    Ok(kind)
}

pub(crate) async fn insert_in_connection(conn: &mut SqliteConnection, input: NewTransaction) -> Result<Transaction, String> {
    write_in_connection(conn, input, None).await
}
async fn write_in_connection(
    conn: &mut SqliteConnection,
    input: NewTransaction,
    existing_id: Option<&str>,
) -> Result<Transaction, String> {
    if !["income", "expense", "transfer", "repayment"].contains(&input.kind.as_str()) {
        return Err("Choose a transaction type.".into());
    }
    let amount = input
        .amount
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if amount <= 0 {
        return Err("Amount must be greater than zero.".into());
    }
    if !valid_date(&input.date) {
        return Err("Enter a valid date.".into());
    }
    let description = input.description.trim();
    if description.chars().count() > 200 {
        return Err("Description must be no longer than 200 characters.".into());
    }
    let paired = ["transfer", "repayment"].contains(&input.kind.as_str());
    if paired != input.destination_account_id.is_some()
        || input.destination_account_id.as_ref() == Some(&input.account_id)
    {
        return Err("Transfers and repayments need two different accounts.".into());
    }
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
    let retained: Option<(Option<String>, Option<String>, Option<String>)> = if let Some(id) = existing_id {
        sqlx::query_as("SELECT payee_id,category_id,income_source_id FROM transactions WHERE id=?").bind(id).fetch_optional(&mut *conn).await.map_err(|e| e.to_string())?
    } else { None };
    if let Some(id) = &input.income_source_id {
        if input.kind != "income" { return Err("Income source applies only to money received.".into()); }
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM incomes i JOIN accounts a ON a.id=i.destination_account_id WHERE i.id=? AND i.is_active=1 AND a.is_archived=0)")
            .bind(id).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        if !active && retained.as_ref().and_then(|r| r.2.as_ref()) != Some(id) { return Err("Choose an existing, active income source.".into()); }
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
            let unchanged = retained.as_ref().and_then(|r| if table == "payees" { r.0.as_ref() } else { r.1.as_ref() }) == Some(id);
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
        let old: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id=?")).bind(id).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        let old_amount = old.amount.parse::<i128>().map_err(|e| e.to_string())?;
        let mut changes = std::collections::BTreeMap::<String, i128>::new();
        *changes.entry(old.account_id).or_default() += if old.kind == "income" { -old_amount } else { old_amount };
        if let Some(destination) = old.destination_account_id { *changes.entry(destination).or_default() -= old_amount; }
        *changes.entry(input.account_id.clone()).or_default() += if input.kind == "income" { i128::from(amount) } else { -i128::from(amount) };
        if let Some(destination) = &input.destination_account_id { *changes.entry(destination.clone()).or_default() += i128::from(amount); }
        for (account, change) in changes {
            let active = account == input.account_id || input.destination_account_id.as_ref() == Some(&account);
            let kind = adjust_wide(conn, &account, change, active).await?;
            if input.kind == "repayment" && input.destination_account_id.as_ref() == Some(&account) && !["credit_card", "loan"].contains(&kind.as_str()) {
                return Err("Repayments must go to a credit card or loan.".into());
            }
        }
    } else {
        adjust(conn, &input.account_id, if input.kind == "income" { amount } else { -amount }, true).await?;
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
        sqlx::query("DELETE FROM transaction_verifications WHERE transaction_id=?").bind(id).execute(&mut *conn).await.map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO transaction_verifications(transaction_id,account_id) SELECT id,account_id FROM transactions WHERE id=? UNION ALL SELECT id,destination_account_id FROM transactions WHERE id=? AND destination_account_id IS NOT NULL")
            .bind(id).bind(id).execute(&mut *conn).await.map_err(|e| e.to_string())?;
        id.to_owned()
    } else { sqlx::query_scalar("INSERT INTO transactions (id, type, account_id, destination_account_id, amount, date, description, payee_id, category_id, income_source_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(input.kind).bind(input.account_id).bind(input.destination_account_id).bind(amount).bind(input.date).bind(description).bind(input.payee_id).bind(input.category_id).bind(input.income_source_id)
        .fetch_one(&mut *conn).await.map_err(|e| e.to_string())? };
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

pub(crate) async fn insert(
    pool: &SqlitePool,
    input: NewTransaction,
) -> Result<Transaction, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    validate_manual(&mut tx, &input).await?;
    let row = insert_in_connection(&mut tx, input).await?;
    crate::loans::validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(row)
}
async fn validate_manual(tx: &mut SqliteConnection, input: &NewTransaction) -> Result<(), String> {
    if matches!(input.kind.as_str(), "repayment" | "transfer") {
        if let Some(destination) = &input.destination_account_id {
            let has_contracts: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=?)")
                .bind(destination).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
            if has_contracts {
                return Err("This loan has contracts. Choose Repayment and select a contract to separate principal, interest and fees.".into());
            }
        }
    }
    if let Some(source) = &input.income_source_id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE income_id=? AND (occurrence=substr(?,1,7) OR substr(date,1,7)=substr(?,1,7)))")
            .bind(source).bind(&input.date).bind(&input.date).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
        if exists { return Err("A salary payment already exists for this month. Review it in Income before adding another receipt.".into()); }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct UpdateTransaction {
    expected: Transaction,
    #[serde(flatten)]
    changes: NewTransaction,
}
async fn update(pool: &SqlitePool, input: UpdateTransaction) -> Result<Transaction, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let id = &input.expected.id;
    let old: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id=?")).bind(id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?.ok_or("Transaction no longer exists. Reload the page.")?;
    let reconciled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transaction_verifications WHERE transaction_id=? AND status='reconciled') OR EXISTS(SELECT 1 FROM reconciliation_entries WHERE transaction_id=?)")
        .bind(id).bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if reconciled { return Err("Transactions with reconciliation history cannot be edited.".into()); }
    let expected = &input.expected;
    if old.kind != expected.kind || old.account_id != expected.account_id || old.destination_account_id != expected.destination_account_id || old.amount != expected.amount || old.date != expected.date || old.description != expected.description || old.payee_id != expected.payee_id || old.category_id != expected.category_id || old.income_source_id != expected.income_source_id {
        return Err("This transaction changed. Close the editor and reload before trying again.".into());
    }
    crate::cloud_sync_ledger::protect_imported_deletion(&mut tx, id).await?;
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payment_transactions WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM loan_payment_parts WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM loan_contracts WHERE borrowing_transaction_id=?) OR EXISTS(SELECT 1 FROM installments WHERE purchase_transaction_id=?) OR EXISTS(SELECT 1 FROM ledger_sync_members WHERE transaction_id=? AND group_id != transaction_id)")
        .bind(id).bind(id).bind(id).bind(id).bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if linked { return Err("Linked salary, loan, and installment entries cannot be edited here. Open their source record to review them.".into()); }
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
#[tauri::command]
pub async fn update_transaction(pool: tauri::State<'_, SqlitePool>, input: UpdateTransaction) -> Result<Transaction, String> {
    let _database_operation = crate::backups::operation()?;
    update(pool.inner(), input).await
}

#[cfg(test)]
pub(crate) async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    remove_confirmed(pool, id, false).await
}

pub(crate) async fn remove_confirmed(
    pool: &SqlitePool,
    id: &str,
    confirmed: bool,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    remove_in_connection(&mut tx,id,confirmed).await?;
    tx.commit().await.map_err(|e|e.to_string())
}
pub(crate) async fn remove_in_connection(conn:&mut SqliteConnection,id:&str,confirmed:bool)->Result<(),String> {
    crate::cloud_sync_ledger::protect_imported_deletion(conn, id).await?;
    let group = crate::salary_payments::group(conn, id).await?;
    let ids = if let Some(group) = &group {
        sqlx::query_scalar("SELECT transaction_id FROM salary_payment_transactions WHERE payment_id=?").bind(group).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?
    } else { crate::loans::deletion_ids(conn, id).await? };
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
        sqlx::query("DELETE FROM salary_payments WHERE id=?").bind(group).execute(&mut *conn).await.map_err(|e|e.to_string())?;
    }
    crate::loans::validate_allocations(conn).await?;
    Ok(())
}

#[tauri::command]
pub async fn create_transaction(
    pool: tauri::State<'_, SqlitePool>,
    input: NewTransaction,
) -> Result<Transaction, String> {
    let _database_operation = crate::backups::operation()?;
    insert(pool.inner(), input).await
}
#[tauri::command]
pub async fn delete_transaction(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
    confirm_reconciled: Option<bool>,
) -> Result<(), String> {
    let _database_operation = crate::backups::operation()?;
    remove_confirmed(pool.inner(), &id, confirm_reconciled.unwrap_or(false)).await
}
#[tauri::command]
pub async fn list_transactions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Transaction>, String> {
    let _database_operation = crate::backups::operation()?;
    let mut rows:Vec<Transaction>=sqlx::query_as(&format!("{SELECT} ORDER BY t.date DESC,t.created_at DESC,t.id DESC"))
        .fetch_all(pool.inner()).await.map_err(|e|e.to_string())?;
    decorate_imported(pool.inner(),&mut rows).await?;
    Ok(rows)
}

async fn decorate_imported(pool:&SqlitePool,rows:&mut [Transaction])->Result<(),String>{
    let mut conn=pool.acquire().await.map_err(|_|crate::cloud_sync_store::DATABASE)?;
    decorate_in_connection(&mut conn,rows).await
}
pub(crate) async fn decorate_in_connection(conn:&mut SqliteConnection,rows:&mut [Transaction])->Result<(),String>{
    let entries=crate::cloud_sync_ledger::imported_entries(&mut *conn).await?;
    let accounts:Vec<(String,String)>=sqlx::query_as("SELECT id,name FROM accounts").fetch_all(&mut *conn).await.map_err(|_|crate::cloud_sync_store::DATABASE)?;
    let names=accounts.into_iter().collect::<std::collections::HashMap<_,_>>();
    for row in rows{
        if let Some((protected,entry))=entries.get(&row.id){
            row.imported_linked_payment=*protected;
            if row.income_source_name.is_none(){if let Some(source)=&entry.income_source{row.income_source_id=Some(source.id.clone());row.income_source_name=Some(source.name.clone());}}
            if row.loan_account_id.is_none(){row.loan_account_id=entry.loan_account_id.clone();row.loan_account_name=entry.loan_account_id.as_ref().and_then(|id|names.get(id).cloned());}
            if row.salary_payment_role.is_none(){row.salary_payment_role=entry.salary_role.clone();}
        }
    }Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn editable_database() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
        sqlx::raw_sql(include_str!("../migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
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
    #[test]
    fn calendar_validation() {
        for date in ["2024-02-29", "2000-02-29", "2025-01-31", "2025-04-30"] {
            assert!(valid_date(date));
        }
        for date in [
            "1900-02-29",
            "2025-02-29",
            "2024-04-31",
            "0000-01-01",
            "2025-13-01",
            "2025-01-00",
            "2025-1-01",
            "abcd-ef-gh",
            "😀2024-01",
        ] {
            assert!(!valid_date(date));
        }
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
        sqlx::raw_sql(include_str!("../migrations/0002_salary_payments.sql")).execute(&pool).await.unwrap();
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
