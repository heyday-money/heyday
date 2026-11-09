//! Income definitions and immutable payslips. No forecast edit posts money.
use crate::{
    cloud_sync_ledger::{self as ledger, amount, id, Entry, TransactionGroup, BLOCKED},
    cloud_sync_protocol::UNSAFE,
    cloud_sync_store::{Record, Value, DATABASE},
};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};

pub const CONSENT: &str = "Enable income and payroll sync to download this cloud data.";
pub const CONTRACT:&str="This loan’s contracts are not on this device. Record contract payments on the device with the original loan details.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Deduction {
    pub id: String,
    pub name: String,
    pub description: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub currency: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub destination_account_id: String,
    pub estimated_amount: String,
    pub recurrence_frequency: String,
    pub recurrence_day_of_month: i64,
    pub is_active: bool,
    pub deductions: Vec<Deduction>,
}
fn nonnegative(s: &str) -> Result<i64, String> {
    let n = amount(s)?;
    if n < 0 {
        Err(UNSAFE.into())
    } else {
        Ok(n)
    }
}
fn label(s: &str, max: usize, empty: bool) -> Result<(), String> {
    if (!empty && s.is_empty()) || s.trim() != s || s.chars().count() > max || s.contains('\0') {
        return Err(UNSAFE.into());
    }
    Ok(())
}
impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        ledger::currency(&self.currency)?;
        label(&self.name, 100, false)?;
        id(&self.destination_account_id)?;
        if !["salary", "variable", "investment", "other"].contains(&self.kind.as_str())
            || !["monthly", "irregular"].contains(&self.recurrence_frequency.as_str())
            || (self.recurrence_frequency == "irregular" && self.estimated_amount != "0")
            || !(1..=31).contains(&self.recurrence_day_of_month)
            || self.deductions.len() > 100
            || (self.kind != "salary" && !self.deductions.is_empty())
        {
            return Err(UNSAFE.into());
        }
        let gross = nonnegative(&self.estimated_amount)?;
        let mut total = 0i128;
        let mut previous = "";
        for d in &self.deductions {
            id(&d.id)?;
            label(&d.name, 100, false)?;
            label(&d.description, 2000, true)?;
            if d.id.as_str() <= previous {
                return Err(UNSAFE.into());
            }
            previous = &d.id;
            if let Some(debt) = &d.debt_account_id {
                id(debt)?;
            }
            total += i128::from(nonnegative(&d.amount)?);
        }
        if total > i128::from(gross) {
            return Err(UNSAFE.into());
        }
        Ok(())
    }
    pub fn dependencies(&self) -> Vec<String> {
        std::iter::once(&self.destination_account_id)
            .chain(
                self.deductions
                    .iter()
                    .filter_map(|d| d.debt_account_id.as_ref()),
            )
            .map(|id| format!("account:{id}"))
            .collect()
    }
}
pub async fn current(conn: &mut SqliteConnection, source: &str) -> Result<Record, String> {
    let row:Option<(String,String,String,String,String,i64,bool)>=sqlx::query_as("SELECT name,type,destination_account_id,CAST(estimated_amount AS TEXT),CASE WHEN is_irregular=1 THEN 'irregular' ELSE recurrence_frequency END,recurrence_day_of_month,is_active FROM incomes WHERE id=?").bind(source).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
    let value = if let Some((
        name,
        kind,
        destination_account_id,
        estimated_amount,
        recurrence_frequency,
        recurrence_day_of_month,
        is_active,
    )) = row
    {
        let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
        let deductions=sqlx::query_as("SELECT id,name,description,CAST(amount AS TEXT) AS amount,debt_account_id FROM income_deductions WHERE income_id=? ORDER BY id").bind(source).fetch_all(&mut *conn).await.map_err(|_|DATABASE)?;
        Some(Value::Income(Definition {
            currency,
            name,
            kind,
            destination_account_id,
            estimated_amount,
            recurrence_frequency,
            recurrence_day_of_month,
            is_active,
            deductions,
        }))
    } else {
        None
    };
    Ok(Record {
        schema: 3,
        entity: "income".into(),
        id: source.into(),
        value,
    })
}
pub async fn apply_definition(
    conn: &mut SqliteConnection,
    source: &str,
    value: Option<&Definition>,
) -> Result<(), String> {
    let Some(v) = value else {
        crate::cloud_sync_planner::protect_source(conn, "income", source).await?;
        let deductions: Vec<String> =
            sqlx::query_scalar("SELECT id FROM income_deductions WHERE income_id=?")
                .bind(source)
                .fetch_all(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
        for d in deductions {
            crate::cloud_sync_planner::protect_source(conn, "deduction", &d).await?;
        }
        sqlx::query("DELETE FROM incomes WHERE id=?")
            .bind(source)
            .execute(conn)
            .await
            .map_err(|_| BLOCKED)?;
        return Ok(());
    };
    v.validate()?;
    ledger::check_currency(conn, &v.currency).await?;
    let kind: Option<String> = sqlx::query_scalar("SELECT type FROM incomes WHERE id=?")
        .bind(source)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
    if kind.as_ref().is_some_and(|kind| kind != &v.kind) {
        return Err(BLOCKED.into());
    }
    for dependency in v.dependencies() {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND conflict=1)",
        )
        .bind(dependency)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
        if blocked {
            return Err(BLOCKED.into());
        }
    }
    for d in &v.deductions {
        let owner: Option<String> =
            sqlx::query_scalar("SELECT income_id FROM income_deductions WHERE id=?")
                .bind(&d.id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
        if owner.as_deref().is_some_and(|owner| owner != source) {
            return Err(BLOCKED.into());
        }
        if let Some(debt) = &d.debt_account_id {
            let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card'))").bind(debt).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !exists {
                return Err(BLOCKED.into());
            }
        }
    }
    sqlx::query("INSERT INTO incomes(id,name,type,destination_account_id,estimated_amount,recurrence_frequency,recurrence_day_of_month,is_active,is_auto_create_transaction,is_irregular) VALUES(?,?,?,?,?,'monthly',?,?,0,?) ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,destination_account_id=EXCLUDED.destination_account_id,estimated_amount=EXCLUDED.estimated_amount,is_irregular=EXCLUDED.is_irregular,recurrence_day_of_month=EXCLUDED.recurrence_day_of_month,is_active=EXCLUDED.is_active,is_auto_create_transaction=0,updated_at=CURRENT_TIMESTAMP")
        .bind(source).bind(&v.name).bind(&v.kind).bind(&v.destination_account_id).bind(nonnegative(&v.estimated_amount)?).bind(v.recurrence_day_of_month).bind(v.is_active).bind(v.recurrence_frequency == "irregular").execute(&mut *conn).await.map_err(|_|BLOCKED)?;
    let old: Vec<String> = sqlx::query_scalar("SELECT id FROM income_deductions WHERE income_id=?")
        .bind(source)
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
    for old in old {
        if !v.deductions.iter().any(|d| d.id == old) {
            crate::cloud_sync_planner::protect_source(conn, "deduction", &old).await?;
            sqlx::query("DELETE FROM income_deductions WHERE id=? AND income_id=?")
                .bind(old)
                .bind(source)
                .execute(&mut *conn)
                .await
                .map_err(|_| BLOCKED)?;
        }
    }
    for d in &v.deductions {
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,description,amount,debt_account_id) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,amount=EXCLUDED.amount,debt_account_id=EXCLUDED.debt_account_id")
            .bind(&d.id).bind(source).bind(&d.name).bind(&d.description).bind(nonnegative(&d.amount)?).bind(&d.debt_account_id).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
    }
    // Reattach old ledger-only imports to their now-synchronized source without
    // changing amounts or replacing historical labels.
    let imported = ledger::imported_entries(conn).await?;
    for (entry_id, (_, entry)) in imported {
        if entry.income_source.as_ref().is_some_and(|s| s.id == source) {
            sqlx::query("UPDATE transactions SET income_source_id=? WHERE id=? AND type='income' AND income_source_id IS NULL").bind(source).bind(entry_id).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
        }
    }
    Ok(())
}

