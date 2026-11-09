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
    pub statement_id: String,
    pub transaction_id: String,
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
    pub account: crate::accounts::Account,
    pub currency: Option<String>,
    pub billing: BillingData,
    pub transactions: Vec<crate::transactions::Transaction>,
    pub entries: Vec<Entry>,
    pub installments: Vec<crate::installments::Installment>,
}
#[derive(Serialize)]
pub struct CardOverview {
    pub selective_defaults: Vec<crate::selective_defaults::Exclusion>,
    pub period_start_day: i64,
    pub accounts: Vec<crate::accounts::Account>,
    pub archived_ids: Vec<String>,
    pub currency: Option<String>,
    pub billing: BillingData,
    pub transactions: Vec<crate::transactions::Transaction>,
}
pub async fn overview_snapshot(pool: &SqlitePool) -> Result<CardOverview, String> {
    let mut tx = pool.begin().await.map_err(err)?;
    let accounts = sqlx::query_as(&format!("SELECT {} FROM accounts WHERE type='credit_card' ORDER BY is_archived,name COLLATE NOCASE,id",crate::accounts::COLUMNS)).fetch_all(&mut *tx).await.map_err(err)?;
    let archived_ids =
        sqlx::query_scalar("SELECT id FROM accounts WHERE type='credit_card' AND is_archived=1")
            .fetch_all(&mut *tx)
            .await
            .map_err(err)?;
    let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    let billing = snapshot(&mut tx).await.map_err(err)?;
    let transactions = sqlx::query_as(&format!(
        "{} WHERE a.type='credit_card' OR d.type='credit_card' ORDER BY t.date DESC,t.id",
        crate::transactions::SELECT
    ))
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    let selective_defaults = crate::selective_defaults::periods(&mut tx)
        .await
        .map_err(err)?;
    let period_start_day = sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    tx.commit().await.map_err(err)?;
    Ok(CardOverview {
        selective_defaults,
        period_start_day,
        accounts,
        archived_ids,
        currency,
        billing,
        transactions,
    })
}
pub async fn get_card_overview(pool: &SqlitePool) -> Result<CardOverview, String> {
    overview_snapshot(pool).await
}
pub async fn account_snapshot(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<AccountBilling, String> {
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
pub fn err(e: sqlx::Error) -> String {
    e.to_string()
}
pub fn amount(s: &str) -> Result<i64, String> {
    let n = s
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if n < 0 {
        Err("Amount cannot be negative.".into())
    } else {
        Ok(n)
    }
}
pub async fn currency(conn: &mut SqliteConnection, code: &str) -> Result<(), String> {
    let saved: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(conn)
        .await
        .map_err(err)?;
    if saved.as_deref() != Some(code) {
        return Err("Currency changed. Reload before saving.".into());
    }
    Ok(())
}
pub async fn paid(conn: &mut SqliteConnection, id: &str) -> Result<i64, String> {
    let values:Vec<i64>=sqlx::query_scalar("SELECT t.amount FROM card_payment_allocations p JOIN transactions t ON t.id=p.transaction_id WHERE p.statement_id=?").bind(id).fetch_all(conn).await.map_err(err)?;
    values.into_iter().try_fold(0i64, |a, b| {
        a.checked_add(b)
            .ok_or("Payments exceed the supported range.".into())
    })
}
pub async fn active_card(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
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
pub async fn cash_account(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank','wallet') AND is_archived=0)").bind(id).fetch_one(conn).await.map_err(err)?;
    if !ok {
        return Err("Choose an active cash, bank or wallet payment account.".into());
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct SaveStatement {
    pub id: Option<String>,
    pub account_id: String,
    pub start_date: String,
    pub end_date: String,
    pub due_date: String,
    pub amount: String,
    pub minimum: String,
    pub currency: String,
    pub installments: Vec<Coverage>,
}
pub async fn save_statement(pool: &SqlitePool, input: SaveStatement) -> Result<(), String> {
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
pub use crate::domain::dates::scheduled_date;
#[derive(Deserialize)]
pub struct SavePaymentPlan {
    pub statement_id: String,
    pub account_id: String,
    pub date: String,
    pub mode: String,
    pub target: String,
    pub currency: String,
}
pub async fn save_plan(pool: &SqlitePool, input: SavePaymentPlan) -> Result<(), String> {
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
    pub statement_id: String,
    pub transaction_id: Option<String>,
    pub account_id: String,
    pub date: String,
    pub amount: String,
    pub currency: String,
}
pub async fn payment(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
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
pub async fn get_card_billing(
    pool: &SqlitePool,
    account_id: String,
) -> Result<AccountBilling, String> {
    account_snapshot(pool, &account_id).await
}
pub async fn save_card_statement(pool: &SqlitePool, input: SaveStatement) -> Result<(), String> {
    save_statement(pool, input).await
}
pub async fn save_card_payment_plan(
    pool: &SqlitePool,
    input: SavePaymentPlan,
) -> Result<(), String> {
    save_plan(pool, input).await
}
pub async fn record_card_payment(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    payment(pool, input).await
}
pub async fn remove_card_billing_record(
    pool: &SqlitePool,
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
        .execute(pool)
        .await
        .map_err(err)?;
    Ok(())
}
