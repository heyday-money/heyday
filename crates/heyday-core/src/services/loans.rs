use crate::transactions::{insert_in_connection, valid_date, NewTransaction};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
pub fn err(e: sqlx::Error) -> String {
    e.to_string()
}
pub fn amount(s: &str) -> Result<i64, String> {
    let n = s
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if n < 0 {
        return Err("Amounts cannot be negative.".into());
    }
    Ok(n)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Contract {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub principal: String,
    pub remaining_principal: String,
    pub borrowing_date: String,
    pub receiving_account_id: String,
    pub payment_account_id: String,
    pub interest_rate: i64,
    pub monthly_amount: String,
    pub installment_count: i64,
    pub first_due_date: String,
    pub schedule_end: Option<String>,
    pub notes: String,
    pub borrowing_kind: String,
    pub borrowing_transaction_id: Option<String>,
    pub needs_review: bool,
    pub accounts_available: bool,
}
pub const SELECT: &str = "SELECT c.id,c.account_id,c.name,CAST(c.principal AS TEXT) AS principal,CAST(c.remaining_principal AS TEXT) AS remaining_principal,c.borrowing_date,c.receiving_account_id,c.payment_account_id,c.interest_rate,CAST(c.monthly_amount AS TEXT) AS monthly_amount,c.installment_count,c.first_due_date,c.schedule_end,c.notes,c.borrowing_kind,c.borrowing_transaction_id,c.needs_review,(a.is_archived=0 AND p.is_archived=0 AND p.type IN ('cash','bank')) AS accounts_available FROM loan_contracts c JOIN accounts a ON a.id=c.account_id JOIN accounts p ON p.id=c.payment_account_id";
pub async fn contracts(conn: &mut SqliteConnection) -> Result<Vec<Contract>, sqlx::Error> {
    sqlx::query_as(SELECT).fetch_all(conn).await
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Facility {
    pub account_id: String,
    pub credit_limit: String,
}
pub async fn facilities(conn: &mut SqliteConnection) -> Result<Vec<Facility>, sqlx::Error> {
    sqlx::query_as(
        "SELECT account_id,CAST(credit_limit AS TEXT) AS credit_limit FROM loan_facilities",
    )
    .fetch_all(conn)
    .await
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PaymentPart {
    pub transaction_id: String,
    pub payment_id: String,
    pub contract_id: Option<String>,
    pub component: String,
    pub amount: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub paid_off_on: Option<String>,
    pub account: crate::accounts::Account,
    pub currency: Option<String>,
    pub facility: Option<Facility>,
    pub contracts: Vec<Contract>,
    pub transactions: Vec<crate::transactions::Transaction>,
    pub payment_parts: Vec<PaymentPart>,
}
pub async fn get_loan_account(pool: &SqlitePool, account_id: String) -> Result<Snapshot, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let account = sqlx::query_as(&format!(
        "SELECT {}, {} FROM accounts WHERE id=? AND type='loan'",
        crate::accounts::COLUMNS,
        crate::accounts::PAYROLL_COLUMN
    ))
    .bind(&account_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?
    .ok_or("Loan account not found.")?;
    let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    let facility = facilities(&mut tx)
        .await
        .map_err(err)?
        .into_iter()
        .find(|f| f.account_id == account_id);
    let contracts = contracts(&mut tx)
        .await
        .map_err(err)?
        .into_iter()
        .filter(|c| c.account_id == account_id)
        .collect();
    let mut transactions:Vec<crate::transactions::Transaction>=sqlx::query_as(&format!("{} WHERE t.account_id=? OR t.destination_account_id=? OR t.id IN (SELECT p.transaction_id FROM loan_payment_parts p LEFT JOIN loan_contracts c ON c.id=p.contract_id WHERE COALESCE(p.loan_account_id,c.account_id)=?) ORDER BY t.date DESC,t.created_at DESC",crate::transactions::SELECT)).bind(&account_id).bind(&account_id).bind(&account_id).fetch_all(&mut *tx).await.map_err(err)?;
    let imported = crate::cloud_sync_ledger::imported_entries(&mut tx).await?;
    for (id, (_, entry)) in &imported {
        if entry.loan_account_id.as_deref() == Some(account_id.as_str())
            && !transactions.iter().any(|t| &t.id == id)
        {
            if let Some(row) =
                sqlx::query_as(&format!("{} WHERE t.id=?", crate::transactions::SELECT))
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(err)?
            {
                transactions.push(row);
            }
        }
    }
    crate::transactions::decorate_in_connection(&mut tx, &mut transactions).await?;
    transactions.sort_by(|a, b| b.order().cmp(&a.order()));
    let payment_parts=sqlx::query_as("SELECT p.transaction_id,p.payment_id,p.contract_id,p.component,CAST(p.amount AS TEXT) AS amount FROM loan_payment_parts p LEFT JOIN loan_contracts c ON c.id=p.contract_id WHERE COALESCE(p.loan_account_id,c.account_id)=?").bind(&account_id).fetch_all(&mut *tx).await.map_err(err)?;
    let paid_off_on = sqlx::query_scalar("SELECT paid_off_on FROM loan_payoffs WHERE account_id=?")
        .bind(&account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(Snapshot {
        paid_off_on,
        account,
        currency,
        facility,
        contracts,
        transactions,
        payment_parts,
    })
}
pub async fn currency(conn: &mut SqliteConnection, expected: &str) -> Result<(), String> {
    let saved: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(conn)
        .await
        .map_err(err)?;
    if saved.as_deref() != Some(expected) {
        return Err("Currency changed. Reload before saving.".into());
    }
    Ok(())
}
pub async fn active_loan(conn: &mut SqliteConnection, id: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=? AND type='loan' AND loan_type='personal_loan' AND is_archived=0").bind(id).fetch_optional(conn).await.map_err(err)?.ok_or("Choose an active Personal Loan account.".into())
}
pub async fn cash(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank') AND is_archived=0)").bind(id).fetch_one(conn).await.map_err(err)?;
    if !ok {
        return Err("Choose an active cash or bank account.".into());
    }
    Ok(())
}
pub async fn save_loan_facility(
    pool: &SqlitePool,
    account_id: String,
    credit_limit: String,
    currency_code: String,
) -> Result<(), String> {
    let limit = amount(&credit_limit)?;
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &currency_code).await?;
    active_loan(&mut tx, &account_id).await?;
    sqlx::query("INSERT INTO loan_facilities(account_id,credit_limit) VALUES (?,?) ON CONFLICT(account_id) DO UPDATE SET credit_limit=excluded.credit_limit").bind(account_id).bind(limit).execute(&mut *tx).await.map_err(err)?;
    tx.commit().await.map_err(err)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveContract {
    pub id: Option<String>,
    pub account_id: String,
    pub name: String,
    pub principal: String,
    pub remaining_principal: String,
    pub borrowing_date: String,
    pub receiving_account_id: String,
    pub payment_account_id: String,
    pub interest_rate: i64,
    pub monthly_amount: String,
    pub installment_count: i64,
    pub first_due_date: String,
    pub schedule_end: Option<String>,
    pub notes: String,
    pub borrowing_kind: String,
    pub currency: String,
    #[serde(default)]
    pub acknowledge_over_limit: bool,
}
pub async fn save(pool: &SqlitePool, input: SaveContract) -> Result<(), String> {
    let principal = amount(&input.principal)?;
    let remaining = amount(&input.remaining_principal)?;
    let monthly = amount(&input.monthly_amount)?;
    if principal == 0
        || remaining > principal
        || monthly == 0
        || !(1..=600).contains(&input.installment_count)
        || monthly.checked_mul(input.installment_count).is_none()
    {
        return Err(
            "Check principal, remaining principal, and schedule amounts (1–600 installments)."
                .into(),
        );
    }
    if input.name.trim().is_empty()
        || input.name.trim().chars().count() > 100
        || input.notes.chars().count() > 1000
        || !(0..=1000000).contains(&input.interest_rate)
    {
        return Err("Check the contract name, notes, and interest rate (0–100%).".into());
    }
    if !valid_date(&input.borrowing_date)
        || !valid_date(&input.first_due_date)
        || input.first_due_date < input.borrowing_date
        || input
            .schedule_end
            .as_ref()
            .is_some_and(|d| !valid_date(d) || d < &input.borrowing_date)
    {
        return Err("Enter valid borrowing, first due, and optional schedule end dates.".into());
    }
    let year: i64 = input.first_due_date[..4].parse().unwrap();
    let month: i64 = input.first_due_date[5..7].parse().unwrap();
    if year * 12 + month - 1 + input.installment_count - 1 >= 120000 {
        return Err("Final payment must be within year 9999.".into());
    }
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &input.currency).await?;
    let balance = active_loan(&mut tx, &input.account_id).await?;
    let limit: i64 =
        sqlx::query_scalar("SELECT credit_limit FROM loan_facilities WHERE account_id=?")
            .bind(&input.account_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("Configure revolving credit first.")?;
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.borrowing_date > today {
        return Err("Borrowings cannot be future dated.".into());
    }
    if let Some(id) = &input.id {
        let old: Contract = sqlx::query_as(&format!("{SELECT} WHERE c.id=? AND c.account_id=?"))
            .bind(id)
            .bind(&input.account_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("Contract no longer exists.")?;
        if old.principal != input.principal
            || old.remaining_principal != input.remaining_principal
            || old.borrowing_date != input.borrowing_date
            || old.receiving_account_id != input.receiving_account_id
            || old.borrowing_kind != input.borrowing_kind
        {
            return Err("Borrowing details and allocated principal are locked. Correct recorded transactions instead.".into());
        }
        if old.payment_account_id != input.payment_account_id {
            cash(&mut tx, &input.payment_account_id).await?;
        }
        sqlx::query("UPDATE loan_contracts SET name=?,payment_account_id=?,interest_rate=?,monthly_amount=?,installment_count=?,first_due_date=?,schedule_end=?,notes=? WHERE id=?").bind(input.name.trim()).bind(input.payment_account_id).bind(input.interest_rate).bind(monthly).bind(input.installment_count).bind(input.first_due_date).bind(input.schedule_end).bind(input.notes.trim()).bind(id).execute(&mut *tx).await.map_err(err)?;
    } else {
        cash(&mut tx, &input.receiving_account_id).await?;
        cash(&mut tx, &input.payment_account_id).await?;
        let borrowing_id = match input.borrowing_kind.as_str() {
            "new" => {
                if remaining != principal {
                    return Err("A new borrowing must allocate its full principal.".into());
                }
                if i128::from(balance.max(0)) + i128::from(principal) > i128::from(limit)
                    && !input.acknowledge_over_limit
                {
                    return Err("Borrowing exceeds estimated available credit. Acknowledge the warning to continue.".into());
                }
                Some(
                    insert_in_connection(
                        &mut tx,
                        NewTransaction {
                            kind: "transfer".into(),
                            account_id: input.account_id.clone(),
                            destination_account_id: Some(input.receiving_account_id.clone()),
                            amount: principal.to_string(),
                            date: input.borrowing_date.clone(),
                            description: format!("Borrowing: {}", input.name.trim()),
                            currency: input.currency.clone(),
                            payee_id: None,
                            category_id: None,
                            income_source_id: None,
                            cleared_account_ids: vec![],
                        },
                    )
                    .await?
                    .id,
                )
            }
            "existing" => None,
            _ => return Err("Choose New borrowing or Already recorded borrowing.".into()),
        };
        sqlx::query("INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,schedule_end,notes,borrowing_kind,borrowing_transaction_id) VALUES(lower(hex(randomblob(16))),?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(input.account_id).bind(input.name.trim()).bind(principal).bind(remaining).bind(input.borrowing_date).bind(input.receiving_account_id).bind(input.payment_account_id).bind(input.interest_rate).bind(monthly).bind(input.installment_count).bind(input.first_due_date).bind(input.schedule_end).bind(input.notes.trim()).bind(input.borrowing_kind).bind(borrowing_id).execute(&mut *tx).await.map_err(err)?;
    }
    validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(err)
}
pub async fn save_loan_contract(pool: &SqlitePool, input: SaveContract) -> Result<(), String> {
    save(pool, input).await
}

// Check after all parts of a mutation: contracts can never claim more debt than exists.
pub async fn validate_allocations(conn: &mut SqliteConnection) -> Result<(), String> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT c.account_id,c.remaining_principal FROM loan_contracts c")
            .fetch_all(&mut *conn)
            .await
            .map_err(err)?;
    let mut totals = std::collections::HashMap::<String, i128>::new();
    for (id, value) in rows {
        *totals.entry(id).or_default() += i128::from(value);
    }
    for (id, total) in totals {
        let balance: i64 = sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=?")
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(err)?;
        if total > i128::from(balance.max(0)) {
            return Err("This would allocate more principal than the account debt. Use a contract repayment, or correct the unassigned debt first.".into());
        }
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repayment {
    pub contract_id: Option<String>,
    pub loan_account_id: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cleared: bool,
    pub account_id: String,
    pub date: String,
    pub total: String,
    pub principal: String,
    pub interest: String,
    pub fee: String,
    pub currency: String,
}
pub async fn repay(pool: &SqlitePool, input: Repayment) -> Result<(), String> {
    let principal = amount(&input.principal)?;
    let interest = amount(&input.interest)?;
    let fee = amount(&input.fee)?;
    let total = amount(&input.total)?;
    if total == 0
        || principal
            .checked_add(interest)
            .and_then(|n| n.checked_add(fee))
            != Some(total)
    {
        return Err("Principal, interest and fees must equal the positive total payment.".into());
    }
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &input.currency).await?;
    let source_ok: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank','wallet') AND is_archived=0)")
        .bind(&input.account_id).fetch_one(&mut *tx).await.map_err(err)?;
    if !source_ok {
        return Err("Choose an active cash, bank or wallet account.".into());
    }
    let (loan_id, name) = if let Some(contract_id) = &input.contract_id {
        let contract: Contract = sqlx::query_as(&format!("{SELECT} WHERE c.id=?"))
            .bind(contract_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("Contract not found.")?;
        active_loan(&mut tx, &contract.account_id).await?;
        if input
            .loan_account_id
            .as_ref()
            .is_some_and(|id| id != &contract.account_id)
        {
            return Err("Choose a contract belonging to this loan account.".into());
        }
        if contract.needs_review || input.date < contract.borrowing_date {
            return Err("Review the borrowing before recording this repayment.".into());
        }
        let remaining = amount(&contract.remaining_principal)?
            .checked_sub(principal)
            .filter(|n| *n >= 0)
            .ok_or("Principal payment exceeds this contract's remaining principal.")?;
        sqlx::query("UPDATE loan_contracts SET remaining_principal=? WHERE id=?")
            .bind(remaining)
            .bind(&contract.id)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        (contract.account_id, contract.name)
    } else {
        let id = input
            .loan_account_id
            .as_ref()
            .ok_or("Choose a loan account.")?;
        let (name, balance): (String, i64) = sqlx::query_as("SELECT name,current_balance FROM accounts WHERE id=? AND type='loan' AND is_archived=0")
            .bind(id).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Choose an active loan account.")?;
        let has_contracts: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=?)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?;
        if has_contracts {
            return Err("Choose a contract for this loan repayment.".into());
        }
        if principal > balance.max(0) {
            return Err("Principal payment exceeds this loan's balance.".into());
        }
        (id.clone(), name)
    };
    // Recreate the default after a data reset, or if its previous name was edited.
    // Existing archived categories stay archived and normal expense validation applies.
    let charge_category: Option<String> = if interest > 0 || fee > 0 {
        sqlx::query("INSERT INTO categories(id,name,name_key,icon) VALUES(lower(hex(randomblob(16))),'Fee/Interest','fee/interest','tag') ON CONFLICT(name_key) DO NOTHING")
            .execute(&mut *tx).await.map_err(err)?;
        Some(
            sqlx::query_scalar("SELECT id FROM categories WHERE name_key='fee/interest'")
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?,
        )
    } else {
        None
    };
    let group: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    for (component, value) in [
        ("principal", principal),
        ("interest", interest),
        ("fee", fee),
    ] {
        if value == 0 {
            continue;
        }
        let row = insert_in_connection(
            &mut tx,
            NewTransaction {
                kind: if component == "principal" {
                    "repayment"
                } else {
                    "expense"
                }
                .into(),
                account_id: input.account_id.clone(),
                destination_account_id: if component == "principal" {
                    Some(loan_id.clone())
                } else {
                    None
                },
                amount: value.to_string(),
                date: input.date.clone(),
                description: if input.description.trim().is_empty() {
                    format!("{} · {} (linked loan payment)", name, component)
                } else {
                    input.description.trim().to_owned()
                },
                currency: input.currency.clone(),
                payee_id: None,
                category_id: if component == "principal" {
                    None
                } else {
                    charge_category.clone()
                },
                income_source_id: None,
                cleared_account_ids: if input.cleared {
                    vec![input.account_id.clone()]
                } else {
                    vec![]
                },
            },
        )
        .await?;
        sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount,loan_account_id) VALUES(?,?,?,?,?,?)").bind(row.id).bind(&group).bind(&input.contract_id).bind(component).bind(value).bind(if input.contract_id.is_none() { Some(&loan_id) } else { None }).execute(&mut *tx).await.map_err(err)?;
    }
    validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(err)
}
pub async fn record_loan_repayment(pool: &SqlitePool, input: Repayment) -> Result<(), String> {
    repay(pool, input).await
}

// Deleting any part reverses the complete payment, with every reconciliation checked.
pub async fn deletion_ids(conn: &mut SqliteConnection, id: &str) -> Result<Vec<String>, String> {
    let ids:Vec<String>=sqlx::query_scalar("SELECT transaction_id FROM loan_payment_parts WHERE payment_id=(SELECT payment_id FROM loan_payment_parts WHERE transaction_id=?)").bind(id).fetch_all(conn).await.map_err(err)?;
    Ok(if ids.is_empty() {
        vec![id.to_owned()]
    } else {
        ids
    })
}
pub async fn before_delete(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let part:Option<(String,i64)>=sqlx::query_as("SELECT contract_id,amount FROM loan_payment_parts WHERE transaction_id=? AND component='principal' AND contract_id IS NOT NULL").bind(id).fetch_optional(&mut *conn).await.map_err(err)?;
    if let Some((contract, value)) = part {
        // remaining + principal cannot exceed the original principal; CHECK catches stale/corrupt data.
        sqlx::query(
            "UPDATE loan_contracts SET remaining_principal=remaining_principal+? WHERE id=?",
        )
        .bind(value)
        .bind(contract)
        .execute(&mut *conn)
        .await
        .map_err(err)?;
    }
    let linked: Option<String> =
        sqlx::query_scalar("SELECT id FROM loan_contracts WHERE borrowing_transaction_id=?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(err)?;
    if let Some(contract) = linked {
        let has: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loan_payment_parts WHERE contract_id=?)",
        )
        .bind(&contract)
        .fetch_one(&mut *conn)
        .await
        .map_err(err)?;
        if has {
            return Err("Delete this contract's repayments before deleting its borrowing.".into());
        }
        sqlx::query("UPDATE loan_contracts SET remaining_principal=0,needs_review=1 WHERE id=?")
            .bind(contract)
            .execute(conn)
            .await
            .map_err(err)?;
    }
    Ok(())
}
pub async fn delete_loan_contract(pool: &SqlitePool, id: String) -> Result<(), String> {
    // Referenced contracts retain their audit trail. Stopping the schedule remains available.
    let result=sqlx::query("DELETE FROM loan_contracts WHERE id=? AND NOT EXISTS(SELECT 1 FROM loan_payment_parts WHERE contract_id=?)").bind(&id).bind(&id).execute(pool).await.map_err(err)?;
    if result.rows_affected() != 1 {
        return Err("Contract has repayments or no longer exists. Stop its schedule to retain repayment history.".into());
    }
    Ok(())
}