// These fields match the immutable native payslip JSON, not editable definitions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayslipLine {
    pub id: String,
    pub name: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
    pub debt_account_name: Option<String>,
    pub principal: String,
    pub interest: String,
    pub fee: String,
    pub contract_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalEntry {
    pub deduction_id: String,
    pub transaction_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payroll {
    pub id: String,
    pub income_id: String,
    pub occurrence: String,
    pub date: String,
    pub gross: String,
    pub net: String,
    pub breakdown: Vec<PayslipLine>,
    pub principal_entries: Vec<PrincipalEntry>,
}
impl Payroll {
    pub fn validate(&self, group: &TransactionGroup, group_id: &str) -> Result<(), String> {
        id(&self.id)?;
        id(&self.income_id)?;
        if group_id != format!("salary-{}", self.id)
            || !group.protected
            || self.breakdown.len() > 100
            || !crate::transactions::valid_date(&self.date)
            || !crate::transactions::valid_date(&format!("{}-01", self.occurrence))
        {
            return Err(UNSAFE.into());
        }
        let gross = nonnegative(&self.gross)?;
        let net = nonnegative(&self.net)?;
        let mut total = 0i128;
        let mut seen = BTreeSet::new();
        let mut expected = BTreeMap::new();
        for line in &self.breakdown {
            id(&line.id)?;
            label(&line.name, 100, false)?;
            if !seen.insert(&line.id) {
                return Err(UNSAFE.into());
            }
            let a = nonnegative(&line.amount)?;
            let p = nonnegative(&line.principal)?;
            let i = nonnegative(&line.interest)?;
            let f = nonnegative(&line.fee)?;
            total += i128::from(a);
            if let Some(debt) = &line.debt_account_id {
                id(debt)?;
                label(line.debt_account_name.as_deref().ok_or(UNSAFE)?, 100, false)?;
                if i128::from(p) + i128::from(i) + i128::from(f) != i128::from(a) {
                    return Err(UNSAFE.into());
                }
                if p > 0 {
                    expected.insert(line.id.as_str(), (debt.as_str(), line.principal.as_str()));
                }
            } else if p != 0
                || i != 0
                || f != 0
                || line.contract_id.is_some()
                || line.debt_account_name.is_some()
            {
                return Err(UNSAFE.into());
            }
            if let Some(contract) = &line.contract_id {
                id(contract)?;
            }
        }
        if i128::from(net) + total != i128::from(gross) {
            return Err(UNSAFE.into());
        }
        let mut links = BTreeMap::new();
        let mut linked = BTreeSet::new();
        for link in &self.principal_entries {
            id(&link.deduction_id)?;
            id(&link.transaction_id)?;
            if links
                .insert(link.deduction_id.as_str(), link.transaction_id.as_str())
                .is_some()
                || !linked.insert(&link.transaction_id)
            {
                return Err(UNSAFE.into());
            }
        }
        if links.len() != expected.len() {
            return Err(UNSAFE.into());
        }
        let net_entries = group
            .entries
            .iter()
            .filter(|e| e.salary_role.as_deref() == Some("net"))
            .collect::<Vec<_>>();
        if net_entries.len() != usize::from(net > 0) {
            return Err(UNSAFE.into());
        }
        for entry in &group.entries {
            if entry.kind != "income"
                || entry.date != self.date
                || entry.payee.is_some()
                || entry.category_id.is_some()
                || entry.destination_account_id.is_some()
            {
                return Err(UNSAFE.into());
            }
            match entry.salary_role.as_deref() {
                Some("net")
                    if entry.amount == self.net
                        && entry
                            .income_source
                            .as_ref()
                            .is_some_and(|s| s.id == self.income_id)
                        && entry.loan_account_id.is_none()
                        && entry.loan_component.is_none() => {}
                Some("principal") if entry.income_source.is_none() => {
                    let deduction = links
                        .iter()
                        .find(|(_, transaction)| **transaction == entry.id)
                        .map(|(d, _)| *d)
                        .ok_or(UNSAFE)?;
                    let (account, principal) = expected.get(deduction).ok_or(UNSAFE)?;
                    if entry.account_id != *account
                        || entry.amount != *principal
                        || entry
                            .loan_account_id
                            .as_deref()
                            .is_some_and(|id| id != *account)
                        || entry
                            .loan_component
                            .as_deref()
                            .is_some_and(|c| c != "principal")
                    {
                        return Err(UNSAFE.into());
                    }
                }
                _ => return Err(UNSAFE.into()),
            }
        }
        if group.entries.len() != expected.len() + usize::from(net > 0) {
            return Err(UNSAFE.into());
        }
        Ok(())
    }
    pub fn dependencies(&self) -> Vec<String> {
        std::iter::once(format!("income:{}", self.income_id))
            .chain(
                self.breakdown
                    .iter()
                    .filter_map(|d| d.debt_account_id.as_ref())
                    .map(|id| format!("account:{id}")),
            )
            .collect()
    }
}
pub async fn load_payroll(
    conn: &mut SqliteConnection,
    group_id: &str,
    entries: &[Entry],
    imported: Option<&Payroll>,
) -> Result<Option<Payroll>, String> {
    let Some(payment) = group_id.strip_prefix("salary-") else {
        return Ok(None);
    };
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT income_enabled FROM cloud_sync_binding WHERE id=1),0)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !enabled {
        return Ok(None);
    }
    let row:Option<(String,String,String,String,String,String)>=sqlx::query_as("SELECT income_id,occurrence,date,CAST(gross AS TEXT),CAST(net AS TEXT),breakdown FROM salary_payments WHERE id=?").bind(payment).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
    let Some((income_id, occurrence, date, gross, net, breakdown)) = row else {
        return Ok(None);
    };
    let breakdown: Vec<PayslipLine> = serde_json::from_str(&breakdown).map_err(|_| DATABASE)?;
    let mut principal_entries = Vec::new();
    let mut used = BTreeSet::new();
    for line in &breakdown {
        let Some(account) = &line.debt_account_id else {
            continue;
        };
        if nonnegative(&line.principal)? == 0 {
            continue;
        }
        let native:Option<String>=sqlx::query_scalar("SELECT transaction_id FROM loan_payment_parts WHERE payment_id=? AND component='principal'").bind(format!("{payment}:{}",line.id)).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
        let exact = native.or_else(|| {
            imported.and_then(|p| {
                p.principal_entries
                    .iter()
                    .find(|p| p.deduction_id == line.id)
                    .map(|p| p.transaction_id.clone())
            })
        });
        let entry = entries
            .iter()
            .find(|e| {
                e.salary_role.as_deref() == Some("principal")
                    && e.account_id == *account
                    && e.amount == line.principal
                    && !used.contains(&e.id)
                    && exact.as_ref().is_none_or(|id| id == &e.id)
            })
            .ok_or(DATABASE)?;
        used.insert(entry.id.clone());
        principal_entries.push(PrincipalEntry {
            deduction_id: line.id.clone(),
            transaction_id: entry.id.clone(),
        });
    }
    principal_entries.sort_by(|a, b| a.deduction_id.cmp(&b.deduction_id));
    Ok(Some(Payroll {
        id: payment.into(),
        income_id,
        occurrence,
        date,
        gross,
        net,
        breakdown,
        principal_entries,
    }))
}
pub async fn install_payroll(
    conn: &mut SqliteConnection,
    group: &TransactionGroup,
) -> Result<(), String> {
    let Some(p) = &group.payroll else {
        return Ok(());
    };
    p.validate(group, &format!("salary-{}", p.id))?;
    let source: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM incomes WHERE id=? AND type='salary')")
            .bind(&p.income_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    let conflict: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND conflict=1)",
    )
    .bind(format!("income:{}", p.income_id))
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !source || conflict {
        return Err(BLOCKED.into());
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE id=?)")
            .bind(&p.id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    if exists {
        return Err(BLOCKED.into());
    } // Callers skip identical installed payslips.
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE income_id=? AND occurrence=?) OR EXISTS(SELECT 1 FROM transactions WHERE income_source_id=? AND (substr(date,1,7)=? OR substr(date,1,7)=?) AND id NOT IN (SELECT transaction_id FROM salary_payment_transactions) AND id NOT IN (SELECT transaction_id FROM ledger_sync_members WHERE group_id=? OR group_id LIKE 'salary-%'))")
        .bind(&p.income_id).bind(&p.occurrence).bind(&p.income_id).bind(&p.occurrence).bind(&p.date[..7]).bind(format!("salary-{}",p.id)).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
    if duplicate {
        return Err(BLOCKED.into());
    }
    sqlx::query("INSERT INTO salary_payments(id,income_id,occurrence,date,gross,net,breakdown) VALUES(?,?,?,?,?,?,?)").bind(&p.id).bind(&p.income_id).bind(&p.occurrence).bind(&p.date).bind(nonnegative(&p.gross)?).bind(nonnegative(&p.net)?).bind(serde_json::to_string(&p.breakdown).map_err(|_|DATABASE)?).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
    for e in &group.entries {
        let kind: String = sqlx::query_scalar("SELECT type FROM accounts WHERE id=?")
            .bind(&e.account_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| BLOCKED)?;
        if e.salary_role.as_deref() == Some("net")
            && !["cash", "bank", "wallet"].contains(&kind.as_str())
        {
            return Err(BLOCKED.into());
        }
        if e.salary_role.as_deref() == Some("principal")
            && !["loan", "credit_card"].contains(&kind.as_str())
        {
            return Err(BLOCKED.into());
        }
        sqlx::query(
            "INSERT INTO salary_payment_transactions(transaction_id,payment_id,role) VALUES(?,?,?)",
        )
        .bind(&e.id)
        .bind(&p.id)
        .bind(&e.salary_role)
        .execute(&mut *conn)
        .await
        .map_err(|_| BLOCKED)?;
    }
    for line in &p.breakdown {
        let Some(account) = &line.debt_account_id else {
            continue;
        };
        let kind: String = sqlx::query_scalar("SELECT type FROM accounts WHERE id=?")
            .bind(account)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| BLOCKED)?;
        if kind == "credit_card"
            && (line.interest != "0" || line.fee != "0" || line.contract_id.is_some())
        {
            return Err(BLOCKED.into());
        }
        if !["credit_card", "loan"].contains(&kind.as_str()) {
            return Err(BLOCKED.into());
        }
        let Some(link) = p
            .principal_entries
            .iter()
            .find(|l| l.deduction_id == line.id)
        else {
            continue;
        };
        if kind != "loan" {
            continue;
        }
        if let Some(contract) = &line.contract_id {
            let found:Option<(String,i64,String)>=sqlx::query_as("SELECT account_id,remaining_principal,borrowing_date FROM loan_contracts WHERE id=?").bind(contract).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
            let Some((owner, remaining, borrowed)) = found else {
                let full: bool = sqlx::query_scalar(
                    "SELECT COALESCE((SELECT full_enabled FROM cloud_sync_binding WHERE id=1),0)",
                )
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
                if full {
                    return Err(BLOCKED.into());
                }
                continue;
            }; // Preserve the immutable link in the payslip until contract sync exists.
            let principal = nonnegative(&line.principal)?;
            let reserved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_sync_reservations WHERE transaction_id=? AND contract_id=? AND amount=?)").bind(&link.transaction_id).bind(contract).bind(principal).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if owner != *account || (!reserved && remaining < principal) || borrowed > p.date {
                return Err(BLOCKED.into());
            }
            crate::cloud_sync_loans::consume_principal(
                conn,
                &link.transaction_id,
                contract,
                principal,
            )
            .await?;
        }
        sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount,loan_account_id) VALUES(?,?,?,'principal',?,?)")
            .bind(&link.transaction_id).bind(format!("{}:{}",p.id,line.id)).bind(&line.contract_id).bind(nonnegative(&line.principal)?).bind(if line.contract_id.is_none(){Some(account)}else{None}).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
    }
    Ok(())
}
pub async fn enable(pool: &SqlitePool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let enabled: bool = sqlx::query_scalar(
        "SELECT income_enabled FROM cloud_sync_binding WHERE id=1 AND financial_enabled=1",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| DATABASE)?;
    if !enabled {
        sqlx::query("UPDATE cloud_sync_binding SET income_enabled=1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
        for query in ["INSERT INTO cloud_sync_dirty(record_id) SELECT 'income:'||id FROM incomes WHERE true ON CONFLICT(record_id) DO UPDATE SET revision=revision+1",
            "INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:salary-'||id FROM salary_payments WHERE true ON CONFLICT(record_id) DO UPDATE SET revision=revision+1"]{
            sqlx::query(query).execute(&mut *tx).await.map_err(|_|DATABASE)?;
        }
    }
    tx.commit().await.map_err(|_| DATABASE.into())
}
pub async fn slot_blocked(conn: &mut SqliteConnection, payroll: &Payroll) -> Result<bool, String> {
    // Distinct offline payments can collide on a business key despite distinct IDs.
    // Keep the losing local operation queued until the cloud payslip is retained
    // or explicitly tombstoned; never upload a second receipt for the same month.
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id<>? AND json_extract(payload,'$.value.payroll.income_id')=? AND json_extract(payload,'$.value.payroll.occurrence')=?)")
        .bind(format!("transaction:salary-{}",payroll.id)).bind(&payroll.income_id).bind(&payroll.occurrence).fetch_one(conn).await.map_err(|_|DATABASE.into())
}

