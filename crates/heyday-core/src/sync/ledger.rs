//! Ledger records are incremental. A transfer or a linked payment is one record;
//! its entries and derived account balances always commit together.
use crate::cloud_sync_protocol::UNSAFE;
use crate::cloud_sync_store::{Category, Record, Value, DATABASE};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};

pub const CONSENT: &str = "Enable account and transaction sync to download this cloud data.";
pub const BLOCKED: &str = "This financial change needs review. Check the currency, linked accounts, and transaction history before choosing a version.";
pub const PROTECTED: &str = "Delete this linked payment on the device with its payroll, loan, or installment details, then sync again.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub currency: String,
    pub opening_balance: String,
    pub is_archived: bool,
    pub institution: Option<String>,
    pub notes: Option<String>,
    pub credit_limit: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub loan_type: Option<String>,
    pub last_four: Option<String>,
    pub interest_rate_ten_thousandths: Option<i64>,
    pub monthly_installment: Option<String>,
    pub initial_loan_amount: Option<String>,
}
pub use crate::database::ledger::{Entry, Label};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionGroup {
    pub currency: String,
    pub protected: bool,
    pub entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payroll: Option<crate::cloud_sync_income::Payroll>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loan: Option<crate::cloud_sync_loans::Payment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<crate::cloud_sync_loans::Links>,
}
pub fn id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(UNSAFE.into());
    }
    Ok(())
}
pub fn amount(value: &str) -> Result<i64, String> {
    let n = value.parse::<i64>().map_err(|_| UNSAFE)?;
    if n.to_string() != value {
        return Err(UNSAFE.into());
    }
    Ok(n)
}
fn text(value: &str, limit: usize, empty: bool) -> Result<(), String> {
    if (!empty && value.is_empty())
        || value.trim() != value
        || value.chars().count() > limit
        || value.contains('\0')
    {
        return Err(UNSAFE.into());
    }
    Ok(())
}
pub fn currency(value: &str) -> Result<(), String> {
    if ![
        "THB", "USD", "EUR", "GBP", "JPY", "CNY", "SGD", "AUD", "CAD", "CHF", "KRW", "INR", "VND",
        "KWD", "BHD",
    ]
    .contains(&value)
    {
        return Err(UNSAFE.into());
    }
    Ok(())
}
impl Account {
    pub fn validate(&self) -> Result<(), String> {
        currency(&self.currency)?;
        text(&self.name, 100, false)?;
        for (v, n) in [(&self.institution, 100), (&self.notes, 1000)] {
            if let Some(v) = v {
                text(v, n, false)?;
            }
        }
        for v in [&self.opening_balance].into_iter().chain(
            [
                &self.credit_limit,
                &self.monthly_installment,
                &self.initial_loan_amount,
            ]
            .into_iter()
            .filter_map(|v| v.as_ref()),
        ) {
            amount(v)?;
        }
        // Use the same domain validation as account creation, including debt fields.
        let input = serde_json::from_value::<crate::accounts::NewAccount>(
            serde_json::to_value(self).map_err(|_| UNSAFE)?,
        )
        .map_err(|_| UNSAFE)?;
        crate::accounts::validate_account(input, true).map_err(|_| UNSAFE)?;
        Ok(())
    }
}
impl TransactionGroup {
    pub fn validate(&self, group: &str) -> Result<(), String> {
        currency(&self.currency)?;
        if (self.entries.is_empty() && self.payroll.is_none()) || self.entries.len() > 256 {
            return Err(UNSAFE.into());
        }
        // Grouped historical payments are protected on devices without definitions.
        if !self.protected && (self.entries.len() != 1 || self.entries[0].id != group) {
            return Err(UNSAFE.into());
        }
        let mut seen = BTreeSet::new();
        let mut last = "";
        for e in &self.entries {
            id(&e.id)?;
            id(&e.account_id)?;
            if !seen.insert(&e.id) || e.id.as_str() <= last {
                return Err(UNSAFE.into());
            }
            last = &e.id;
            if !["income", "expense", "transfer", "repayment"].contains(&e.kind.as_str())
                || amount(&e.amount)? <= 0
                || !crate::transactions::valid_date(&e.date)
            {
                return Err(UNSAFE.into());
            }
            text(&e.description, 200, true)?;
            let paired = matches!(e.kind.as_str(), "transfer" | "repayment");
            if paired != e.destination_account_id.is_some()
                || e.destination_account_id.as_ref() == Some(&e.account_id)
            {
                return Err(UNSAFE.into());
            }
            for v in [
                &e.destination_account_id,
                &e.category_id,
                &e.loan_account_id,
            ]
            .into_iter()
            .flatten()
            {
                id(v)?;
            }
            if e.kind != "expense" && (e.payee.is_some() || e.category_id.is_some()) {
                return Err(UNSAFE.into());
            }
            if e.loan_component
                .as_deref()
                .is_some_and(|v| !["principal", "interest", "fee"].contains(&v))
                || (e.loan_component.is_some() && (e.loan_account_id.is_none() || !self.protected))
            {
                return Err(UNSAFE.into());
            }
            if e.salary_role
                .as_deref()
                .is_some_and(|role| !["net", "principal"].contains(&role))
                || (e.salary_role.is_some() && (!self.protected || e.kind != "income"))
            {
                return Err(UNSAFE.into());
            }
            if e.income_source.is_some() && e.kind != "income" {
                return Err(UNSAFE.into());
            }
            for label in [&e.payee, &e.income_source].into_iter().flatten() {
                id(&label.id)?;
                text(&label.name, 100, false)?;
            }
        }
        if let Some(loan) = &self.loan {
            loan.validate(self, group)?;
        }
        if let Some(links) = &self.links {
            for link in [&links.borrowing_contract_id, &links.installment_id]
                .into_iter()
                .flatten()
            {
                id(link)?;
            }
            if (links.borrowing_contract_id.is_some() || links.installment_id.is_some())
                && (self.entries.len() != 1 || self.loan.is_some() || self.payroll.is_some())
            {
                return Err(UNSAFE.into());
            }
            if self.loan.is_none()
                && self.payroll.is_none()
                && self
                    .entries
                    .iter()
                    .any(|e| e.loan_component.is_some() || e.salary_role.is_some())
            {
                return Err(UNSAFE.into());
            }
        }
        if let Some(payroll) = &self.payroll {
            payroll.validate(self, group)?;
        }
        Ok(())
    }
    pub fn accounts(&self) -> BTreeSet<&str> {
        self.entries
            .iter()
            .flat_map(|e| {
                std::iter::once(e.account_id.as_str())
                    .chain(e.destination_account_id.as_deref())
                    .chain(e.loan_account_id.as_deref())
            })
            .collect()
    }
}
pub async fn enable(pool: &SqlitePool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let enabled: bool =
        sqlx::query_scalar("SELECT financial_enabled FROM cloud_sync_binding WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    if !enabled {
        verify_balances(&mut tx).await?;
        sqlx::query("UPDATE cloud_sync_binding SET financial_enabled=1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
        for query in ["INSERT INTO cloud_sync_dirty(record_id) SELECT 'account:'||id FROM accounts WHERE true ON CONFLICT DO NOTHING",
            "INSERT INTO cloud_sync_dirty(record_id) SELECT DISTINCT 'transaction:'||m.group_id FROM ledger_sync_members m JOIN transactions t ON t.id=m.transaction_id WHERE true ON CONFLICT DO NOTHING"] {
            sqlx::query(query).execute(&mut *tx).await.map_err(|_| DATABASE)?;
        }
    }
    tx.commit().await.map_err(|_| DATABASE.into())
}
async fn verify_balances(conn: &mut SqliteConnection) -> Result<(), String> {
    let accounts: Vec<(String, String, i64, i64)> =
        sqlx::query_as("SELECT id,type,opening_balance,current_balance FROM accounts")
            .fetch_all(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    let rows: Vec<(String, String, Option<String>, i64)> =
        sqlx::query_as("SELECT type,account_id,destination_account_id,amount FROM transactions")
            .fetch_all(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    let mut changes: BTreeMap<String, i128> = BTreeMap::new();
    for (kind, source, destination, amount) in rows {
        *changes.entry(source).or_default() += if kind == "income" {
            i128::from(amount)
        } else {
            -i128::from(amount)
        };
        if let Some(id) = destination {
            *changes.entry(id).or_default() += i128::from(amount);
        }
    }
    for (id, kind, opening, balance) in accounts {
        let inflow = changes.get(&id).copied().unwrap_or(0);
        let expected = i128::from(opening)
            + if matches!(kind.as_str(), "loan" | "credit_card") {
                -inflow
            } else {
                inflow
            };
        if expected != i128::from(balance) {
            return Err("Account balances do not match transaction history. Review your local data before enabling financial sync.".into());
        }
    }
    Ok(())
}
pub async fn current(
    conn: &mut SqliteConnection,
    entity: &str,
    local_id: &str,
) -> Result<Record, String> {
    let value=match entity {
        "account" => sqlx::query_as::<_,Account>("SELECT name,type AS kind,(SELECT currency FROM settings WHERE id=1) AS currency,CAST(opening_balance AS TEXT) AS opening_balance,is_archived,institution,notes,CAST(credit_limit AS TEXT) AS credit_limit,statement_day,payment_due_day,loan_type,last_four,interest_rate_ten_thousandths,CAST(monthly_installment AS TEXT) AS monthly_installment,CAST(initial_loan_amount AS TEXT) AS initial_loan_amount FROM accounts WHERE id=?")
            .bind(local_id).fetch_optional(&mut *conn).await.map_err(|_| DATABASE)?.map(Value::Account),
        "transaction" => group(conn,local_id).await?.map(Value::Transaction),
        _=>return Err(UNSAFE.into()),
    };
    let income: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT income_enabled FROM cloud_sync_binding WHERE id=1),0)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    let full: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT full_enabled FROM cloud_sync_binding WHERE id=1),0)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    let schema = if matches!(&value,Some(Value::Transaction(g)) if g.links.is_some())
        || (full && entity == "transaction" && value.is_none())
    {
        5
    } else if matches!(&value,Some(Value::Transaction(g)) if g.payroll.is_some())
        || (income && value.is_none() && entity == "transaction" && local_id.starts_with("salary-"))
    {
        3
    } else {
        2
    };
    Ok(Record {
        schema,
        entity: entity.into(),
        id: local_id.into(),
        value,
    })
}
async fn group(
    conn: &mut SqliteConnection,
    group_id: &str,
) -> Result<Option<TransactionGroup>, String> {
    let imported: Option<String> =
        sqlx::query_scalar("SELECT payload FROM ledger_sync_imports WHERE group_id=?")
            .bind(group_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    let imported = imported
        .map(|s| serde_json::from_str::<TransactionGroup>(&s).map_err(|_| DATABASE))
        .transpose()?;
    let rows=sqlx::query("SELECT t.*,(SELECT role FROM salary_payment_transactions WHERE transaction_id=t.id) AS salary_role,lp.component AS loan_component,p.name AS payee_name,i.name AS income_name,COALESCE(lp.loan_account_id,lc.account_id) AS linked_loan, EXISTS(SELECT 1 FROM salary_payment_transactions WHERE transaction_id=t.id) OR lp.transaction_id IS NOT NULL OR EXISTS(SELECT 1 FROM loan_contracts WHERE borrowing_transaction_id=t.id) OR EXISTS(SELECT 1 FROM installments WHERE purchase_transaction_id=t.id) AS protected FROM ledger_sync_members m JOIN transactions t ON t.id=m.transaction_id LEFT JOIN payees p ON p.id=t.payee_id LEFT JOIN incomes i ON i.id=t.income_source_id LEFT JOIN loan_payment_parts lp ON lp.transaction_id=t.id LEFT JOIN loan_contracts lc ON lc.id=lp.contract_id WHERE m.group_id=? ORDER BY t.id")
        .bind(group_id).fetch_all(&mut *conn).await.map_err(|_| DATABASE)?;
    let mut protected = imported.as_ref().is_some_and(|g| g.protected);
    let mut entries = Vec::new();
    for row in rows {
        let row_id: String = row.get("id");
        let old = imported
            .as_ref()
            .and_then(|g| g.entries.iter().find(|e| e.id == row_id));
        protected |= row.get::<bool, _>("protected");
        let label = |id_col, name_col| -> Option<Label> {
            row.get::<Option<String>, _>(id_col)
                .zip(row.get::<Option<String>, _>(name_col))
                .map(|(id, name)| Label { id, name })
        };
        entries.push(Entry {
            id: row_id,
            kind: row.get("type"),
            account_id: row.get("account_id"),
            destination_account_id: row.get("destination_account_id"),
            amount: row.get::<i64, _>("amount").to_string(),
            date: row.get("date"),
            description: row.get("description"),
            payee: label("payee_id", "payee_name").or_else(|| old.and_then(|e| e.payee.clone())),
            category_id: row.get("category_id"),
            income_source: label("income_source_id", "income_name")
                .or_else(|| old.and_then(|e| e.income_source.clone())),
            loan_component: row
                .get::<Option<String>, _>("loan_component")
                .or_else(|| old.and_then(|e| e.loan_component.clone())),
            salary_role: row
                .get::<Option<String>, _>("salary_role")
                .or_else(|| old.and_then(|e| e.salary_role.clone())),
            loan_account_id: row
                .get::<Option<String>, _>("linked_loan")
                .or_else(|| old.and_then(|e| e.loan_account_id.clone())),
        });
    }
    let payroll = crate::cloud_sync_income::load_payroll(
        conn,
        group_id,
        &entries,
        imported.as_ref().and_then(|g| g.payroll.as_ref()),
    )
    .await?;
    if entries.is_empty() && payroll.is_none() {
        return Ok(None);
    }
    protected |= payroll.is_some();
    let (loan, links) =
        crate::cloud_sync_loans::load(conn, group_id, &entries, imported.as_ref()).await?;
    if let Some(links) = &links {
        protected = payroll.is_some()
            || loan.is_some()
            || links.borrowing_contract_id.is_some()
            || links.installment_id.is_some();
    }
    let currency = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(conn)
        .await
        .map_err(|_| DATABASE)?;
    Ok(Some(TransactionGroup {
        currency,
        protected,
        entries,
        payroll,
        loan,
        links,
    }))
}
// Labels may be renamed independently; that must not turn identical ledger rows
// into financial conflicts or overwrite a newer local name.
pub fn same(a: &TransactionGroup, b: &TransactionGroup) -> bool {
    let normalize = |g: &TransactionGroup| {
        let mut g = g.clone();
        for e in &mut g.entries {
            if let Some(v) = &mut e.payee {
                v.name.clear();
            }
            if let Some(v) = &mut e.income_source {
                v.name.clear();
            }
        }
        g
    };
    normalize(a) == normalize(b)
}
pub fn same_ledger(a: &TransactionGroup, b: &TransactionGroup) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.payroll = None;
    b.payroll = None;
    a.loan = None;
    b.loan = None;
    a.links = None;
    b.links = None;
    a.protected = false;
    b.protected = false;
    same(&a, &b)
}
pub async fn check_currency(conn: &mut SqliteConnection, value: &str) -> Result<(), String> {
    let local: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
    if local.as_deref() == Some(value) {
        return Ok(());
    }
    if local.is_some() {
        return Err(BLOCKED.into());
    }
    let any: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts) OR EXISTS(SELECT 1 FROM incomes)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if any {
        return Err(BLOCKED.into());
    }
    sqlx::query("UPDATE settings SET currency=? WHERE id=1")
        .bind(value)
        .execute(conn)
        .await
        .map_err(|_| DATABASE)?;
    Ok(())
}
async fn apply_account(
    conn: &mut SqliteConnection,
    id: &str,
    value: Option<&Account>,
) -> Result<(), String> {
    let Some(v) = value else {
        // Accounts are archived by the UI; retain independent child forecasts.
        crate::cloud_sync_metadata_rules::guard_references(conn, &format!("account:{id}")).await?;
        sqlx::query("DELETE FROM accounts WHERE id=?")
            .bind(id)
            .execute(conn)
            .await
            .map_err(|_| BLOCKED)?;
        return Ok(());
    };
    v.validate()?;
    check_currency(conn, &v.currency).await?;
    let old: Option<(String, i64)> =
        sqlx::query_as("SELECT type,opening_balance FROM accounts WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    if old
        .as_ref()
        .is_some_and(|(kind, opening)| kind != &v.kind || opening.to_string() != v.opening_balance)
    {
        return Err(BLOCKED.into());
    }
    if !v.is_archived {
        let paid: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_payoffs WHERE account_id=?)")
                .bind(id)
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
        if paid {
            return Err(BLOCKED.into());
        }
    }
    // Immutable opening balance/type. Never copy a separately editable current balance.
    sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance,is_archived,institution,notes,credit_limit,statement_day,payment_due_day,loan_type,last_four,interest_rate_ten_thousandths,monthly_installment,initial_loan_amount) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,is_archived=EXCLUDED.is_archived,institution=EXCLUDED.institution,notes=EXCLUDED.notes,credit_limit=EXCLUDED.credit_limit,statement_day=EXCLUDED.statement_day,payment_due_day=EXCLUDED.payment_due_day,loan_type=EXCLUDED.loan_type,last_four=EXCLUDED.last_four,interest_rate_ten_thousandths=EXCLUDED.interest_rate_ten_thousandths,monthly_installment=EXCLUDED.monthly_installment,initial_loan_amount=EXCLUDED.initial_loan_amount,updated_at=CURRENT_TIMESTAMP")
        .bind(id).bind(&v.name).bind(&v.kind).bind(amount(&v.opening_balance)?).bind(amount(&v.opening_balance)?).bind(v.is_archived).bind(&v.institution).bind(&v.notes).bind(v.credit_limit.as_deref().map(amount).transpose()?).bind(v.statement_day).bind(v.payment_due_day).bind(&v.loan_type).bind(&v.last_four).bind(v.interest_rate_ten_thousandths).bind(v.monthly_installment.as_deref().map(amount).transpose()?).bind(v.initial_loan_amount.as_deref().map(amount).transpose()?).execute(&mut *conn).await.map_err(|_| BLOCKED)?;
    // Full sync owns institution links in a separate record. Legacy label-only
    // imports may clear a mismatched local link; full sync must preserve it.
    sqlx::query("UPDATE accounts SET institution_id=NULL WHERE id=? AND NOT EXISTS(SELECT 1 FROM cloud_sync_binding WHERE full_enabled=1) AND institution_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM institutions WHERE institutions.id=accounts.institution_id AND institutions.name=accounts.institution)").bind(id).execute(conn).await.map_err(|_| DATABASE)?;
    Ok(())
}
fn deltas(
    group: &TransactionGroup,
    sign: i128,
    changes: &mut BTreeMap<String, i128>,
) -> Result<(), String> {
    for e in &group.entries {
        let value = i128::from(amount(&e.amount)?) * sign;
        *changes.entry(e.account_id.clone()).or_default() +=
            if e.kind == "income" { value } else { -value };
        if let Some(id) = &e.destination_account_id {
            *changes.entry(id.clone()).or_default() += value;
        }
    }
    Ok(())
}
async fn apply_group(
    conn: &mut SqliteConnection,
    id: &str,
    value: Option<&TransactionGroup>,
    confirmed: bool,
) -> Result<(), String> {
    let old = group(conn, id).await?;
    if let (Some(a), Some(b)) = (&old, value) {
        if same(a, b) {
            return Ok(());
        }
        // Protocol-2 payroll receives its missing metadata without posting cash again.
        if same_ledger(a, b)
            && (a.payroll.is_none() && b.payroll.is_some()
                || b.links.is_some() && (a.loan.is_none() || a.loan == b.loan))
        {
            if a.payroll != b.payroll && a.payroll.is_some() {
                return Err(BLOCKED.into());
            }
            if a.payroll.is_none() {
                crate::cloud_sync_income::install_payroll(conn, b).await?;
            }
            crate::cloud_sync_loans::install(conn, b).await?;
            sqlx::query("INSERT INTO ledger_sync_imports(group_id,payload) VALUES(?,?) ON CONFLICT(group_id) DO UPDATE SET payload=EXCLUDED.payload").bind(id).bind(serde_json::to_string(b).map_err(|_|DATABASE)?).execute(&mut *conn).await.map_err(|_|DATABASE)?;
            crate::loans::validate_allocations(conn)
                .await
                .map_err(|_| BLOCKED)?;
            return Ok(());
        }
        // Ledger rows and full payslips are immutable.
        return Err(BLOCKED.into());
    }
    if value.is_none() {
        if let Some(group) = old
            .as_ref()
            .filter(|g| g.links.is_some() && !g.entries.is_empty())
        {
            crate::transactions::remove_in_connection(conn, &group.entries[0].id, confirmed)
                .await
                .map_err(|_| BLOCKED)?;
            sqlx::query("DELETE FROM ledger_sync_imports WHERE group_id=?")
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
            return Ok(());
        }
        if let Some(payroll) = old.as_ref().and_then(|g| g.payroll.as_ref()) {
            if let Some(entry) = old.as_ref().and_then(|g| g.entries.first()) {
                crate::transactions::remove_in_connection(conn, &entry.id, confirmed)
                    .await
                    .map_err(|_| BLOCKED)?;
            } else {
                sqlx::query("DELETE FROM salary_payments WHERE id=?")
                    .bind(&payroll.id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|_| BLOCKED)?;
            }
            sqlx::query("DELETE FROM ledger_sync_imports WHERE group_id=?")
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
            return Ok(());
        }
    }
    let mut changes = BTreeMap::new();
    if let Some(v) = &old {
        deltas(v, -1, &mut changes)?;
    }
    if let Some(v) = value {
        v.validate(id)?;
        check_currency(conn, &v.currency).await?;
        deltas(v, 1, &mut changes)?;
    }
    let mut balances = Vec::new();
    for (id, inflow) in changes {
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT type,current_balance FROM accounts WHERE id=?")
                .bind(&id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
        let (kind, balance) = row.ok_or(BLOCKED)?;
        let next = i128::from(balance)
            + if matches!(kind.as_str(), "loan" | "credit_card") {
                -inflow
            } else {
                inflow
            };
        balances.push((id, i64::try_from(next).map_err(|_| BLOCKED)?));
    }
    if let Some(old) = old {
        // Downloaded deletes retain the normal reconciliation review requirement.
        // Native linked metadata must be changed on its originating device first.
        for e in &old.entries {
            let native:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_payment_parts WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM salary_payment_transactions WHERE transaction_id=?) OR EXISTS(SELECT 1 FROM loan_contracts WHERE borrowing_transaction_id=?) OR EXISTS(SELECT 1 FROM installments WHERE purchase_transaction_id=?)")
                .bind(&e.id).bind(&e.id).bind(&e.id).bind(&e.id).fetch_one(&mut *conn).await.map_err(|_| DATABASE)?;
            if native {
                return Err(BLOCKED.into());
            }
            crate::reconciliation::before_delete(conn, &e.id, confirmed)
                .await
                .map_err(|_| BLOCKED)?;
            sqlx::query("DELETE FROM transactions WHERE id=?")
                .bind(&e.id)
                .execute(&mut *conn)
                .await
                .map_err(|_| BLOCKED)?;
        }
    }
    if let Some(v) = value {
        for account in v.accounts() {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND conflict=1)",
            )
            .bind(format!("account:{account}"))
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?)")
                    .bind(account)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|_| DATABASE)?;
            if blocked || !exists {
                return Err(BLOCKED.into());
            }
        }
        for e in &v.entries {
            if let Some(dest) = &e.destination_account_id {
                if e.kind == "repayment" {
                    let debt: bool = sqlx::query_scalar(
                        "SELECT type IN ('loan','credit_card') FROM accounts WHERE id=?",
                    )
                    .bind(dest)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|_| BLOCKED)?;
                    if !debt {
                        return Err(BLOCKED.into());
                    }
                }
            }
            if let Some(payee) = &e.payee {
                sqlx::query(
                    "INSERT INTO payees(id,name,name_key) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING",
                )
                .bind(&payee.id)
                .bind(&payee.name)
                .bind(payee.name.to_lowercase())
                .execute(&mut *conn)
                .await
                .map_err(|_| BLOCKED)?;
            }
            let source = if let Some(source) = &e.income_source {
                sqlx::query_scalar::<_, String>("SELECT id FROM incomes WHERE id=?")
                    .bind(&source.id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|_| DATABASE)?
            } else {
                None
            };
            // Prevent a transaction ID from being moved between atomic groups.
            let owner: Option<String> = sqlx::query_scalar(
                "SELECT group_id FROM ledger_sync_members WHERE transaction_id=?",
            )
            .bind(&e.id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
            if owner.as_deref().is_some_and(|g| g != id) {
                return Err(BLOCKED.into());
            }
            sqlx::query("INSERT INTO ledger_sync_members(transaction_id,group_id) VALUES(?,?) ON CONFLICT(transaction_id) DO NOTHING").bind(&e.id).bind(id).execute(&mut *conn).await.map_err(|_| DATABASE)?;
            sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description,payee_id,category_id,income_source_id) VALUES(?,?,?,?,?,?,?,?,?,?)")
                .bind(&e.id).bind(&e.kind).bind(&e.account_id).bind(&e.destination_account_id).bind(amount(&e.amount)?).bind(&e.date).bind(&e.description).bind(e.payee.as_ref().map(|p|&p.id)).bind(&e.category_id).bind(source).execute(&mut *conn).await.map_err(|_| BLOCKED)?;
        }
        sqlx::query("INSERT INTO ledger_sync_imports(group_id,payload) VALUES(?,?) ON CONFLICT(group_id) DO UPDATE SET payload=EXCLUDED.payload").bind(id).bind(serde_json::to_string(v).map_err(|_| DATABASE)?).execute(&mut *conn).await.map_err(|_| DATABASE)?;
    }
    for (id, balance) in balances {
        sqlx::query(
            "UPDATE accounts SET current_balance=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
        )
        .bind(balance)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|_| BLOCKED)?;
    }
    if let Some(v) = value {
        crate::cloud_sync_income::install_payroll(conn, v).await?;
        crate::cloud_sync_loans::install(conn, v).await?;
    }
    crate::loans::validate_allocations(conn)
        .await
        .map_err(|_| BLOCKED)?;
    Ok(())
}
pub async fn apply(
    conn: &mut SqliteConnection,
    record: &Record,
    confirmed: bool,
) -> Result<bool, String> {
    // A failed dependency/constraint never leaves a partial group or balance delta.
    sqlx::query("SAVEPOINT apply_ledger_record")
        .execute(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
    let result = match (&*record.entity, &record.value) {
        ("metadata", Some(Value::Metadata(v))) => {
            crate::cloud_sync_metadata::apply(conn, &record.id, Some(v), confirmed).await
        }
        ("metadata", None) => {
            crate::cloud_sync_metadata::apply(conn, &record.id, None, confirmed).await
        }
        ("income-plan", Some(Value::Planner(v))) => {
            crate::cloud_sync_planner::apply(conn, &record.id, Some(v)).await
        }
        ("income-plan", None) => crate::cloud_sync_planner::apply(conn, &record.id, None).await,
        ("account", Some(Value::Account(v))) => apply_account(conn, &record.id, Some(v)).await,
        ("account", None) => apply_account(conn, &record.id, None).await,
        ("income", Some(Value::Income(v))) => {
            crate::cloud_sync_income::apply_definition(conn, &record.id, Some(v)).await
        }
        ("income", None) => {
            crate::cloud_sync_income::apply_definition(conn, &record.id, None).await
        }
        ("transaction", Some(Value::Transaction(v))) => {
            apply_group(conn, &record.id, Some(v), confirmed).await
        }
        ("transaction", None) => apply_group(conn, &record.id, None, confirmed).await,
        _ => Err(UNSAFE.into()),
    };
    if result.is_err() {
        sqlx::query("ROLLBACK TO apply_ledger_record")
            .execute(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    }
    sqlx::query("RELEASE apply_ledger_record")
        .execute(conn)
        .await
        .map_err(|_| DATABASE)?;
    match result {
        Ok(()) => Ok(true),
        Err(e) if e == BLOCKED => Ok(false),
        Err(e) => Err(e),
    }
}
pub fn summary(value: &Value) -> Category {
    match value {
        Value::Category(v) => v.clone(),
        Value::Metadata(v) => Category {
            name: crate::cloud_sync_metadata::optional(&v.fields, "name")
                .or_else(|| crate::cloud_sync_metadata::optional(&v.fields, "month"))
                .or_else(|| crate::cloud_sync_metadata::optional(&v.fields, "completed_at"))
                .unwrap_or(&v.kind)
                .into(),
            icon: "folder".into(),
            is_archived: crate::cloud_sync_metadata::optional(&v.fields, "is_archived")
                == Some("1"),
        },
        Value::Planner(v) => Category {
            name: v.month.clone(),
            icon: "calendar".into(),
            is_archived: false,
        },
        Value::Income(v) => Category {
            name: v.name.clone(),
            icon: "wallet".into(),
            is_archived: !v.is_active,
        },
        Value::Account(v) => Category {
            name: v.name.clone(),
            icon: "wallet".into(),
            is_archived: v.is_archived,
        },
        Value::Transaction(v) => Category {
            name: v
                .entries
                .first()
                .map(|e| {
                    if e.description.is_empty() {
                        e.date.clone()
                    } else {
                        e.description.clone()
                    }
                })
                .unwrap_or_else(|| {
                    v.payroll
                        .as_ref()
                        .map(|p| p.occurrence.clone())
                        .unwrap_or_default()
                }),
            icon: "receipt".into(),
            is_archived: false,
        },
    }
}
pub async fn protect_imported_deletion(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(), String> {
    // Historical alpha fixtures lack the sync tables; released beta databases migrate them.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ledger_sync_imports')",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !exists {
        return Ok(());
    }
    let payload:Option<String>=sqlx::query_scalar("SELECT i.payload FROM ledger_sync_imports i JOIN ledger_sync_members m USING(group_id) WHERE m.transaction_id=?").bind(id).fetch_optional(&mut *conn).await.map_err(|_| DATABASE)?;
    if let Some(payload) = payload {
        let group = serde_json::from_str::<TransactionGroup>(&payload).map_err(|_| DATABASE)?;
        if group.protected && group.payroll.is_none() {
            if group.links.is_none() {
                return Err(PROTECTED.into());
            }
            if let Some(p) = &group.loan {
                let found: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM loan_payment_parts WHERE payment_id=?)",
                )
                .bind(&p.id)
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
                if !found {
                    return Err(PROTECTED.into());
                }
            }
            let links = group.links.unwrap();
            for (kind, id) in [
                ("contract", links.borrowing_contract_id),
                ("installment", links.installment_id),
            ] {
                if let Some(id) = id {
                    let current =
                        crate::cloud_sync_metadata::current(conn, &format!("{kind}:{id}")).await?;
                    if let Some(Value::Metadata(value)) = &current.value {
                        let field = if kind == "contract" {
                            "borrowing_transaction_id"
                        } else {
                            "purchase_transaction_id"
                        };
                        if !crate::cloud_sync_metadata::optional(&value.fields, field)
                            .is_some_and(|t| group.entries.iter().any(|e| e.id == t))
                        {
                            return Err(PROTECTED.into());
                        }
                    } else {
                        let deleted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND json_type(payload,'$.value')='null')").bind(format!("metadata:{kind}:{id}")).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
                        if !deleted {
                            return Err(PROTECTED.into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn imported_entries(
    conn: &mut SqliteConnection,
) -> Result<BTreeMap<String, (bool, Entry)>, String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ledger_sync_imports')",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    let mut result = BTreeMap::new();
    if exists {
        let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM ledger_sync_imports")
            .fetch_all(conn)
            .await
            .map_err(|_| DATABASE)?;
        for payload in payloads {
            let group: TransactionGroup = serde_json::from_str(&payload).map_err(|_| DATABASE)?;
            for entry in group.entries {
                result.insert(
                    entry.id.clone(),
                    (
                        group.protected && group.payroll.is_none() && group.links.is_none(),
                        entry,
                    ),
                );
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud_sync_protocol::{self as protocol, Change, Head, Mutation},
        cloud_sync_store as store,
        transactions::{self, NewTransaction},
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use zeroize::Zeroizing;
    const ORIGIN: &str = "https://example.test/";
    async fn db() -> SqlitePool {
        let p = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&p).await.unwrap();
        store::enable(&p, ORIGIN, "alice", "dataset", "key")
            .await
            .unwrap();
        enable(&p).await.unwrap();
        p
    }
    async fn seed(p: &SqlitePool) {
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',9007199254740993,9007199254740993),('cash','Cash','cash',0,0),('card','Card','credit_card',20000,20000);").execute(p).await.unwrap();
    }
    fn input(kind: &str, source: &str, destination: Option<&str>, amount: &str) -> NewTransaction {
        NewTransaction {
            kind: kind.into(),
            account_id: source.into(),
            destination_account_id: destination.map(str::to_string),
            amount: amount.into(),
            date: "2026-01-01".into(),
            description: String::new(),
            currency: "THB".into(),
            payee_id: None,
            category_id: None,
            income_source_id: None,
            cleared_account_ids: vec![],
        }
    }
    async fn balance(p: &SqlitePool, id: &str) -> i64 {
        sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=?")
            .bind(id)
            .fetch_one(p)
            .await
            .unwrap()
    }
    fn accepted(m: Mutation) -> Change {
        Change {
            sequence: (m.previous.number().unwrap() + 1).to_string(),
            version: (protocol::number(&m.base_version).unwrap() + 1).to_string(),
            request: m,
        }
    }
    async fn deliver(p: &SqlitePool, changes: &[Change]) -> Result<(), String> {
        let head = store::binding(p).await?.unwrap().head();
        let mut end = head.clone();
        let mut decoded = vec![];
        for c in changes {
            end = protocol::verify_change(c, &end)?;
            decoded.push((
                c.clone(),
                Record::decode(c, &Zeroizing::new([9; 32]), "dataset", "key")?,
            ));
        }
        store::apply_page(p, &head, &decoded, &end, "test-cursor", true).await
    }
    async fn drain(p: &SqlitePool) -> Vec<Change> {
        let mut result = vec![];
        loop {
            let b = store::binding(p).await.unwrap().unwrap();
            let Some(m) = store::prepare(p, &Zeroizing::new([9; 32]), &b)
                .await
                .unwrap()
            else {
                break;
            };
            let c = accepted(m);
            deliver(p, &[c.clone()]).await.unwrap();
            result.push(c);
            assert!(result.len() < 100);
        }
        result
    }
    async fn shared() -> (SqlitePool, SqlitePool) {
        let a = db().await;
        let b = db().await;
        seed(&a).await;
        deliver(&a, &[]).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        (a, b)
    }
    #[tokio::test]
    async fn bootstrap_transfers_and_deletions_preserve_exact_balances() {
        let a = db().await;
        let b = db().await;
        seed(&a).await;
        let transfer = transactions::insert(
            &a,
            input("transfer", "bank", Some("cash"), "9007199254740993"),
        )
        .await
        .unwrap();
        transactions::insert(&a, input("repayment", "cash", Some("card"), "1000"))
            .await
            .unwrap();
        let mut expense = input("expense", "cash", None, "123");
        expense.category_id = Some("planner-expense-0".into());
        transactions::insert(&a, expense).await.unwrap();
        deliver(&a, &[]).await.unwrap();
        let all = drain(&a).await;
        assert_eq!(all.iter().filter(|c| c.request.protocol == 2).count(), 6);
        deliver(&b, &all).await.unwrap();
        for id in ["bank", "cash", "card"] {
            assert_eq!(balance(&a, id).await, balance(&b, id).await);
        }
        assert_eq!(balance(&b, "cash").await, 9007199254739870);
        assert_eq!(balance(&b, "card").await, 19000);
        assert_eq!(store::status(&b, ORIGIN, "alice").await.unwrap().pending, 0);
        // Imported ordinary transfers remain editable on either device.
        transactions::remove(&b, &transfer.id).await.unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(balance(&a, "bank").await, 9007199254740993);
        assert_eq!(balance(&a, "cash").await, -1123);
        verify_balances(&mut a.acquire().await.unwrap())
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn concurrent_distinct_transactions_merge_and_account_conflicts_do_not_overwrite_balances(
    ) {
        let (a, b) = shared().await;
        transactions::insert(&a, input("expense", "bank", None, "10"))
            .await
            .unwrap();
        transactions::insert(&b, input("income", "bank", None, "20"))
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(balance(&a, "bank").await, 9007199254741003);
        assert_eq!(balance(&a, "bank").await, balance(&b, "bank").await);
        sqlx::query("UPDATE accounts SET name='A' WHERE id='bank'")
            .execute(&a)
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET name='B' WHERE id='bank'")
            .execute(&b)
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        let status = store::status(&b, ORIGIN, "alice").await.unwrap();
        assert_eq!(status.conflict_count, 1);
        let c = &status.conflicts[0];
        assert_eq!(c.entity, "account");
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
        assert_eq!(balance(&b, "bank").await, 9007199254741003);
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT name FROM accounts WHERE id='bank'")
                .fetch_one(&b)
                .await
                .unwrap(),
            "A"
        );
    }
    #[tokio::test]
    async fn deletion_during_upload_stays_pending_without_reapplying_older_balances() {
        let (a, b) = shared().await;
        let row = transactions::insert(&a, input("transfer", "bank", Some("cash"), "100"))
            .await
            .unwrap();
        let bound = store::binding(&a).await.unwrap().unwrap();
        let sent = store::prepare(&a, &Zeroizing::new([9; 32]), &bound)
            .await
            .unwrap()
            .unwrap();
        transactions::remove(&a, &row.id).await.unwrap();
        assert_eq!(store::pending(&a).await.unwrap().unwrap(), sent);
        let change = accepted(sent);
        deliver(&a, &[change.clone()]).await.unwrap();
        deliver(&b, &[change]).await.unwrap();
        assert_eq!(balance(&a, "bank").await, 9007199254740993);
        assert_eq!(store::status(&a, ORIGIN, "alice").await.unwrap().pending, 1);
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(balance(&b, "bank").await, 9007199254740993);
    }
    #[tokio::test]
    async fn whole_group_rolls_back_on_failure_and_overflow_and_requires_currency_match() {
        let (a, b) = shared().await;
        let first = transactions::insert(&a, input("transfer", "bank", Some("cash"), "100"))
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_incoming BEFORE INSERT ON transactions WHEN NEW.destination_account_id='cash' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END").execute(&b).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(balance(&b, "bank").await, 9007199254740993);
        assert_eq!(balance(&b, "cash").await, 0);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions WHERE id=?")
                .bind(first.id)
                .fetch_one(&b)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            store::status(&b, ORIGIN, "alice")
                .await
                .unwrap()
                .conflict_count,
            1
        );
        sqlx::query("DROP TRIGGER reject_incoming")
            .execute(&b)
            .await
            .unwrap();
        deliver(&b, &[]).await.unwrap();
        assert_eq!(balance(&b, "cash").await, 100);
        let c = db().await;
        sqlx::query("UPDATE settings SET currency='USD'")
            .execute(&c)
            .await
            .unwrap();
        let value = current(&mut a.acquire().await.unwrap(), "account", "bank")
            .await
            .unwrap();
        assert!(!apply(&mut c.acquire().await.unwrap(), &value, false)
            .await
            .unwrap());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM accounts")
                .fetch_one(&c)
                .await
                .unwrap(),
            0
        );
        sqlx::query("UPDATE accounts SET current_balance=9223372036854775807 WHERE id='cash'")
            .execute(&b)
            .await
            .unwrap();
        transactions::insert(&a, input("transfer", "bank", Some("cash"), "1"))
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(balance(&b, "cash").await, i64::MAX);
        assert_eq!(balance(&b, "bank").await, 9007199254740893);
    }
    #[tokio::test]
    async fn grouped_payments_are_one_record_and_imported_deletion_is_protected() {
        let (a, b) = shared().await;
        sqlx::query("INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Loan','loan','personal_loan',1000,1000)").execute(&a).await.unwrap();
        let mut tx = a.begin().await.unwrap();
        let principal = transactions::insert_in_connection(
            &mut tx,
            input("repayment", "bank", Some("loan"), "100"),
        )
        .await
        .unwrap();
        let fee = transactions::insert_in_connection(&mut tx, input("expense", "bank", None, "20"))
            .await
            .unwrap();
        for (id, kind, amount) in [(&principal.id, "principal", 100), (&fee.id, "fee", 20)] {
            sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,loan_account_id,component,amount) VALUES(?,'payment','loan',?,?)").bind(id).bind(kind).bind(amount).execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();
        let changes = drain(&a).await;
        let groups = changes
            .iter()
            .filter(|c| {
                Record::decode(c, &Zeroizing::new([9; 32]), "dataset", "key")
                    .unwrap()
                    .entity
                    == "transaction"
            })
            .count();
        assert_eq!(groups, 1);
        deliver(&b, &changes).await.unwrap();
        let report = crate::cashflow::snapshot(&mut b.acquire().await.unwrap())
            .await
            .unwrap();
        let report = serde_json::to_value(report).unwrap();
        let charge = report["ledger_transactions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == fee.id)
            .unwrap();
        assert_eq!(charge["loan_account_id"], "loan");
        assert_eq!(charge["loan_component"], "fee");
        let mut rows: Vec<crate::transactions::Transaction> =
            sqlx::query_as(crate::transactions::SELECT)
                .fetch_all(&b)
                .await
                .unwrap();
        crate::transactions::decorate_in_connection(&mut b.acquire().await.unwrap(), &mut rows)
            .await
            .unwrap();
        let history = serde_json::to_value(rows).unwrap();
        assert!(history
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["imported_linked_payment"] == true));
        assert_eq!(balance(&b, "loan").await, 900);
        assert_eq!(balance(&b, "bank").await, 9007199254740873);
        assert_eq!(
            transactions::remove(&b, &fee.id).await.unwrap_err(),
            PROTECTED
        );
        transactions::remove(&a, &fee.id).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(balance(&b, "loan").await, 1000);
        assert_eq!(balance(&b, "bank").await, 9007199254740993);
    }
    #[tokio::test]
    async fn financial_enable_requires_new_consent_and_rejects_inconsistent_balances() {
        let p = db().await;
        sqlx::query("UPDATE cloud_sync_binding SET financial_enabled=0")
            .execute(&p)
            .await
            .unwrap();
        seed(&p).await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cloud_sync_dirty WHERE record_id LIKE 'account:%'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(count, 0);
        sqlx::query("UPDATE accounts SET current_balance=1 WHERE id='cash'")
            .execute(&p)
            .await
            .unwrap();
        assert!(enable(&p)
            .await
            .unwrap_err()
            .contains("balances do not match"));
        assert!(!store::binding(&p).await.unwrap().unwrap().financial_enabled);
        sqlx::query("UPDATE accounts SET current_balance=0 WHERE id='cash'")
            .execute(&p)
            .await
            .unwrap();
        enable(&p).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM cloud_sync_dirty WHERE record_id LIKE 'account:%'"
            )
            .fetch_one(&p)
            .await
            .unwrap(),
            3
        );
    }
    #[test]
    fn financial_ciphertext_is_namespaced_bounded_and_authenticated() {
        let root = Zeroizing::new([9; 32]);
        assert_ne!(
            protocol::wire_id(&root, "dataset", "key", "account:same").unwrap(),
            protocol::wire_id(&root, "dataset", "key", "transaction:same").unwrap()
        );
        let data = vec![b'x'; 20000];
        let m = protocol::encrypt(
            &root,
            "dataset",
            "key",
            "transaction:group",
            0,
            &Head::default(),
            false,
            &data,
        )
        .unwrap();
        assert_eq!(m.protocol, 2);
        assert_eq!(*m.decrypt(&root, "dataset", "key").unwrap(), data);
        let mut changed = m.clone();
        changed.protocol = 1;
        assert!(changed.decrypt(&root, "dataset", "key").is_err());
        assert!(protocol::encrypt(
            &root,
            "dataset",
            "key",
            "transaction:group",
            0,
            &Head::default(),
            false,
            &vec![0; 262129]
        )
        .is_err());
        assert!(amount("9007199254740993").is_ok());
        assert!(amount("01").is_err());
        assert!(amount("1.5").is_err());
    }

    #[tokio::test]
    async fn payroll_with_loan_parts_remains_one_atomic_group_and_round_trips_labels() {
        let (a, b) = shared().await;
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance) VALUES('loan','Loan','loan','personal_loan',1000,1000); INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary source','bank','salary',1000,25); INSERT INTO income_deductions(id,income_id,name,amount,debt_account_id) VALUES('deduction','salary','Loan principal',100,'loan');").execute(&a).await.unwrap();
        crate::salary_payments::record(
            &a,
            crate::salary_payments::PaymentInput {
                income_id: "salary".into(),
                destination_account_id: "bank".into(),
                occurrence: "2026-01".into(),
                date: "2026-01-25".into(),
                gross: "1000".into(),
                currency: "THB".into(),
                confirmed: true,
                deductions: vec![crate::salary_payments::Deduction {
                    id: "deduction".into(),
                    amount: "100".into(),
                    debt_account_id: Some("loan".into()),
                    interest: "0".into(),
                    fee: "0".into(),
                    contract_id: None,
                }],
            },
        )
        .await
        .unwrap();
        let changes = drain(&a).await;
        let groups = changes
            .iter()
            .map(|c| Record::decode(c, &Zeroizing::new([9; 32]), "dataset", "key").unwrap())
            .filter(|r| r.entity == "transaction")
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), 1);
        assert!(groups[0].id.starts_with("salary-"));
        let Some(Value::Transaction(group)) = &groups[0].value else {
            panic!()
        };
        assert_eq!(group.entries.len(), 2);
        assert!(group
            .entries
            .iter()
            .any(|e| e.salary_role.as_deref() == Some("principal")));
        // Fail the second insert: the first entry and both account balances roll back.
        let last = &group.entries.last().unwrap().id;
        // IDs are locally generated hex; a bind parameter is unavailable in CREATE TRIGGER.
        assert!(last.bytes().all(|c| c.is_ascii_hexdigit()));
        sqlx::query(&format!("CREATE TRIGGER reject_second BEFORE INSERT ON transactions WHEN NEW.id='{last}' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END")).execute(&b).await.unwrap();
        deliver(&b, &changes).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&b)
                .await
                .unwrap(),
            0
        );
        assert_eq!(balance(&b, "bank").await, 9007199254740993);
        assert_eq!(balance(&b, "loan").await, 1000);
        sqlx::query("DROP TRIGGER reject_second")
            .execute(&b)
            .await
            .unwrap();
        deliver(&b, &[]).await.unwrap();
        assert_eq!(balance(&b, "bank").await, 9007199254741893);
        assert_eq!(balance(&b, "loan").await, 900);
        let snapshot = current(
            &mut b.acquire().await.unwrap(),
            "transaction",
            &groups[0].id,
        )
        .await
        .unwrap();
        assert!(snapshot == groups[0]);
        // Group labels are financial backup content, while sync ownership stays local.
        let dir = tempfile::tempdir().unwrap();
        let saved: Vec<String> = sqlx::query_scalar("SELECT payload FROM ledger_sync_imports")
            .fetch_all(&b)
            .await
            .unwrap();
        sqlx::query("VACUUM INTO ?")
            .bind(dir.path().join("ledger.db").to_str().unwrap())
            .execute(&b)
            .await
            .unwrap();
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(dir.path().join("ledger.db")))
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT payload FROM ledger_sync_imports")
                .fetch_all(&reopened)
                .await
                .unwrap(),
            saved
        );
        let principal = sqlx::query_scalar::<_, String>(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='principal'",
        )
        .fetch_one(&a)
        .await
        .unwrap();
        transactions::remove(&a, &principal).await.unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(balance(&b, "bank").await, 9007199254740993);
        assert_eq!(balance(&b, "loan").await, 1000);
    }
}
