//! Contract allocations are derived from a stable principal basis and immutable
//! payment parts. Reservations support upgrading already-imported ledger history.
use crate::{
    cloud_sync_ledger::{self as ledger, TransactionGroup, BLOCKED},
    cloud_sync_metadata::{self as meta, get, number, optional, Metadata, Row},
    cloud_sync_protocol::UNSAFE,
    cloud_sync_store::{Record, Value, DATABASE},
};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub transaction_id: String,
    pub component: String,
    pub amount: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub id: String,
    pub loan_account_id: String,
    pub contract_id: Option<String>,
    pub parts: Vec<Part>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Links {
    pub borrowing_contract_id: Option<String>,
    pub installment_id: Option<String>,
}
impl Payment {
    pub fn validate(&self, group: &TransactionGroup, id: &str) -> Result<(), String> {
        ledger::id(&self.id)?;
        ledger::id(&self.loan_account_id)?;
        if let Some(c) = &self.contract_id {
            ledger::id(c)?;
        }
        if id != format!("loan-{}", self.id)
            || self.parts.is_empty()
            || self.parts.len() > 3
            || self.parts.len() != group.entries.len()
            || group.payroll.is_some()
        {
            return Err(UNSAFE.into());
        }
        let mut components = BTreeSet::new();
        let mut previous = "";
        let mut cash = None;
        let mut date = None;
        for p in &self.parts {
            ledger::id(&p.transaction_id)?;
            if p.transaction_id.as_str() <= previous
                || !components.insert(&p.component)
                || !["principal", "interest", "fee"].contains(&p.component.as_str())
                || ledger::amount(&p.amount)? <= 0
            {
                return Err(UNSAFE.into());
            }
            previous = &p.transaction_id;
            let e = group
                .entries
                .iter()
                .find(|e| e.id == p.transaction_id)
                .ok_or(UNSAFE)?;
            if e.amount != p.amount
                || e.loan_component.as_deref() != Some(&p.component)
                || e.loan_account_id.as_deref() != Some(&self.loan_account_id)
                || e.salary_role.is_some()
                || cash.is_some_and(|c| c != &e.account_id)
                || date.is_some_and(|d| d != &e.date)
            {
                return Err(UNSAFE.into());
            }
            cash = Some(&e.account_id);
            date = Some(&e.date);
            if p.component == "principal" {
                if e.kind != "repayment"
                    || e.destination_account_id.as_deref() != Some(&self.loan_account_id)
                {
                    return Err(UNSAFE.into());
                }
            } else if e.kind != "expense" || e.destination_account_id.is_some() {
                return Err(UNSAFE.into());
            }
        }
        Ok(())
    }
}
pub async fn load(
    conn: &mut SqliteConnection,
    group_id: &str,
    entries: &[ledger::Entry],
    imported: Option<&TransactionGroup>,
) -> Result<(Option<Payment>, Option<Links>), String> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT full_enabled FROM cloud_sync_binding WHERE id=1),0)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !enabled {
        return Ok((
            imported.and_then(|g| g.loan.clone()),
            imported.and_then(|g| g.links.clone()),
        ));
    }
    let mut loan = None;
    if let Some(id) = group_id.strip_prefix("loan-") {
        let parts:Vec<(String,String,String,Option<String>,String)>=sqlx::query_as("SELECT p.transaction_id,p.component,CAST(p.amount AS TEXT),p.contract_id,COALESCE(p.loan_account_id,c.account_id) FROM loan_payment_parts p LEFT JOIN loan_contracts c ON c.id=p.contract_id WHERE p.payment_id=? ORDER BY p.transaction_id").bind(id).fetch_all(&mut *conn).await.map_err(|_|DATABASE)?;
        if let Some(first) = parts.first() {
            loan = Some(Payment {
                id: id.into(),
                contract_id: first.3.clone(),
                loan_account_id: first.4.clone(),
                parts: parts
                    .into_iter()
                    .map(|(transaction_id, component, amount, _, _)| Part {
                        transaction_id,
                        component,
                        amount,
                    })
                    .collect(),
            });
        } else {
            loan = imported.and_then(|g| g.loan.clone());
        }
    }
    let mut links = Links::default();
    for e in entries {
        links.borrowing_contract_id =
            sqlx::query_scalar("SELECT id FROM loan_contracts WHERE borrowing_transaction_id=?")
                .bind(&e.id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|_| DATABASE)?
                .or(links.borrowing_contract_id);
        links.installment_id =
            sqlx::query_scalar("SELECT id FROM installments WHERE purchase_transaction_id=?")
                .bind(&e.id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|_| DATABASE)?
                .or(links.installment_id);
    }
    if let Some(old) = imported.and_then(|g| g.links.as_ref()) {
        for (kind, link, old) in [
            (
                "contract",
                &mut links.borrowing_contract_id,
                &old.borrowing_contract_id,
            ),
            (
                "installment",
                &mut links.installment_id,
                &old.installment_id,
            ),
        ] {
            if link.is_none() {
                if let Some(id) = old {
                    let key = meta::key(kind, &[id]);
                    let deleted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND json_type(payload,'$.value')='null') OR EXISTS(SELECT 1 FROM cloud_sync_dirty WHERE record_id=?)").bind(&key).bind(&key).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
                    if !deleted {
                        *link = Some(id.clone());
                    }
                }
            }
        }
    }
    // An older imported linked payment has insufficient information to invent links.
    let unknown = imported.is_some_and(|g| g.protected && g.links.is_none() && g.payroll.is_none())
        && loan.is_none()
        && links == Links::default();
    Ok((loan, if unknown { None } else { Some(links) }))
}
pub async fn basis(conn: &mut SqliteConnection, fields: &Row) -> Result<i64, String> {
    let id = get(fields, "id")?;
    let remaining = number(fields, "basis_principal")?;
    let parts:Vec<i64>=sqlx::query_scalar("SELECT amount FROM loan_payment_parts WHERE contract_id=? AND component='principal' UNION ALL SELECT amount FROM loan_sync_reservations WHERE contract_id=?").bind(id).bind(id).fetch_all(conn).await.map_err(|_|DATABASE)?;
    let total = parts
        .into_iter()
        .fold(i128::from(remaining), |n, p| n + i128::from(p));
    i64::try_from(total).map_err(|_| BLOCKED.into())
}
pub async fn apply_contract(conn: &mut SqliteConnection, v: &Metadata) -> Result<(), String> {
    let f = &v.fields;
    let id = get(f, "id")?;
    let local = meta::current(conn, &format!("contract:{id}")).await?;
    if let Some(Value::Metadata(old)) = &local.value {
        for field in [
            "account_id",
            "principal",
            "borrowing_date",
            "receiving_account_id",
            "borrowing_kind",
        ] {
            if old.fields[field] != f[field] {
                return Err(BLOCKED.into());
            }
        }
        // Only native borrowing reversal may reduce its allocation basis to zero.
        if old.fields["basis_principal"] != f["basis_principal"]
            && !(get(f, "borrowing_kind")? == "new"
                && get(f, "needs_review")? == "1"
                && optional(f, "borrowing_transaction_id").is_none()
                && number(f, "basis_principal")? == 0)
        {
            return Err(BLOCKED.into());
        }
    }
    if let Some(t) = optional(f, "borrowing_transaction_id") {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE id=? AND type='transfer' AND account_id=? AND destination_account_id=? AND amount=? AND date=?)").bind(t).bind(get(f,"account_id")?).bind(get(f,"receiving_account_id")?).bind(number(f,"principal")?).bind(get(f,"borrowing_date")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
        if !valid {
            return Err(BLOCKED.into());
        }
    }
    let mut fields = f.clone();
    fields.remove("basis_principal");
    fields.insert("remaining_principal".into(), Some("0".into()));
    let columns = meta::spec("contract")?
        .columns
        .replace("basis_principal=money", "remaining_principal=money");
    meta::write_row(conn, "loan_contracts", &columns, "id", &fields).await?;
    // Already present legacy ledger rows can be reserved from their authenticated
    // complete shadow until the payment metadata is installed, without posting cash.
    let payloads:Vec<String>=sqlx::query_scalar("SELECT payload FROM cloud_sync_shadow WHERE record_id LIKE 'transaction:%' AND json_type(payload,'$.value')<>'null'").fetch_all(&mut *conn).await.map_err(|_|DATABASE)?;
    for payload in payloads {
        let r: Record = serde_json::from_str(&payload).map_err(|_| DATABASE)?;
        if let Some(Value::Transaction(group)) = r.value {
            let local =
                crate::cloud_sync_store::current(conn, &format!("transaction:{}", r.id)).await?;
            if !matches!(local.value,Some(Value::Transaction(ref existing)) if ledger::same_ledger(existing,&group))
            {
                continue;
            }
            let mut candidates = vec![];
            if let Some(p) = group.loan {
                if p.contract_id.as_deref() == Some(id) {
                    for part in p.parts {
                        if part.component == "principal" {
                            candidates.push((part.transaction_id, ledger::amount(&part.amount)?));
                        }
                    }
                }
            }
            if let Some(p) = group.payroll {
                for line in p.breakdown {
                    if line.contract_id.as_deref() == Some(id) {
                        if let Some(link) = p
                            .principal_entries
                            .iter()
                            .find(|l| l.deduction_id == line.id)
                        {
                            candidates.push((
                                link.transaction_id.clone(),
                                ledger::amount(&line.principal)?,
                            ));
                        }
                    }
                }
            }
            for (transaction, amount) in candidates {
                sqlx::query("INSERT INTO loan_sync_reservations(transaction_id,contract_id,amount) SELECT ?,?,? WHERE EXISTS(SELECT 1 FROM transactions WHERE id=?) AND NOT EXISTS(SELECT 1 FROM loan_payment_parts WHERE transaction_id=?) ON CONFLICT(transaction_id) DO NOTHING").bind(&transaction).bind(id).bind(amount).bind(&transaction).bind(&transaction).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
            }
        }
    }
    let allocated = basis(
        conn,
        &fields
            .iter()
            .map(|(k, v)| {
                (
                    if k == "remaining_principal" {
                        "basis_principal".into()
                    } else {
                        k.clone()
                    },
                    v.clone(),
                )
            })
            .collect(),
    )
    .await?;
    let remaining = number(f, "basis_principal")?
        .checked_sub(allocated)
        .filter(|n| *n >= 0)
        .ok_or(BLOCKED)?;
    sqlx::query("UPDATE loan_contracts SET remaining_principal=? WHERE id=?")
        .bind(remaining)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|_| BLOCKED)?;
    restore_payroll_allocations(conn).await?;
    crate::loans::validate_allocations(conn)
        .await
        .map_err(|_| BLOCKED)?;
    Ok(())
}
pub async fn install(conn: &mut SqliteConnection, group: &TransactionGroup) -> Result<(), String> {
    let Some(p) = &group.loan else {
        return Ok(());
    };
    p.validate(group, &format!("loan-{}", p.id))?;
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='loan')")
            .bind(&p.loan_account_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    if !valid {
        return Err(BLOCKED.into());
    }
    let cash: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank'))",
    )
    .bind(&group.entries[0].account_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !cash {
        return Err(BLOCKED.into());
    }
    for part in &p.parts {
        let existing:Option<(String,Option<String>,String,i64)>=sqlx::query_as("SELECT payment_id,contract_id,component,amount FROM loan_payment_parts WHERE transaction_id=?").bind(&part.transaction_id).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
        if let Some((payment, contract, component, amount)) = existing {
            if payment != p.id
                || contract != p.contract_id
                || component != part.component
                || amount.to_string() != part.amount
            {
                return Err(BLOCKED.into());
            }
            continue;
        }
        if let Some(contract) = &p.contract_id {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE id=? AND account_id=? AND borrowing_date<=?)").bind(contract).bind(&p.loan_account_id).bind(&group.entries[0].date).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !valid {
                return Err(BLOCKED.into());
            }
            if part.component == "principal" {
                consume_principal(
                    conn,
                    &part.transaction_id,
                    contract,
                    ledger::amount(&part.amount)?,
                )
                .await?;
            }
        }
        sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount,loan_account_id) VALUES(?,?,?,?,?,?)").bind(&part.transaction_id).bind(&p.id).bind(&p.contract_id).bind(&part.component).bind(ledger::amount(&part.amount)?).bind(if p.contract_id.is_none(){Some(&p.loan_account_id)}else{None}).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
    }
    Ok(())
}
pub async fn consume_principal(
    conn: &mut SqliteConnection,
    transaction: &str,
    contract: &str,
    amount: i64,
) -> Result<(), String> {
    let reserved: Option<(String, i64)> = sqlx::query_as(
        "SELECT contract_id,amount FROM loan_sync_reservations WHERE transaction_id=?",
    )
    .bind(transaction)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if let Some((c, n)) = reserved {
        if c != contract || n != amount {
            return Err(BLOCKED.into());
        }
        sqlx::query("DELETE FROM loan_sync_reservations WHERE transaction_id=?")
            .bind(transaction)
            .execute(conn)
            .await
            .map_err(|_| DATABASE)?;
    } else {
        let result=sqlx::query("UPDATE loan_contracts SET remaining_principal=remaining_principal-? WHERE id=? AND remaining_principal>=?").bind(amount).bind(contract).bind(amount).execute(conn).await.map_err(|_|BLOCKED)?;
        if result.rows_affected() != 1 {
            return Err(BLOCKED.into());
        }
    }
    Ok(())
}

