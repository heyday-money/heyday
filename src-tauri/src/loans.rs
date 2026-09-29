use crate::transactions::{insert_in_connection, valid_date, NewTransaction};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
fn err(e: sqlx::Error) -> String {
    e.to_string()
}
fn amount(s: &str) -> Result<i64, String> {
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
    transaction_id: String,
    payment_id: String,
    contract_id: String,
    component: String,
    amount: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    paid_off_on: Option<String>,
    account: crate::accounts::Account,
    currency: Option<String>,
    facility: Option<Facility>,
    contracts: Vec<Contract>,
    transactions: Vec<crate::transactions::Transaction>,
    payment_parts: Vec<PaymentPart>,
}
#[tauri::command]
pub async fn get_loan_account(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<Snapshot, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let account = sqlx::query_as(&format!(
        "SELECT {} FROM accounts WHERE id=? AND type='loan'",
        crate::accounts::COLUMNS
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
    let transactions=sqlx::query_as(&format!("{} WHERE t.account_id=? OR t.destination_account_id=? OR t.id IN (SELECT p.transaction_id FROM loan_payment_parts p JOIN loan_contracts c ON c.id=p.contract_id WHERE c.account_id=?) ORDER BY t.date DESC,t.created_at DESC",crate::transactions::SELECT)).bind(&account_id).bind(&account_id).bind(&account_id).fetch_all(&mut *tx).await.map_err(err)?;
    let payment_parts=sqlx::query_as("SELECT p.transaction_id,p.payment_id,p.contract_id,p.component,CAST(p.amount AS TEXT) AS amount FROM loan_payment_parts p JOIN loan_contracts c ON c.id=p.contract_id WHERE c.account_id=?").bind(&account_id).fetch_all(&mut *tx).await.map_err(err)?;
    let paid_off_on = sqlx::query_scalar("SELECT paid_off_on FROM loan_payoffs WHERE account_id=?").bind(&account_id).fetch_optional(&mut *tx).await.map_err(err)?;
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
async fn currency(conn: &mut SqliteConnection, expected: &str) -> Result<(), String> {
    let saved: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(conn)
        .await
        .map_err(err)?;
    if saved.as_deref() != Some(expected) {
        return Err("Currency changed. Reload before saving.".into());
    }
    Ok(())
}
async fn active_loan(conn: &mut SqliteConnection, id: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=? AND type='loan' AND loan_type='personal_loan' AND is_archived=0").bind(id).fetch_optional(conn).await.map_err(err)?.ok_or("Choose an active Personal Loan account.".into())
}
async fn cash(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank') AND is_archived=0)").bind(id).fetch_one(conn).await.map_err(err)?;
    if !ok {
        return Err("Choose an active cash or bank account.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn save_loan_facility(
    pool: tauri::State<'_, SqlitePool>,
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
    id: Option<String>,
    account_id: String,
    name: String,
    principal: String,
    remaining_principal: String,
    borrowing_date: String,
    receiving_account_id: String,
    payment_account_id: String,
    interest_rate: i64,
    monthly_amount: String,
    installment_count: i64,
    first_due_date: String,
    schedule_end: Option<String>,
    notes: String,
    borrowing_kind: String,
    currency: String,
    #[serde(default)]
    acknowledge_over_limit: bool,
}
async fn save(pool: &SqlitePool, input: SaveContract) -> Result<(), String> {
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
#[tauri::command]
pub async fn save_loan_contract(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveContract,
) -> Result<(), String> {
    save(pool.inner(), input).await
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
    contract_id: String,
    account_id: String,
    date: String,
    total: String,
    principal: String,
    interest: String,
    fee: String,
    currency: String,
}
async fn repay(pool: &SqlitePool, input: Repayment) -> Result<(), String> {
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
    cash(&mut tx, &input.account_id).await?;
    let contract: Contract = sqlx::query_as(&format!("{SELECT} WHERE c.id=?"))
        .bind(&input.contract_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?
        .ok_or("Contract not found.")?;
    active_loan(&mut tx, &contract.account_id).await?;
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
                    Some(contract.account_id.clone())
                } else {
                    None
                },
                amount: value.to_string(),
                date: input.date.clone(),
                description: format!("{} · {} (linked loan payment)", contract.name, component),
                currency: input.currency.clone(),
                payee_id: None,
                category_id: None,
                income_source_id: None,
                cleared_account_ids: vec![],
            },
        )
        .await?;
        sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount) VALUES(?,?,?,?,?)").bind(row.id).bind(&group).bind(&contract.id).bind(component).bind(value).execute(&mut *tx).await.map_err(err)?;
    }
    validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(err)
}
#[tauri::command]
pub async fn record_loan_repayment(
    pool: tauri::State<'_, SqlitePool>,
    input: Repayment,
) -> Result<(), String> {
    repay(pool.inner(), input).await
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
    let part:Option<(String,i64)>=sqlx::query_as("SELECT contract_id,amount FROM loan_payment_parts WHERE transaction_id=? AND component='principal'").bind(id).fetch_optional(&mut *conn).await.map_err(err)?;
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
#[tauri::command]
pub async fn delete_loan_contract(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    // Referenced contracts retain their audit trail. Stopping the schedule remains available.
    let result=sqlx::query("DELETE FROM loan_contracts WHERE id=? AND NOT EXISTS(SELECT 1 FROM loan_payment_parts WHERE contract_id=?)").bind(&id).bind(&id).execute(pool.inner()).await.map_err(err)?;
    if result.rows_affected() != 1 {
        return Err("Contract has repayments or no longer exists. Stop its schedule to retain repayment history.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Cash card','loan','personal_loan',0,0),('bank','Bank','bank',NULL,5000000,5000000); INSERT INTO loan_facilities VALUES('loan',10000000);").execute(&pool).await.unwrap();
        pool
    }
    fn input() -> SaveContract {
        serde_json::from_value(json!({"id":null,"account_id":"loan","name":"Borrowing 1","principal":"2000000","remaining_principal":"2000000","borrowing_date":"2024-01-31","receiving_account_id":"bank","payment_account_id":"bank","interest_rate":75000,"monthly_amount":"210000","installment_count":10,"first_due_date":"2024-02-29","schedule_end":null,"notes":"","borrowing_kind":"new","currency":"THB"})).unwrap()
    }
    fn payment(id: &str) -> Repayment {
        serde_json::from_value(json!({"contract_id":id,"account_id":"bank","date":"2024-03-01","total":"210000","principal":"200000","interest":"9000","fee":"1000","currency":"THB"})).unwrap()
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
        for m in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 25)
        {
            sqlx::raw_sql(&m.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Loan','loan','personal_loan',3000000,3000000),('bank','Bank','bank',NULL,0,0); INSERT INTO planner_debt_amounts VALUES('loan','2024-02',0);").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0025_loan_contracts.sql"))
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
        std::fs::remove_file(path).unwrap();
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
