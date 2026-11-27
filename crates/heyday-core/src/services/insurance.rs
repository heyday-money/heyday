//! Insurance is informational; only explicitly confirmed premiums post money.
use crate::domain::dates::{scheduled_date, valid_date};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn money(v: &str) -> Result<i64, String> {
    let n = v.parse::<i64>().map_err(err)?;
    if n < 0 || n.to_string() != v {
        return Err("Enter a canonical nonnegative integer.".into());
    }
    Ok(n)
}
fn id(v: &str) -> Result<(), String> {
    crate::cloud_sync_ledger::id(v)
}
fn date(v: &str) -> Result<(), String> {
    if valid_date(v) {
        Ok(())
    } else {
        Err("Enter a valid date.".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Details {
    pub name: String,
    pub insurer: String,
    pub policy_type: String,
    pub number_suffix: Option<String>,
    pub coverage_start: String,
    pub coverage_end: Option<String>,
    pub renewal_date: Option<String>,
    pub coverage_amount: String,
    pub deductible: String,
    pub notes: String,
    pub status: String,
    pub status_date: String,
    pub asset_ids: Vec<String>,
    pub cash_value_id: Option<String>,
    pub replaces: Option<Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub kind: String,
    pub id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub id: String,
    pub effective_from: String,
    pub first_date: String,
    pub end_date: Option<String>,
    pub frequency: String,
    pub amount: String,
    pub account_id: String,
    pub category_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub id: String,
    pub schedule_id: String,
    pub due_date: String,
    pub date: String,
    pub amount: String,
    pub account_id: String,
    pub category_id: String,
    pub notes: String,
    pub reversal_id: Option<String>,
}
impl Payment {
    pub fn occurrence(&self) -> String {
        format!("{}:{}", self.schedule_id, self.due_date)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub id: String,
    pub revision: String,
    pub currency: String,
    pub details: Details,
    pub schedules: Vec<Schedule>,
    pub payments: Vec<Payment>,
}
pub fn scheduled(s: &Schedule, d: &str) -> bool {
    if !valid_date(d)
        || d < s.first_date.as_str()
        || d < s.effective_from.as_str()
        || s.end_date.as_deref().is_some_and(|e| d > e)
    {
        return false;
    }
    if s.frequency == "once" {
        return d == s.first_date;
    }
    let months = (d[..4].parse::<i64>().unwrap() - s.first_date[..4].parse::<i64>().unwrap()) * 12
        + d[5..7].parse::<i64>().unwrap()
        - s.first_date[5..7].parse::<i64>().unwrap();
    let step = match s.frequency.as_str() {
        "monthly" => 1,
        "quarterly" => 3,
        "yearly" => 12,
        _ => return false,
    };
    months >= 0 && months % step == 0 && scheduled_date(&s.first_date, months) == d
}
impl Policy {
    pub fn validate(&self) -> Result<(), String> {
        id(&self.id)?;
        money(&self.revision)?;
        if self.schedules.len() > 600
            || self.payments.len() > 5000
            || self.details.asset_ids.len() > 200
        {
            return Err("Insurance history exceeds the supported limit.".into());
        }
        if crate::domain::money::fraction_digits(&self.currency).is_none() {
            return Err("Choose a supported currency.".into());
        }
        let d = &self.details;
        for text in [&d.name, &d.insurer] {
            if text.trim() != text || text.is_empty() || text.chars().count() > 100 {
                return Err("Enter a policy name and insurer, up to 100 characters each.".into());
            }
        }
        if !["life", "health", "motor", "property", "travel", "other"]
            .contains(&d.policy_type.as_str())
            || !["active", "expired", "cancelled", "archived"].contains(&d.status.as_str())
        {
            return Err("Choose a supported insurance type and status.".into());
        }
        if d.notes.chars().count() > 2000
            || d.number_suffix.as_ref().is_some_and(|n| {
                n.is_empty() || n.len() > 4 || !n.chars().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err("Store only the last four letters or digits of the policy number.".into());
        }
        date(&d.coverage_start)?;
        date(&d.status_date)?;
        for v in [&d.coverage_end, &d.renewal_date].into_iter().flatten() {
            date(v)?;
            if v < &d.coverage_start {
                return Err("Coverage and renewal dates must follow the start date.".into());
            }
        }
        money(&d.coverage_amount)?;
        money(&d.deductible)?;
        let mut assets = BTreeSet::new();
        for a in &d.asset_ids {
            id(a)?;
            if !assets.insert(a) {
                return Err("Choose each insured asset once.".into());
            }
        }
        if let Some(a) = &d.cash_value_id {
            id(a)?;
        }
        if let Some(s) = &d.replaces {
            id(&s.id)?;
            if !["subscription", "payment-plan", "planner-item"].contains(&s.kind.as_str()) {
                return Err("Choose a subscription or expense plan to replace.".into());
            }
        }
        if d.replaces.is_some() && self.schedules.is_empty() {
            return Err("Add a premium schedule before replacing an existing forecast.".into());
        }
        let mut ids = BTreeSet::new();
        let mut last = String::new();
        for s in &self.schedules {
            id(&s.id)?;
            id(&s.account_id)?;
            id(&s.category_id)?;
            date(&s.first_date)?;
            date(&s.effective_from)?;
            if !ids.insert(s.id.clone())
                || s.effective_from <= last
                || money(&s.amount)? == 0
                || !["once", "monthly", "quarterly", "yearly"].contains(&s.frequency.as_str())
            {
                return Err(
                    "Premium schedules must have positive amounts and increasing effective dates."
                        .into(),
                );
            }
            if let Some(end) = &s.end_date {
                date(end)?;
                if end < &s.first_date {
                    return Err("Premium end date must follow its first date.".into());
                }
            }
            last = s.effective_from.clone();
        }
        let mut active = BTreeSet::new();
        for p in &self.payments {
            id(&p.id)?;
            date(&p.date)?;
            date(&p.due_date)?;
            let s = self
                .schedules
                .iter()
                .find(|s| s.id == p.schedule_id)
                .ok_or("Premium schedule is missing.")?;
            if !ids.insert(p.id.clone())
                || !scheduled(s, &p.due_date)
                || p.amount != s.amount
                || p.account_id != s.account_id
                || p.category_id != s.category_id
                || p.notes.chars().count() > 1000
            {
                return Err("Premium history does not match its schedule.".into());
            }
            if self.schedules.iter().any(|next| {
                next.effective_from > s.effective_from && next.effective_from <= p.due_date
            }) {
                return Err("Premium belongs to a replaced schedule.".into());
            }
            if let Some(r) = &p.reversal_id {
                id(r)?;
                if !ids.insert(r.clone()) {
                    return Err("Duplicate premium action.".into());
                }
            } else if !active.insert(p.occurrence()) {
                return Err("This premium occurrence is already paid.".into());
            }
        }
        let json = serde_json::to_string(self).map_err(err)?;
        if json.len() > 1_500_000 || serde_json::to_vec(&json).map_err(err)?.len() > 1_800_000 {
            return Err("Insurance history is too large to sync safely.".into());
        }
        Ok(())
    }
    pub fn payable(&self, s: &Schedule, d: &str) -> bool {
        scheduled(s, d)
            && d >= self.details.coverage_start.as_str()
            && self
                .details
                .coverage_end
                .as_deref()
                .is_none_or(|end| d <= end)
            && self.schedules.iter().all(|next| {
                next.effective_from <= s.effective_from || d < next.effective_from.as_str()
            })
            && (!["cancelled", "expired"].contains(&self.details.status.as_str())
                || d < self.details.status_date.as_str())
    }
}
pub async fn all(c: &mut SqliteConnection) -> Result<Vec<Policy>, String> {
    if !super::tracking::has_table(c, "insurance_policies").await? {
        return Ok(vec![]);
    }
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id,payload FROM insurance_policies ORDER BY id")
            .fetch_all(c)
            .await
            .map_err(err)?;
    rows.into_iter()
        .map(|(id, s)| {
            let p: Policy = serde_json::from_str(&s).map_err(err)?;
            if id != p.id {
                return Err("Insurance policy identity is inconsistent.".into());
            }
            Ok(p)
        })
        .collect()
}
pub async fn get(c: &mut SqliteConnection, id: &str) -> Result<Option<Policy>, String> {
    let row: Option<String> =
        sqlx::query_scalar("SELECT payload FROM insurance_policies WHERE id=?")
            .bind(id)
            .fetch_optional(c)
            .await
            .map_err(err)?;
    row.map(|s| serde_json::from_str(&s).map_err(err))
        .transpose()
}
async fn refs(
    c: &mut SqliteConnection,
    p: &Policy,
    old: Option<&Policy>,
    remote: bool,
) -> Result<(), String> {
    for a in &p.details.asset_ids {
        let found: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracking_accounts WHERE account_id=?)")
                .bind(a)
                .fetch_one(&mut *c)
                .await
                .map_err(err)?;
        if !found {
            return Err("Choose an existing Tracking asset.".into());
        }
    }
    if let Some(a) = &p.details.cash_value_id {
        let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracking_accounts WHERE account_id=? AND subtype='insurance_cash_value')").bind(a).fetch_one(&mut *c).await.map_err(err)?;
        if !found {
            return Err("Choose an insurance cash-value account.".into());
        }
    }
    for s in &p.schedules {
        let retained = remote || old.is_some_and(|o| o.schedules.contains(s));
        let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank','wallet','credit_card') AND (is_archived=0 OR ?))").bind(&s.account_id).bind(retained).fetch_one(&mut *c).await.map_err(err)?;
        if !found {
            return Err("Choose an active cash, bank, wallet or credit card account.".into());
        }
        let found: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id=? AND (is_archived=0 OR ?))",
        )
        .bind(&s.category_id)
        .bind(retained)
        .fetch_one(&mut *c)
        .await
        .map_err(err)?;
        if !found {
            return Err("Choose an active expense category.".into());
        }
    }
    if let Some(source) = &p.details.replaces {
        let query = match source.kind.as_str() {
            "subscription" => "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id=?)",
            "payment-plan" => {
                "SELECT EXISTS(SELECT 1 FROM payment_plans WHERE id=? AND type='expense')"
            }
            _ => "SELECT EXISTS(SELECT 1 FROM planner_items WHERE id=? AND category_id='expenses')",
        };
        if !sqlx::query_scalar::<_, bool>(query)
            .bind(&source.id)
            .fetch_one(&mut *c)
            .await
            .map_err(err)?
        {
            return Err("The replaced expense schedule no longer exists.".into());
        }
    }
    Ok(())
}
async fn adjust(c: &mut SqliteConnection, p: &Payment, reverse: bool) -> Result<(), String> {
    let (kind, balance): (String, i64) =
        sqlx::query_as("SELECT type,current_balance FROM accounts WHERE id=?")
            .bind(&p.account_id)
            .fetch_one(&mut *c)
            .await
            .map_err(err)?;
    let delta = i128::from(money(&p.amount)?)
        * if kind == "credit_card" { 1 } else { -1 }
        * if reverse { -1 } else { 1 };
    let next = i64::try_from(i128::from(balance) + delta)
        .map_err(|_| "Premium exceeds the supported balance range.")?;
    sqlx::query("UPDATE accounts SET current_balance=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(next)
        .bind(&p.account_id)
        .execute(c)
        .await
        .map_err(err)?;
    Ok(())
}
fn active(p: &Policy) -> BTreeMap<&str, &Payment> {
    p.payments
        .iter()
        .filter(|p| p.reversal_id.is_none())
        .map(|p| (p.id.as_str(), p))
        .collect()
}
pub async fn apply(c: &mut SqliteConnection, p: &Policy, remote: bool) -> Result<(), String> {
    p.validate()?;
    let old = get(c, &p.id).await?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *c)
        .await
        .map_err(err)?;
    if currency.as_ref().is_some_and(|v| v != &p.currency) {
        return Err("Currency changed. Reload before saving.".into());
    }
    if currency.is_none() {
        sqlx::query("UPDATE settings SET currency=? WHERE id=1")
            .bind(&p.currency)
            .execute(&mut *c)
            .await
            .map_err(err)?;
    }
    refs(c, p, old.as_ref(), remote).await?;
    if let Some(o) = &old {
        for payment in &p.payments {
            if let Some(prev) = o.payments.iter().find(|v| v.id == payment.id) {
                let mut compare = payment.clone();
                compare.reversal_id = prev.reversal_id.clone();
                if &compare != prev {
                    return Err("Recorded premiums cannot be edited.".into());
                }
            }
        }
        for s in &p.schedules {
            if let Some(prev) = o.schedules.iter().find(|v| v.id == s.id) {
                if prev != s {
                    return Err("Saved premium schedules cannot be overwritten.".into());
                }
            }
        }
    }
    let new = active(p);
    if let Some(o) = &old {
        for (tx, payment) in active(o) {
            if !new.contains_key(tx) {
                let protected:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transaction_verifications WHERE transaction_id=? AND status='reconciled') OR EXISTS(SELECT 1 FROM reconciliation_entries WHERE transaction_id=?)").bind(tx).bind(tx).fetch_one(&mut *c).await.map_err(err)?;
                if protected {
                    return Err("This premium has reconciliation history and cannot be reversed or replaced.".into());
                }
                adjust(c, payment, true).await?;
                sqlx::query("DELETE FROM transactions WHERE id=?")
                    .bind(tx)
                    .execute(&mut *c)
                    .await
                    .map_err(err)?;
                sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
                    .bind(format!("transaction:{tx}"))
                    .execute(&mut *c)
                    .await
                    .map_err(err)?;
            }
        }
    }
    sqlx::query("INSERT INTO insurance_policies(id,payload) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload").bind(&p.id).bind(serde_json::to_string(p).map_err(err)?).execute(&mut *c).await.map_err(err)?;
    for (tx, payment) in new {
        if old.as_ref().is_some_and(|o| active(o).contains_key(tx)) {
            continue;
        }
        adjust(c, payment, false).await?;
        sqlx::query(
            "INSERT INTO insurance_entries(transaction_id,policy_id,occurrence) VALUES(?,?,?)",
        )
        .bind(tx)
        .bind(&p.id)
        .bind(payment.occurrence())
        .execute(&mut *c)
        .await
        .map_err(err)?;
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description,category_id) VALUES(?,'expense',?,?,?,?,?)").bind(tx).bind(&payment.account_id).bind(money(&payment.amount)?).bind(&payment.date).bind(format!("Insurance: {}",p.details.name)).bind(&payment.category_id).execute(&mut *c).await.map_err(err)?;
        sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
            .bind(format!("transaction:{tx}"))
            .execute(&mut *c)
            .await
            .map_err(err)?;
    }
    sqlx::query("DELETE FROM insurance_links WHERE policy_id=?")
        .bind(&p.id)
        .execute(&mut *c)
        .await
        .map_err(err)?;
    let mut links: BTreeSet<(String, &str)> = p
        .details
        .asset_ids
        .iter()
        .map(|a| (a.clone(), "asset"))
        .collect();
    if let Some(a) = &p.details.cash_value_id {
        links.insert((a.clone(), "cash_value"));
    }
    for s in &p.schedules {
        links.insert((s.account_id.clone(), "payment"));
    }
    for (a, role) in links {
        sqlx::query("INSERT INTO insurance_links VALUES(?,?,?)")
            .bind(&p.id)
            .bind(a)
            .bind(role)
            .execute(&mut *c)
            .await
            .map_err(|_| "An insurance cash-value account can belong to only one policy.")?;
    }
    sqlx::query("DELETE FROM insurance_categories WHERE policy_id=?")
        .bind(&p.id)
        .execute(&mut *c)
        .await
        .map_err(err)?;
    for category in p
        .schedules
        .iter()
        .map(|s| &s.category_id)
        .collect::<BTreeSet<_>>()
    {
        sqlx::query("INSERT INTO insurance_categories VALUES(?,?)")
            .bind(&p.id)
            .bind(category)
            .execute(&mut *c)
            .await
            .map_err(err)?;
    }
    sqlx::query("DELETE FROM insurance_sources WHERE policy_id=?")
        .bind(&p.id)
        .execute(&mut *c)
        .await
        .map_err(err)?;
    if let Some(s) = &p.details.replaces {
        sqlx::query("INSERT INTO insurance_sources(policy_id,subscription_id,plan_id,item_id) VALUES(?,?,?,?)").bind(&p.id).bind((s.kind=="subscription").then_some(&s.id)).bind((s.kind=="payment-plan").then_some(&s.id)).bind((s.kind=="planner-item").then_some(&s.id)).execute(&mut *c).await.map_err(|_|"This expense schedule is already linked to another policy.")?;
    }
    Ok(())
}
async fn consent(c: &mut SqliteConnection) -> Result<(), String> {
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM cloud_sync_binding WHERE full_enabled=0)",
    )
    .fetch_one(c)
    .await
    .map_err(err)?
    {
        return Err("Enable complete financial sync before adding insurance data.".into());
    }
    Ok(())
}
pub async fn list(pool: &SqlitePool) -> Result<Vec<Policy>, String> {
    let _guard = crate::database::operations::operation()?;
    all(&mut *pool.acquire().await.map_err(err)?).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub id: String,
    pub expected_revision: Option<String>,
    pub currency: String,
    pub details: Details,
    pub schedule: Option<Schedule>,
}
pub async fn save(pool: &SqlitePool, input: Save) -> Result<Policy, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    consent(&mut tx).await?;
    let old = get(&mut tx, &input.id).await?;
    if let Some(o) = &old {
        let expected = input.expected_revision.as_deref().unwrap_or("0");
        let next = money(expected)?
            .checked_add(1)
            .ok_or("Revision limit reached.")?
            .to_string();
        if o.revision == next
            && o.currency == input.currency
            && o.details == input.details
            && input
                .schedule
                .as_ref()
                .is_none_or(|s| o.schedules.last() == Some(s))
        {
            return Ok(o.clone());
        }
    }
    if old.as_ref().map(|o| &o.revision) != input.expected_revision.as_ref() {
        return Err("Insurance policy changed. Reload before saving.".into());
    }
    let today = sqlx::query_scalar::<_, String>("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.details.status_date > today {
        return Err("Policy status cannot be changed in the future.".into());
    }
    let mut p = old.clone().unwrap_or(Policy {
        id: input.id,
        revision: "0".into(),
        currency: input.currency.clone(),
        details: input.details.clone(),
        schedules: vec![],
        payments: vec![],
    });
    if p.currency != input.currency {
        return Err("Currency changed. Reload before saving.".into());
    }
    p.details = input.details;
    if let Some(s) = input.schedule {
        if old.is_some() && s.effective_from < today {
            return Err("Schedule changes take effect today or later.".into());
        }
        if p.payments.iter().any(|v| v.due_date >= s.effective_from) {
            return Err(
                "The new schedule must start after all recorded premium occurrences.".into(),
            );
        }
        p.schedules.push(s);
    }
    p.revision = money(&p.revision)?
        .checked_add(1)
        .ok_or("Revision limit reached.")?
        .to_string();
    apply(&mut tx, &p, false).await?;
    tx.commit().await.map_err(err)?;
    Ok(p)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPayment {
    pub id: String,
    pub policy_id: String,
    pub expected_revision: String,
    pub schedule_id: String,
    pub due_date: String,
    pub date: String,
    pub notes: String,
}
pub async fn pay(pool: &SqlitePool, input: RecordPayment) -> Result<Policy, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    consent(&mut tx).await?;
    let mut p = get(&mut tx, &input.policy_id)
        .await?
        .ok_or("Insurance policy not found.")?;
    if let Some(existing) = p.payments.iter().find(|v| v.id == input.id) {
        if existing.schedule_id == input.schedule_id
            && existing.due_date == input.due_date
            && existing.date == input.date
            && existing.notes == input.notes
        {
            return Ok(p);
        }
        return Err("Premium action ID is already in use.".into());
    }
    if p.revision != input.expected_revision || p.details.status == "archived" {
        return Err("Insurance policy changed or is archived. Reload before paying.".into());
    }
    date(&input.date)?;
    if input.date
        > sqlx::query_scalar::<_, String>("SELECT date('now','localtime')")
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?
    {
        return Err("A future premium is a plan, not a payment.".into());
    }
    let s = p
        .schedules
        .iter()
        .find(|s| s.id == input.schedule_id)
        .ok_or("Premium schedule not found.")?;
    if !p.payable(s, &input.due_date) {
        return Err("This premium is outside the policy schedule.".into());
    }
    let available: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND is_archived=0)")
            .bind(&s.account_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
    if !available {
        return Err("Restore the paying account before recording a premium.".into());
    }
    p.payments.push(Payment {
        id: input.id,
        schedule_id: s.id.clone(),
        due_date: input.due_date,
        date: input.date,
        amount: s.amount.clone(),
        account_id: s.account_id.clone(),
        category_id: s.category_id.clone(),
        notes: input.notes,
        reversal_id: None,
    });
    p.revision = money(&p.revision)?
        .checked_add(1)
        .ok_or("Revision limit reached.")?
        .to_string();
    apply(&mut tx, &p, false).await?;
    tx.commit().await.map_err(err)?;
    Ok(p)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reverse {
    pub id: String,
    pub policy_id: String,
    pub payment_id: String,
    pub expected_revision: String,
}
pub async fn reverse(pool: &SqlitePool, input: Reverse) -> Result<Policy, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    consent(&mut tx).await?;
    let mut p = get(&mut tx, &input.policy_id)
        .await?
        .ok_or("Insurance policy not found.")?;
    if p.payments
        .iter()
        .any(|v| v.id == input.payment_id && v.reversal_id.as_ref() == Some(&input.id))
    {
        return Ok(p);
    }
    if p.revision != input.expected_revision || p.details.status == "archived" {
        return Err("Insurance policy changed or is archived. Reload before reversing.".into());
    }
    let payment = p
        .payments
        .iter_mut()
        .find(|v| v.id == input.payment_id && v.reversal_id.is_none())
        .ok_or("Premium is already reversed or missing.")?;
    payment.reversal_id = Some(input.id);
    p.revision = money(&p.revision)?
        .checked_add(1)
        .ok_or("Revision limit reached.")?
        .to_string();
    apply(&mut tx, &p, false).await?;
    tx.commit().await.map_err(err)?;
    Ok(p)
}
pub async fn validate_database(c: &mut SqliteConnection) -> Result<(), String> {
    for p in all(c).await? {
        p.validate()?;
        let currency: Option<String> =
            sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                .fetch_one(&mut *c)
                .await
                .map_err(err)?;
        if currency.as_deref() != Some(p.currency.as_str()) {
            return Err("Insurance currency does not match the database.".into());
        }
        refs(c, &p, Some(&p), true).await?;
        let rows:Vec<(String,String,String,String,String,String,String)>=sqlx::query_as("SELECT t.id,t.type,t.account_id,CAST(t.amount AS TEXT),t.date,t.category_id,e.occurrence FROM insurance_entries e JOIN transactions t ON t.id=e.transaction_id WHERE e.policy_id=?").bind(&p.id).fetch_all(&mut *c).await.map_err(err)?;
        let links: Vec<(String, String)> =
            sqlx::query_as("SELECT account_id,role FROM insurance_links WHERE policy_id=?")
                .bind(&p.id)
                .fetch_all(&mut *c)
                .await
                .map_err(err)?;
        let mut expected: BTreeSet<(String, String)> = p
            .details
            .asset_ids
            .iter()
            .map(|a| (a.clone(), "asset".into()))
            .collect();
        if let Some(a) = &p.details.cash_value_id {
            expected.insert((a.clone(), "cash_value".into()));
        }
        for s in &p.schedules {
            expected.insert((s.account_id.clone(), "payment".into()));
        }
        if expected != links.into_iter().collect() {
            return Err("Insurance account links are incomplete.".into());
        }
        let categories: Vec<String> =
            sqlx::query_scalar("SELECT category_id FROM insurance_categories WHERE policy_id=?")
                .bind(&p.id)
                .fetch_all(&mut *c)
                .await
                .map_err(err)?;
        if p.schedules
            .iter()
            .map(|s| s.category_id.clone())
            .collect::<BTreeSet<_>>()
            != categories.into_iter().collect()
        {
            return Err("Insurance category links are incomplete.".into());
        }
        let sources: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT subscription_id,plan_id,item_id FROM insurance_sources WHERE policy_id=?",
        )
        .bind(&p.id)
        .fetch_all(&mut *c)
        .await
        .map_err(err)?;
        let expected = p
            .details
            .replaces
            .as_ref()
            .map(|s| {
                vec![(
                    (s.kind == "subscription").then(|| s.id.clone()),
                    (s.kind == "payment-plan").then(|| s.id.clone()),
                    (s.kind == "planner-item").then(|| s.id.clone()),
                )]
            })
            .unwrap_or_default();
        if sources != expected {
            return Err("Insurance forecast links are incomplete.".into());
        }
        let active = active(&p);
        if active.len() != rows.len() {
            return Err("Insurance premium history is incomplete.".into());
        }
        for (id, kind, account, amount, date, category, occurrence) in rows {
            let payment = active
                .get(id.as_str())
                .ok_or("Unknown insurance payment.")?;
            if occurrence != payment.occurrence()
                || kind != "expense"
                || account != payment.account_id
                || amount != payment.amount
                || date != payment.date
                || category != payment.category_id
            {
                return Err("Insurance premium transaction is inconsistent.".into());
            }
        }
    }
    Ok(())
}