pub async fn restore_payroll_allocations(conn: &mut SqliteConnection) -> Result<(), String> {
    let payloads: Vec<String> = sqlx::query_scalar(
        "SELECT payload FROM ledger_sync_imports WHERE json_type(payload,'$.payroll')='object'",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    for payload in payloads {
        let group: TransactionGroup = serde_json::from_str(&payload).map_err(|_| DATABASE)?;
        let p = group.payroll.as_ref().ok_or(DATABASE)?;
        p.validate(&group, &format!("salary-{}", p.id))?;
        for link in &p.principal_entries {
            let line = p
                .breakdown
                .iter()
                .find(|d| d.id == link.deduction_id)
                .ok_or(DATABASE)?;
            let Some(contract) = &line.contract_id else {
                continue;
            };
            let amount = ledger::amount(&line.principal)?;
            let reserved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_sync_reservations r JOIN salary_payment_transactions s ON s.transaction_id=r.transaction_id WHERE r.transaction_id=? AND r.contract_id=? AND r.amount=? AND s.payment_id=? AND s.role='principal')")
                .bind(&link.transaction_id).bind(contract).bind(amount).bind(&p.id).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !reserved {
                continue;
            }
            consume_principal(conn, &link.transaction_id, contract, amount).await?;
            sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount) VALUES(?,?,?,'principal',?)")
                .bind(&link.transaction_id).bind(format!("{}:{}",p.id,line.id)).bind(contract).bind(amount).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
        }
    }
    Ok(())
}
