use crate::transactions::{insert_in_connection, NewTransaction};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
pub fn err(e: sqlx::Error) -> String {
    e.to_string()
}
use crate::domain::money::nonnegative_amount as amount;
use crate::domain::payroll;
pub use crate::domain::payroll::{Deduction, PaymentInput};
#[derive(Serialize, Deserialize)]
pub struct Breakdown {
    pub name: String,
    pub debt_account_name: Option<String>,
    pub principal: String,
    #[serde(flatten)]
    pub values: Deduction,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Payment {
    pub id: String,
    pub income_id: String,
    pub occurrence: String,
    pub date: String,
    pub gross: String,
    pub net: String,
    pub breakdown: String,
}
pub async fn record(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    let gross = payroll::validate(&input)?;
    let mut tx = pool.begin().await.map_err(err)?;
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.date > today || input.occurrence.as_str() > &today[..7] {
        return Err("Transactions cannot be dated in the future.".into());
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if currency.as_deref() != Some(&input.currency) {
        return Err("Currency changed. Reload transactions before saving.".into());
    }
    let (name,bank):(String,String)=sqlx::query_as("SELECT i.name,i.destination_account_id FROM incomes i JOIN accounts a ON a.id=i.destination_account_id WHERE i.id=? AND i.type='salary' AND i.is_active=1 AND a.is_archived=0 AND a.type IN ('cash','bank','wallet')")
        .bind(&input.income_id).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Choose an active salary with a cash, bank or wallet destination.")?;
    if bank != input.destination_account_id {
        return Err("Salary destination changed. Reload before recording.".into());
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE income_id=? AND occurrence=?) OR EXISTS(SELECT 1 FROM transactions WHERE income_source_id=? AND (substr(date,1,7)=? OR substr(date,1,7)=?) AND id NOT IN (SELECT transaction_id FROM salary_payment_transactions))")
        .bind(&input.income_id).bind(&input.occurrence).bind(&input.income_id).bind(&input.date[..7]).bind(&input.occurrence).fetch_one(&mut *tx).await.map_err(err)?;
    if duplicate {
        return Err("A salary payment or linked income receipt already exists for this month. Review and reverse the existing entry before recording again.".into());
    }
    let definitions: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id,name,debt_account_id FROM income_deductions WHERE income_id=? ORDER BY id",
    )
    .bind(&input.income_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    if definitions.len() != input.deductions.len() || input.deductions.len() > 100 {
        return Err("Salary deductions changed. Reload before recording.".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut total = 0i64;
    let mut breakdown = Vec::new();
    for d in &input.deductions {
        let (_, label, debt) = definitions
            .iter()
            .find(|v| v.0 == d.id)
            .ok_or("Salary deductions changed. Reload before recording.")?;
        if &d.debt_account_id != debt {
            return Err("Salary deductions changed. Reload before recording.".into());
        }
        if !seen.insert(&d.id) {
            return Err("Duplicate salary deduction.".into());
        }
        let value = amount(&d.amount)?;
        let interest = amount(&d.interest)?;
        let fee = amount(&d.fee)?;
        let principal = payroll::principal(value, interest, fee)?;
        total = payroll::add_deduction(total, value)?;
        let mut debt_name = None;
        if let Some(id) = debt {
            let (n,kind,balance):(String,String,i64)=sqlx::query_as("SELECT name,type,current_balance FROM accounts WHERE id=? AND is_archived=0 AND type IN ('loan','credit_card')").bind(id).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Choose an active debt account for every linked deduction.")?;
            debt_name = Some(n);
            if kind == "credit_card" && (interest != 0 || fee != 0 || d.contract_id.is_some()) {
                return Err(
                    "Credit card payroll payments use the full deduction without a loan split."
                        .into(),
                );
            }
            if let Some(contract) = &d.contract_id {
                let updated=sqlx::query("UPDATE loan_contracts SET remaining_principal=remaining_principal-? WHERE id=? AND account_id=? AND needs_review=0 AND borrowing_date<=? AND remaining_principal>=?")
                    .bind(principal).bind(contract).bind(id).bind(&input.date).bind(principal).execute(&mut *tx).await.map_err(err)?;
                if updated.rows_affected() != 1 {
                    return Err(
                        "Choose a valid loan contract with sufficient remaining principal.".into(),
                    );
                }
            } else if kind == "loan" {
                crate::cloud_sync_income::require_known_contracts(&mut tx, id).await?;
                let contracts: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=?)",
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?;
                if contracts {
                    return Err("Choose a contract for this loan repayment.".into());
                }
                if principal > balance.max(0) {
                    return Err("Principal payment exceeds this loan's balance.".into());
                }
            }
        } else if interest != 0 || fee != 0 || d.contract_id.is_some() {
            return Err("Only linked loans support principal, interest and fees.".into());
        }
        breakdown.push(Breakdown {
            name: label.clone(),
            debt_account_name: debt_name,
            principal: if debt.is_some() {
                principal.to_string()
            } else {
                "0".into()
            },
            values: d.clone(),
        });
    }
    let net = payroll::net(gross, total)?;
    let id:String=sqlx::query_scalar("INSERT INTO salary_payments(id,income_id,occurrence,date,gross,net,breakdown) VALUES(lower(hex(randomblob(16))),?,?,?,?,?,?) RETURNING id")
        .bind(&input.income_id).bind(&input.occurrence).bind(&input.date).bind(gross).bind(net).bind(serde_json::to_string(&breakdown).map_err(|e|e.to_string())?).fetch_one(&mut *tx).await.map_err(err)?;
    for (account, value, role, part) in
        std::iter::once((bank, net, "net", None)).chain(breakdown.iter().filter_map(|b| {
            b.values.debt_account_id.as_ref().map(|a| {
                (
                    a.clone(),
                    b.principal.parse::<i64>().unwrap(),
                    "principal",
                    Some(b),
                )
            })
        }))
    {
        if value == 0 {
            continue;
        }
        let row = insert_in_connection(
            &mut tx,
            NewTransaction {
                kind: "income".into(),
                account_id: account,
                destination_account_id: None,
                amount: value.to_string(),
                date: input.date.clone(),
                description: format!(
                    "{} · {} · {}",
                    name,
                    input.occurrence,
                    if role == "net" {
                        "Net salary"
                    } else {
                        "Payroll principal"
                    }
                )
                .chars()
                .take(200)
                .collect(),
                currency: input.currency.clone(),
                payee_id: None,
                category_id: None,
                income_source_id: if role == "net" {
                    Some(input.income_id.clone())
                } else {
                    None
                },
                cleared_account_ids: vec![],
            },
        )
        .await?;
        sqlx::query("INSERT INTO salary_payment_transactions VALUES(?,?,?)")
            .bind(&row.id)
            .bind(&id)
            .bind(role)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        if let Some(b) = part {
            let kind: String = sqlx::query_scalar("SELECT type FROM accounts WHERE id=?")
                .bind(&b.values.debt_account_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?;
            if kind == "loan" {
                sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount,loan_account_id) VALUES(?,?,?,'principal',?,?)")
                    .bind(&row.id).bind(format!("{}:{}",id,b.values.id)).bind(&b.values.contract_id).bind(value).bind(if b.values.contract_id.is_none(){b.values.debt_account_id.as_deref()}else{None}).execute(&mut *tx).await.map_err(err)?;
            }
        }
    }
    for b in &breakdown {
        if let Some(account) = &b.values.debt_account_id {
            let negative: bool = sqlx::query_scalar(
                "SELECT type='loan' AND current_balance<0 FROM accounts WHERE id=?",
            )
            .bind(account)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
            if negative {
                return Err("Principal payment exceeds this loan's balance.".into());
            }
        }
    }
    crate::loans::validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(err)
}
pub async fn group(conn: &mut SqliteConnection, id: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT payment_id FROM salary_payment_transactions WHERE transaction_id=?")
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(err)
}
pub async fn record_salary_payment(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    record(pool, input).await
}
pub async fn list_salary_payments(
    pool: &SqlitePool,
    income_id: Option<String>,
    payment_id: Option<String>,
    account_id: Option<String>,
) -> Result<Vec<Payment>, String> {
    history(pool, income_id, payment_id, account_id).await
}
pub async fn history(
    pool: &SqlitePool,
    income_id: Option<String>,
    payment_id: Option<String>,
    account_id: Option<String>,
) -> Result<Vec<Payment>, String> {
    sqlx::query_as("SELECT id,income_id,occurrence,date,CAST(gross AS TEXT) gross,CAST(net AS TEXT) net,breakdown FROM salary_payments WHERE (? IS NULL OR income_id=?) AND (? IS NULL OR id=?) AND (? IS NULL OR EXISTS(SELECT 1 FROM json_each(breakdown) WHERE json_extract(value,'$.debt_account_id')=?)) ORDER BY date DESC,id")
        .bind(&income_id).bind(&income_id).bind(&payment_id).bind(&payment_id).bind(&account_id).bind(&account_id).fetch_all(pool).await.map_err(err)
}

pub async fn delete_salary_payment(
    pool: &SqlitePool,
    id: String,
    confirmed: bool,
) -> Result<(), String> {
    reverse(pool, &id, confirmed).await
}
pub async fn reverse(pool: &SqlitePool, id: &str, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("Confirm reversal of the entire salary payment.".into());
    }
    let transaction: Option<String> = sqlx::query_scalar(
        "SELECT transaction_id FROM salary_payment_transactions WHERE payment_id=? LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    if let Some(transaction) = transaction {
        crate::transactions::remove_confirmed(pool, &transaction, true).await
    } else {
        sqlx::query("DELETE FROM salary_payments WHERE id=? AND NOT EXISTS(SELECT 1 FROM salary_payment_transactions WHERE payment_id=?)").bind(&id).bind(&id).execute(pool).await.map_err(err)?;
        Ok(())
    }
}