pub async fn require_known_contracts(
    conn: &mut SqliteConnection,
    account: &str,
) -> Result<(), String> {
    let missing:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments p,json_each(p.breakdown) d WHERE json_extract(d.value,'$.debt_account_id')=? AND json_extract(d.value,'$.contract_id') IS NOT NULL AND NOT EXISTS(SELECT 1 FROM loan_contracts c WHERE c.id=json_extract(d.value,'$.contract_id') AND c.account_id=?))")
        .bind(account).bind(account).fetch_one(conn).await.map_err(|_|DATABASE)?;
    if missing {
        Err(CONTRACT.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud_sync_protocol::{self as protocol, Change, Mutation},
        cloud_sync_store as store, salary_payments as salary,
    };
    use zeroize::Zeroizing;
    const ORIGIN: &str = "https://example.test";
    async fn connect(p: &SqlitePool, income: bool) {
        store::enable(p, ORIGIN, "alice", "dataset", "key")
            .await
            .unwrap();
        ledger::enable(p).await.unwrap();
        if income {
            enable(p).await.unwrap();
        }
    }
    async fn empty(income: bool) -> SqlitePool {
        let p = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&p).await.unwrap();
        connect(&p, income).await;
        p
    }
    async fn deliver(p: &SqlitePool, changes: &[Change]) -> Result<(), String> {
        let head = store::binding(p).await?.unwrap().head();
        let mut end = head.clone();
        let mut decoded = Vec::new();
        for c in changes {
            end = protocol::verify_change(c, &end)?;
            decoded.push((
                c.clone(),
                Record::decode(c, &Zeroizing::new([9; 32]), "dataset", "key")?,
            ));
        }
        store::apply_page(p, &head, &decoded, &end, "test", true).await
    }
    fn accepted(request: Mutation) -> Change {
        Change {
            sequence: (request.previous.number().unwrap() + 1).to_string(),
            version: (protocol::number(&request.base_version).unwrap() + 1).to_string(),
            request,
        }
    }
    async fn drain(p: &SqlitePool) -> Vec<Change> {
        let mut changes = Vec::new();
        loop {
            let bound = store::binding(p).await.unwrap().unwrap();
            let Some(request) = store::prepare(p, &Zeroizing::new([9; 32]), &bound)
                .await
                .unwrap()
            else {
                break;
            };
            let c = accepted(request);
            deliver(p, &[c.clone()]).await.unwrap();
            changes.push(c);
            assert!(changes.len() < 200);
        }
        changes
    }
    async fn shared() -> (SqlitePool, SqlitePool) {
        let a = crate::sync::test_support::database().await;
        connect(&a, true).await;
        deliver(&a, &[]).await.unwrap();
        let b = empty(true).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        (a, b)
    }
    async fn balance(p: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(p)
            .await
            .unwrap()
    }
    async fn count(p: &SqlitePool, query: &str) -> i64 {
        sqlx::query_scalar(query).fetch_one(p).await.unwrap()
    }
    async fn record(p: &SqlitePool) -> Record {
        let id: String = sqlx::query_scalar("SELECT id FROM salary_payments LIMIT 1")
            .fetch_one(p)
            .await
            .unwrap();
        store::current(
            &mut *p.acquire().await.unwrap(),
            &format!("transaction:salary-{id}"),
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn income_aggregate_round_trip_conflicts_and_validation_are_atomic() {
        let (a, b) = shared().await;
        assert_eq!(balance(&a).await, balance(&b).await);
        assert_eq!(count(&b, "SELECT count(*) FROM income_deductions").await, 3);
        let before = balance(&b).await;
        sqlx::raw_sql("UPDATE incomes SET estimated_amount=9007199254740993,is_active=0 WHERE id='salary'; UPDATE income_deductions SET description='Updated note' WHERE id='home'; UPDATE accounts SET is_archived=1 WHERE id='student'").execute(&a).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(
            count(&b, "SELECT estimated_amount FROM incomes WHERE id='salary'").await,
            9007199254740993
        );
        assert_eq!(
            count(&b, "SELECT is_active FROM incomes WHERE id='salary'").await,
            0
        );
        assert_eq!(balance(&b).await, before);
        let mut r = current(&mut *a.acquire().await.unwrap(), "salary")
            .await
            .unwrap();
        if let Some(Value::Income(v)) = &mut r.value {
            v.name = "Must rollback".into();
            v.deductions[0].debt_account_id = Some("missing".into());
        }
        assert!(!ledger::apply(&mut *b.acquire().await.unwrap(), &r, false)
            .await
            .unwrap());
        assert_eq!(
            current(&mut *a.acquire().await.unwrap(), "salary")
                .await
                .unwrap(),
            current(&mut *b.acquire().await.unwrap(), "salary")
                .await
                .unwrap()
        );
        sqlx::query("UPDATE incomes SET name='On A' WHERE id='salary'")
            .execute(&a)
            .await
            .unwrap();
        sqlx::query("UPDATE incomes SET name='On B' WHERE id='salary'")
            .execute(&b)
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        let status = store::status(&b, ORIGIN, "alice").await.unwrap();
        assert_eq!(status.conflict_count, 1);
        let c = &status.conflicts[0];
        assert_eq!(c.entity, "income");
        store::resolve(
            &b,
            &c.record_id,
            c.local_revision,
            c.remote_version,
            "remote",
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            current(&mut *a.acquire().await.unwrap(), "salary")
                .await
                .unwrap(),
            current(&mut *b.acquire().await.unwrap(), "salary")
                .await
                .unwrap()
        );
    }
    #[tokio::test]
    async fn full_payroll_round_trip_and_remote_reversal_restore_balances() {
        let (a, b) = shared().await;
        let before = balance(&a).await;
        salary::record(&a, crate::sync::test_support::input())
            .await
            .unwrap();
        let changes = drain(&a).await;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].request.protocol, 3);
        deliver(&b, &changes).await.unwrap();
        assert_eq!(balance(&a).await, balance(&b).await);
        assert_eq!(record(&a).await, record(&b).await);
        assert_eq!(
            count(&b, "SELECT count(*) FROM loan_payment_parts").await,
            2
        );
        let tx: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='net'",
        )
        .fetch_one(&b)
        .await
        .unwrap();
        crate::transactions::remove_confirmed(&b, &tx, true)
            .await
            .unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(balance(&a).await, before);
        assert_eq!(balance(&b).await, before);
        assert_eq!(count(&a, "SELECT count(*) FROM salary_payments").await, 0);
        assert_eq!(
            count(
                &a,
                "SELECT count(*) FROM cloud_sync_shadow WHERE conflict=1"
            )
            .await,
            0
        );
    }
    #[tokio::test]
    async fn zero_ledger_payslip_is_synced_and_deleted() {
        let (a, b) = shared().await;
        let before = balance(&a).await;
        let mut input = crate::sync::test_support::input();
        input.gross = "0".into();
        for d in &mut input.deductions {
            d.amount = "0".into();
            d.interest = "0".into();
            d.fee = "0".into();
        }
        salary::record(&a, input).await.unwrap();
        let changes = drain(&a).await;
        assert_eq!(changes.len(), 1);
        deliver(&b, &changes).await.unwrap();
        assert_eq!(count(&b, "SELECT count(*) FROM salary_payments").await, 1);
        assert_eq!(count(&b, "SELECT count(*) FROM transactions").await, 0);
        sqlx::query("DELETE FROM salary_payments")
            .execute(&b)
            .await
            .unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(count(&a, "SELECT count(*) FROM salary_payments").await, 0);
        assert_eq!(balance(&a).await, before);
        assert_eq!(balance(&b).await, before);
    }
    #[tokio::test]
    async fn legacy_payroll_upgrade_preserves_pending_ciphertext_and_does_not_post_twice() {
        let a = crate::sync::test_support::database().await;
        connect(&a, false).await;
        deliver(&a, &[]).await.unwrap();
        let b = empty(false).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        salary::record(&a, crate::sync::test_support::input())
            .await
            .unwrap();
        let bound = store::binding(&a).await.unwrap().unwrap();
        let pending = store::prepare(&a, &Zeroizing::new([9; 32]), &bound)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pending.protocol, 2);
        enable(&a).await.unwrap();
        assert_eq!(store::pending(&a).await.unwrap().unwrap(), pending);
        let c = accepted(pending);
        deliver(&a, &[c.clone()]).await.unwrap();
        deliver(&b, &[c]).await.unwrap();
        let after = balance(&b).await;
        let upgrade = drain(&a).await;
        assert_eq!(upgrade.len(), 2);
        assert_eq!(deliver(&b, &upgrade).await.unwrap_err(), CONSENT);
        enable(&b).await.unwrap();
        deliver(&b, &upgrade).await.unwrap();
        assert_eq!(balance(&b).await, after);
        assert_eq!(record(&a).await, record(&b).await);
        assert_eq!(count(&b, "SELECT count(*) FROM salary_payments").await, 1);
        assert!(drain(&b).await.is_empty());
    }
    #[tokio::test]
    async fn simultaneous_monthly_payroll_requires_explicit_choice_before_upload() {
        let (a, b) = shared().await;
        salary::record(&a, crate::sync::test_support::input())
            .await
            .unwrap();
        salary::record(&b, crate::sync::test_support::input())
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        let status = store::status(&b, ORIGIN, "alice").await.unwrap();
        assert_eq!(status.conflict_count, 1);
        assert!(drain(&b).await.is_empty());
        let c = &status.conflicts[0];
        assert!(c.local.is_none());
        assert!(store::resolve(
            &b,
            &c.record_id,
            c.local_revision,
            c.remote_version,
            "remote",
            None
        )
        .await
        .is_err());
        store::resolve(
            &b,
            &c.record_id,
            c.local_revision,
            c.remote_version,
            "local",
            None,
        )
        .await
        .unwrap();
        let changes = drain(&b).await;
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].request.operation, "delete");
        deliver(&a, &changes).await.unwrap();
        assert_eq!(record(&a).await, record(&b).await);
        assert_eq!(balance(&a).await, balance(&b).await);
    }
    #[tokio::test]
    async fn missing_contract_stays_in_payslip_and_reversal_restores_origin_allocation() {
        let (a, b) = shared().await;
        sqlx::raw_sql("UPDATE accounts SET loan_type='personal_loan' WHERE id='home'; INSERT INTO loan_facilities VALUES('home',3000000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','home','Contract',2000000,2000000,'2023-01-01','bank','bank',0,920000,12,'2024-01-25','existing');").execute(&a).await.unwrap();
        let mut input = crate::sync::test_support::input();
        input.gross = "9007199254740993".into();
        input.deductions[0].contract_id = Some("contract".into());
        salary::record(&a, input).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(record(&a).await, record(&b).await);
        assert_eq!(balance(&a).await, balance(&b).await);
        assert_eq!(
            count(&a, "SELECT remaining_principal FROM loan_contracts").await,
            1300000
        );
        let mut next = crate::sync::test_support::input();
        next.occurrence = "2024-02".into();
        next.date = "2024-02-25".into();
        assert_eq!(salary::record(&b, next).await.unwrap_err(), CONTRACT);
        let tx: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='principal' LIMIT 1",
        )
        .fetch_one(&b)
        .await
        .unwrap();
        crate::transactions::remove_confirmed(&b, &tx, true)
            .await
            .unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(
            count(&a, "SELECT remaining_principal FROM loan_contracts").await,
            2000000
        );
        assert_eq!(balance(&a).await, balance(&b).await);
    }
    #[tokio::test]
    async fn invalid_payslip_roles_amounts_and_dependencies_never_partially_apply() {
        let (a, b) = shared().await;
        salary::record(&a, crate::sync::test_support::input())
            .await
            .unwrap();
        let original = record(&a).await;
        let before = balance(&b).await;
        for invalid in 0..4 {
            let mut r = original.clone();
            if let Some(Value::Transaction(g)) = &mut r.value {
                match invalid {
                    0 => g.payroll.as_mut().unwrap().net = "1".into(),
                    1 => g.entries[0].salary_role = None,
                    2 => g.payroll.as_mut().unwrap().principal_entries.clear(),
                    _ => g.payroll.as_mut().unwrap().breakdown[0].principal = "-1".into(),
                }
            }
            assert!(r.validate().is_err());
        }
        let mut missing = original.clone();
        if let Some(Value::Transaction(g)) = &mut missing.value {
            let p = g.payroll.as_mut().unwrap();
            p.income_id = "missing".into();
            for e in &mut g.entries {
                if let Some(source) = &mut e.income_source {
                    source.id = "missing".into();
                }
            }
        }
        assert!(
            !ledger::apply(&mut *b.acquire().await.unwrap(), &missing, false)
                .await
                .unwrap()
        );
        assert_eq!(balance(&b).await, before);
        assert_eq!(count(&b, "SELECT count(*) FROM transactions").await, 0);
        assert_eq!(count(&b, "SELECT count(*) FROM salary_payments").await, 0);
    }
    #[tokio::test]
    async fn two_salary_months_paid_on_the_same_day_are_distinct_receipts() {
        let (a, b) = shared().await;
        salary::record(&a, crate::sync::test_support::input())
            .await
            .unwrap();
        let mut late = crate::sync::test_support::input();
        late.occurrence = "2023-12".into();
        salary::record(&a, late).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(count(&b, "SELECT count(*) FROM salary_payments").await, 2);
        assert_eq!(
            count(
                &b,
                "SELECT count(*) FROM cloud_sync_shadow WHERE conflict=1"
            )
            .await,
            0
        );
        assert_eq!(balance(&a).await, balance(&b).await);
    }
    #[tokio::test]
    async fn definition_edits_preserve_retained_cycle_overrides_and_rollback_partial_writes() {
        let (a, b) = shared().await;
        sqlx::raw_sql("INSERT INTO planner_deduction_amounts VALUES('tax','2024-01',123); INSERT INTO planner_income_amounts VALUES('salary','2024-01',456)").execute(&b).await.unwrap();
        sqlx::raw_sql(
            "UPDATE income_deductions SET name='Tax edited',amount=200000 WHERE id='tax'",
        )
        .execute(&a)
        .await
        .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(
            count(&b, "SELECT amount FROM planner_deduction_amounts").await,
            123
        );
        assert_eq!(
            count(&b, "SELECT amount FROM planner_income_amounts").await,
            456
        );
        let before = current(&mut *b.acquire().await.unwrap(), "salary")
            .await
            .unwrap();
        let mut incoming = before.clone();
        if let Some(Value::Income(v)) = &mut incoming.value {
            v.name = "Partial edit".into();
            v.deductions[0].description = "Partial note".into();
        }
        sqlx::query("CREATE TRIGGER fail_definition BEFORE UPDATE ON income_deductions WHEN NEW.id='tax' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&b).await.unwrap();
        assert!(
            !ledger::apply(&mut *b.acquire().await.unwrap(), &incoming, false)
                .await
                .unwrap()
        );
        assert_eq!(
            current(&mut *b.acquire().await.unwrap(), "salary")
                .await
                .unwrap(),
            before
        );
        sqlx::query("DROP TRIGGER fail_definition")
            .execute(&b)
            .await
            .unwrap();
        sqlx::query("DELETE FROM income_deductions WHERE id='tax'")
            .execute(&a)
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(
            count(&b, "SELECT count(*) FROM planner_deduction_amounts").await,
            0
        );
        assert_eq!(
            count(&b, "SELECT amount FROM planner_income_amounts").await,
            456
        );
    }
    #[tokio::test]
    async fn largest_supported_income_definition_fits_encrypted_protocol_three() {
        let a = crate::sync::test_support::database().await;
        connect(&a, true).await;
        let mut r = current(&mut *a.acquire().await.unwrap(), "salary")
            .await
            .unwrap();
        if let Some(Value::Income(v)) = &mut r.value {
            v.deductions = (0..100)
                .map(|n| Deduction {
                    id: format!("deduction-{n:03}"),
                    name: "𝄞".repeat(100),
                    description: "\u{1}".repeat(2000),
                    amount: "1".into(),
                    debt_account_id: None,
                })
                .collect();
        }
        r.validate().unwrap();
        let bytes = serde_json::to_vec(&r).unwrap();
        assert!(bytes.len() > 262144);
        let m = protocol::encrypt_with_protocol(
            &Zeroizing::new([9; 32]),
            "dataset",
            "key",
            &r.key(),
            0,
            &protocol::Head::default(),
            false,
            &bytes,
            3,
        )
        .unwrap();
        assert_eq!(
            Record::decode(&accepted(m), &Zeroizing::new([9; 32]), "dataset", "key").unwrap(),
            r
        );
    }
}
