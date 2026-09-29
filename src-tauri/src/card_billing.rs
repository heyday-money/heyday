use crate::transactions::{valid_date, NewTransaction};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Statement {
    pub id: String,
    pub account_id: String,
    pub start_date: String,
    pub end_date: String,
    pub due_date: String,
    pub amount: String,
    pub minimum: String,
    pub needs_review: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Plan {
    pub statement_id: String,
    pub account_id: String,
    pub date: String,
    pub mode: String,
    pub target: String,
    pub available: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Allocation {
    pub statement_id: String,
    pub transaction_id: String,
    pub amount: String,
    pub date: String,
}
#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Coverage {
    pub installment_id: String,
    pub date: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct IncludedInstallment {
    pub statement_id: String,
    pub installment_id: String,
    pub date: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Entry {
    statement_id: String,
    transaction_id: String,
}
#[derive(Serialize)]
pub struct BillingData {
    pub statements: Vec<Statement>,
    pub plans: Vec<Plan>,
    pub allocations: Vec<Allocation>,
    pub installments: Vec<IncludedInstallment>,
}
pub async fn snapshot(conn: &mut SqliteConnection) -> Result<BillingData, sqlx::Error> {
    Ok(BillingData {
        statements: sqlx::query_as("SELECT id,account_id,start_date,end_date,due_date,CAST(amount AS TEXT) amount,CAST(minimum AS TEXT) minimum,needs_review FROM card_statements ORDER BY end_date DESC,id").fetch_all(&mut *conn).await?,
        plans: sqlx::query_as("SELECT p.statement_id,p.account_id,p.date,p.mode,CAST(p.target AS TEXT) target,(a.is_archived=0 AND c.is_archived=0) available FROM card_payment_plans p JOIN accounts a ON a.id=p.account_id JOIN card_statements s ON s.id=p.statement_id JOIN accounts c ON c.id=s.account_id").fetch_all(&mut *conn).await?,
        allocations: sqlx::query_as("SELECT p.statement_id,p.transaction_id,CAST(t.amount AS TEXT) amount,t.date FROM card_payment_allocations p JOIN transactions t ON t.id=p.transaction_id").fetch_all(&mut *conn).await?,
        installments: sqlx::query_as("SELECT statement_id,installment_id,date FROM card_statement_installments").fetch_all(conn).await?,
    })
}
#[derive(Serialize)]
pub struct AccountBilling {
    account: crate::accounts::Account,
    currency: Option<String>,
    billing: BillingData,
    transactions: Vec<crate::transactions::Transaction>,
    entries: Vec<Entry>,
    installments: Vec<crate::installments::Installment>,
}
#[derive(Serialize)]
pub struct CardOverview {
    accounts: Vec<crate::accounts::Account>,
    archived_ids: Vec<String>,
    currency: Option<String>,
    billing: BillingData,
    transactions: Vec<crate::transactions::Transaction>,
}
async fn overview_snapshot(pool: &SqlitePool) -> Result<CardOverview, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let accounts = sqlx::query_as(&format!("SELECT {} FROM accounts WHERE type='credit_card' ORDER BY is_archived,name COLLATE NOCASE,id",crate::accounts::COLUMNS)).fetch_all(&mut *tx).await.map_err(err)?;
    let archived_ids = sqlx::query_scalar("SELECT id FROM accounts WHERE type='credit_card' AND is_archived=1").fetch_all(&mut *tx).await.map_err(err)?;
    let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1").fetch_one(&mut *tx).await.map_err(err)?;
    let billing = snapshot(&mut tx).await.map_err(err)?;
    let transactions = sqlx::query_as(&format!("{} WHERE a.type='credit_card' OR d.type='credit_card' ORDER BY t.date DESC,t.id",crate::transactions::SELECT)).fetch_all(&mut *tx).await.map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(CardOverview { accounts, archived_ids, currency, billing, transactions })
}
#[tauri::command]
pub async fn get_card_overview(pool: tauri::State<'_, SqlitePool>) -> Result<CardOverview, String> {
    overview_snapshot(pool.inner()).await
}
async fn account_snapshot(pool: &SqlitePool, account_id: &str) -> Result<AccountBilling, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let account = sqlx::query_as(&format!(
        "SELECT {} FROM accounts WHERE id=? AND type='credit_card'",
        crate::accounts::COLUMNS
    ))
    .bind(account_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(err)?
    .ok_or("Credit card not found.")?;
    let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    let mut billing = snapshot(&mut tx).await.map_err(err)?;
    billing.statements.retain(|s| s.account_id == account_id);
    billing
        .plans
        .retain(|p| billing.statements.iter().any(|s| s.id == p.statement_id));
    billing
        .allocations
        .retain(|p| billing.statements.iter().any(|s| s.id == p.statement_id));
    billing
        .installments
        .retain(|p| billing.statements.iter().any(|s| s.id == p.statement_id));
    let transactions = sqlx::query_as(&format!(
        "{} WHERE t.account_id=? OR t.destination_account_id=? ORDER BY t.date DESC,t.id",
        crate::transactions::SELECT
    ))
    .bind(account_id)
    .bind(account_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let entries=sqlx::query_as("SELECT e.statement_id,e.transaction_id FROM card_statement_entries e JOIN card_statements s ON s.id=e.statement_id WHERE s.account_id=?").bind(account_id).fetch_all(&mut *tx).await.map_err(err)?;
    let installments = sqlx::query_as(&format!(
        "{} WHERE i.debt_account_id=?",
        crate::installments::SELECT
            .split(" ORDER BY")
            .next()
            .unwrap()
    ))
    .bind(account_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(AccountBilling {
        account,
        currency,
        billing,
        transactions,
        entries,
        installments,
    })
}
fn err(e: sqlx::Error) -> String {
    e.to_string()
}
fn amount(s: &str) -> Result<i64, String> {
    let n = s
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if n < 0 {
        Err("Amount cannot be negative.".into())
    } else {
        Ok(n)
    }
}
async fn currency(conn: &mut SqliteConnection, code: &str) -> Result<(), String> {
    let saved: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(conn)
        .await
        .map_err(err)?;
    if saved.as_deref() != Some(code) {
        return Err("Currency changed. Reload before saving.".into());
    }
    Ok(())
}
async fn paid(conn: &mut SqliteConnection, id: &str) -> Result<i64, String> {
    let values:Vec<i64>=sqlx::query_scalar("SELECT t.amount FROM card_payment_allocations p JOIN transactions t ON t.id=p.transaction_id WHERE p.statement_id=?").bind(id).fetch_all(conn).await.map_err(err)?;
    values.into_iter().try_fold(0i64, |a, b| {
        a.checked_add(b)
            .ok_or("Payments exceed the supported range.".into())
    })
}
async fn active_card(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='credit_card' AND is_archived=0)",
    )
    .bind(id)
    .fetch_one(conn)
    .await
    .map_err(err)?;
    if !ok {
        return Err("Choose an active credit card.".into());
    }
    Ok(())
}
async fn cash_account(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank','wallet') AND is_archived=0)").bind(id).fetch_one(conn).await.map_err(err)?;
    if !ok {
        return Err("Choose an active cash, bank or wallet payment account.".into());
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct SaveStatement {
    id: Option<String>,
    account_id: String,
    start_date: String,
    end_date: String,
    due_date: String,
    amount: String,
    minimum: String,
    currency: String,
    installments: Vec<Coverage>,
}
async fn save_statement(pool: &SqlitePool, input: SaveStatement) -> Result<(), String> {
    if ![&input.start_date, &input.end_date, &input.due_date]
        .iter()
        .all(|d| valid_date(d))
        || input.start_date > input.end_date
        || input.due_date <= input.end_date
    {
        return Err("Enter a valid billing range and a due date after its closing date.".into());
    }
    let total = amount(&input.amount)?;
    let minimum = amount(&input.minimum)?;
    if minimum > total {
        return Err("Minimum cannot exceed the statement amount.".into());
    }
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &input.currency).await?;
    active_card(&mut tx, &input.account_id).await?;
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.end_date > today {
        return Err("Only closed billing periods can be confirmed.".into());
    }
    let id = if let Some(id) = input.id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM card_statements WHERE id=? AND account_id=?)",
        )
        .bind(&id)
        .bind(&input.account_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
        if !exists {
            return Err("Statement no longer exists. Reload.".into());
        }
        id
    } else {
        sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?
    };
    let overlaps:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM card_statements WHERE account_id=? AND id<>? AND start_date<=? AND end_date>=?)").bind(&input.account_id).bind(&id).bind(&input.end_date).bind(&input.start_date).fetch_one(&mut *tx).await.map_err(err)?;
    if overlaps {
        return Err("This billing period overlaps another saved statement.".into());
    }
    if paid(&mut tx, &id).await? > total {
        return Err("Statement amount cannot be below its applied payments. Unlink payments before correcting it.".into());
    }
    let invalid_payments: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM card_payment_allocations p JOIN transactions t ON t.id=p.transaction_id WHERE p.statement_id=? AND t.date<=?)")
        .bind(&id).bind(&input.end_date).fetch_one(&mut *tx).await.map_err(err)?;
    let invalid_plan: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM card_payment_plans WHERE statement_id=? AND (date<=? OR (mode='custom' AND target>?)))")
        .bind(&id).bind(&input.end_date).bind(total).fetch_one(&mut *tx).await.map_err(err)?;
    if invalid_payments || invalid_plan {
        return Err("This correction conflicts with applied payments or the payment plan. Unlink payments or remove the plan before changing the statement.".into());
    }
    sqlx::query("INSERT INTO card_statements(id,account_id,start_date,end_date,due_date,amount,minimum) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET start_date=excluded.start_date,end_date=excluded.end_date,due_date=excluded.due_date,amount=excluded.amount,minimum=excluded.minimum,needs_review=0").bind(&id).bind(&input.account_id).bind(&input.start_date).bind(&input.end_date).bind(&input.due_date).bind(total).bind(minimum).execute(&mut *tx).await.map_err(err)?;
    sqlx::query("UPDATE card_payment_plans SET target=CASE mode WHEN 'full' THEN ? WHEN 'minimum' THEN ? ELSE target END WHERE statement_id=?").bind(total).bind(minimum).bind(&id).execute(&mut *tx).await.map_err(err)?;
    sqlx::query("DELETE FROM card_statement_entries WHERE statement_id=?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    sqlx::query("INSERT INTO card_statement_entries SELECT ?,id FROM transactions WHERE (account_id=? OR destination_account_id=?) AND date BETWEEN ? AND ?").bind(&id).bind(&input.account_id).bind(&input.account_id).bind(&input.start_date).bind(&input.end_date).execute(&mut *tx).await.map_err(err)?;
    sqlx::query("DELETE FROM card_statement_installments WHERE statement_id=?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(err)?;
    for coverage in input.installments {
        if !valid_date(&coverage.date) {
            return Err("Invalid installment date.".into());
        }
        let schedule:Option<(String,i64)>=sqlx::query_as("SELECT first_due_date,installment_count FROM installments WHERE id=? AND debt_account_id=?").bind(&coverage.installment_id).bind(&input.account_id).fetch_optional(&mut *tx).await.map_err(err)?;
        let (first, count) = schedule.ok_or("Choose an installment belonging to this card.")?;
        if !(0..count).any(|n| scheduled_date(&first, n) == coverage.date) {
            return Err("Choose an actual scheduled installment date.".into());
        }
        sqlx::query("INSERT INTO card_statement_installments VALUES(?,?,?)")
            .bind(&id)
            .bind(coverage.installment_id)
            .bind(coverage.date)
            .execute(&mut *tx)
            .await
            .map_err(|_| {
                "This installment occurrence is already included in another statement.".to_string()
            })?;
    }
    tx.commit().await.map_err(err)
}
fn scheduled_date(first: &str, offset: i64) -> String {
    let y: i64 = first[..4].parse().unwrap();
    let m: i64 = first[5..7].parse().unwrap();
    let day: i64 = first[8..].parse().unwrap();
    let index = y * 12 + m - 1 + offset;
    let y = index / 12;
    let m = index % 12 + 1;
    let max = match m {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 31,
    };
    format!("{y:04}-{m:02}-{:02}", day.min(max))
}
#[derive(Deserialize)]
pub struct SavePaymentPlan {
    statement_id: String,
    account_id: String,
    date: String,
    mode: String,
    target: String,
    currency: String,
}
async fn save_plan(pool: &SqlitePool, input: SavePaymentPlan) -> Result<(), String> {
    if !valid_date(&input.date) || !["full", "minimum", "custom"].contains(&input.mode.as_str()) {
        return Err("Choose a valid payment date and payment option.".into());
    }
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &input.currency).await?;
    cash_account(&mut tx, &input.account_id).await?;
    let (card, total, minimum, end): (String, i64, i64, String) =
        sqlx::query_as("SELECT account_id,amount,minimum,end_date FROM card_statements WHERE id=?")
            .bind(&input.statement_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("Statement not found.")?;
    active_card(&mut tx, &card).await?;
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.date < today || input.date <= end {
        return Err("Plan a payment today or later, after the statement closing date.".into());
    }
    let target = match input.mode.as_str() {
        "full" => total,
        "minimum" => minimum,
        _ => amount(&input.target)?,
    };
    if target > total {
        return Err(
            "Payment target cannot exceed this statement. Record extra payments separately.".into(),
        );
    }
    sqlx::query("INSERT INTO card_payment_plans VALUES(?,?,?,?,?) ON CONFLICT(statement_id) DO UPDATE SET account_id=excluded.account_id,date=excluded.date,mode=excluded.mode,target=excluded.target").bind(input.statement_id).bind(input.account_id).bind(input.date).bind(input.mode).bind(target).execute(&mut *tx).await.map_err(err)?;
    tx.commit().await.map_err(err)
}
#[derive(Deserialize)]
pub struct PaymentInput {
    statement_id: String,
    transaction_id: Option<String>,
    account_id: String,
    date: String,
    amount: String,
    currency: String,
}
async fn payment(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(err)?;
    currency(&mut tx, &input.currency).await?;
    let (card, total, end): (String, i64, String) =
        sqlx::query_as("SELECT account_id,amount,end_date FROM card_statements WHERE id=?")
            .bind(&input.statement_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("Statement not found.")?;
    let id = if let Some(id) = input.transaction_id {
        id
    } else {
        cash_account(&mut tx, &input.account_id).await?;
        crate::transactions::insert_in_connection(
            &mut tx,
            NewTransaction {
                kind: "repayment".into(),
                account_id: input.account_id,
                destination_account_id: Some(card.clone()),
                amount: input.amount,
                date: input.date,
                description: "Credit card statement payment".into(),
                currency: input.currency,
                payee_id: None,
                category_id: None,
                income_source_id: None,
                cleared_account_ids: vec![],
            },
        )
        .await?
        .id
    };
    let record:Option<(i64,String)>=sqlx::query_as("SELECT t.amount,t.date FROM transactions t JOIN accounts a ON a.id=t.account_id WHERE t.id=? AND t.destination_account_id=? AND t.type IN ('repayment','transfer') AND a.type IN ('cash','bank','wallet')").bind(&id).bind(card).fetch_optional(&mut *tx).await.map_err(err)?;
    let (value, date) = record.ok_or("Choose a cash, bank or wallet payment to this card.")?;
    if date <= end {
        return Err("Only payments after statement close can be applied. Earlier payments belong in the statement balance.".into());
    }
    if paid(&mut tx, &input.statement_id)
        .await?
        .checked_add(value)
        .ok_or("Payment total is too large.")?
        > total
    {
        return Err("Applied payments cannot exceed the statement balance. Split a payment in Transactions when it covers multiple bills.".into());
    }
    sqlx::query("INSERT INTO card_payment_allocations VALUES(?,?)")
        .bind(id)
        .bind(input.statement_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "This payment is already linked to a statement.".to_string())?;
    tx.commit().await.map_err(err)
}
#[tauri::command]
pub async fn get_card_billing(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<AccountBilling, String> {
    account_snapshot(pool.inner(), &account_id).await
}
#[tauri::command]
pub async fn save_card_statement(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveStatement,
) -> Result<(), String> {
    save_statement(pool.inner(), input).await
}
#[tauri::command]
pub async fn save_card_payment_plan(
    pool: tauri::State<'_, SqlitePool>,
    input: SavePaymentPlan,
) -> Result<(), String> {
    save_plan(pool.inner(), input).await
}
#[tauri::command]
pub async fn record_card_payment(
    pool: tauri::State<'_, SqlitePool>,
    input: PaymentInput,
) -> Result<(), String> {
    payment(pool.inner(), input).await
}
#[tauri::command]
pub async fn remove_card_billing_record(
    pool: tauri::State<'_, SqlitePool>,
    kind: String,
    id: String,
) -> Result<(), String> {
    let query = match kind.as_str() {
        "statement" => "DELETE FROM card_statements WHERE id=?",
        "plan" => "DELETE FROM card_payment_plans WHERE statement_id=?",
        "allocation" => "DELETE FROM card_payment_allocations WHERE transaction_id=?",
        _ => return Err("Unknown billing record.".into()),
    };
    sqlx::query(query)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(err)?;
    Ok(())
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
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
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
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
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
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
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
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let snapshot = account_snapshot(&p, "card").await.unwrap();
        assert_eq!(snapshot.billing.statements[0].id, sid);
        assert_eq!(snapshot.billing.allocations[0].amount, "1000");
        assert_eq!(balances(&p).await[1].1, 9000);
        p.close().await;
        std::fs::remove_file(path).unwrap();
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
        let all = sqlx::migrate!("./migrations");
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
    #[test]
    fn independently_clamped_dates() {
        assert_eq!(scheduled_date("2024-01-31", 1), "2024-02-29");
        assert_eq!(scheduled_date("2024-01-31", 2), "2024-03-31");
        assert_eq!(scheduled_date("2023-01-31", 1), "2023-02-28");
    }
}
