//! Atomic Tracking aggregates. Cash legs, event history and projections commit together.
use crate::domain::{
    dates::valid_date,
    tracking::{calculate, TrackingAction, TrackingDraft, TrackingEffect},
};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn number(s: &str) -> Result<i64, String> {
    let n: i64 = s.parse().map_err(err)?;
    if n < 0 || n.to_string() != s {
        return Err("Enter a canonical nonnegative integer.".into());
    }
    Ok(n)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventAction {
    Value { value: String },
    Lend { principal: String },
    Receive { principal: String, interest: String },
    WriteOff { principal: String },
    Buy { cost: String },
    Sell { proceeds: String },
    Reverse { event_id: String },
}
impl EventAction {
    fn domain(&self) -> Result<TrackingAction, String> {
        serde_json::from_value(serde_json::to_value(self).map_err(err)?).map_err(err)
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Value { .. } => "value",
            Self::Lend { .. } => "lend",
            Self::Receive { .. } => "receive",
            Self::WriteOff { .. } => "write_off",
            Self::Buy { .. } => "buy",
            Self::Sell { .. } => "sell",
            Self::Reverse { .. } => "reversal",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub revision: String,
    pub date: String,
    pub notes: String,
    pub previous_value: String,
    pub next_value: String,
    pub action: EventAction,
    pub cash_account_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub id: String,
    pub draft: TrackingDraft,
    pub archived: bool,
    pub revision: String,
    pub events: Vec<Event>,
}
#[derive(Debug, Serialize)]
pub struct TrackingAccount {
    pub id: String,
    pub draft: TrackingDraft,
    pub current_value: String,
    pub revision: String,
    pub latest_valuation_date: String,
    pub archived: bool,
    pub events: Vec<Event>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub id: String,
    pub account_id: String,
    pub expected_revision: String,
    pub currency: String,
    pub date: String,
    pub notes: String,
    pub action: EventAction,
    #[serde(default)]
    pub cash_account_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Leg {
    event: String,
    role: &'static str,
    cash: String,
    delta: i64,
    date: String,
    description: String,
    asset: String,
}
struct Projection {
    value: String,
    valuation_date: String,
    active: Vec<Event>,
    legs: BTreeMap<String, Leg>,
}
fn effect(s: &Snapshot, e: &Event) -> Result<TrackingEffect, String> {
    calculate(
        s.draft.subtype,
        &e.previous_value,
        false,
        &e.action.domain()?,
    )
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        self.project().map(|_| ())
    }
    fn project(&self) -> Result<Projection, String> {
        crate::cloud_sync_ledger::id(&self.id)?;
        if self.draft.clone().validate(Some(&self.draft.currency))? != self.draft {
            return Err("Tracking metadata is not normalized.".into());
        }
        let revision = number(&self.revision)?;
        let mut value = number(&self.draft.opening_value)?.to_string();
        let mut date = self.draft.opening_date.clone();
        let mut rev = 0;
        let mut seen = BTreeSet::new();
        let mut active: Vec<Event> = vec![];
        let encoded = serde_json::to_string(self).map_err(err)?;
        if encoded.len() > 1_500_000 || serde_json::to_vec(&encoded).map_err(err)?.len() > 1_800_000
        {
            return Err("Tracking history is too large to sync safely.".into());
        }
        for e in &self.events {
            crate::cloud_sync_ledger::id(&e.id)?;
            if e.id.len() > 96
                || !seen.insert(&e.id)
                || e.notes.chars().count() > 1000
                || !valid_date(&e.date)
                || e.date < date
                || number(&e.revision)? <= rev
                || number(&e.revision)? > revision
                || e.previous_value != value
            {
                return Err("Tracking history does not match its balance or revision.".into());
            }
            number(&e.next_value)?;
            if let serde_json::Value::Object(fields) =
                serde_json::to_value(&e.action).map_err(err)?
            {
                for (key, value) in fields {
                    if key != "kind" && key != "event_id" {
                        number(value.as_str().ok_or("Invalid Tracking amount.")?)?;
                    }
                }
            }

            if let EventAction::Reverse { event_id } = &e.action {
                let target = active.pop().ok_or("There is no event to reverse.")?;
                if target.id != *event_id
                    || e.next_value != target.previous_value
                    || e.cash_account_id.is_some()
                {
                    return Err("Reverse later Tracking events first.".into());
                }
            } else {
                if active
                    .last()
                    .is_some_and(|last| matches!(last.action, EventAction::Sell { .. }))
                {
                    return Err("Reverse the disposal before adding another asset action.".into());
                }
                let change = effect(self, e)?;
                if change.next_value != e.next_value {
                    return Err("Tracking event values are inconsistent.".into());
                }
                let cash: i64 = change.cash_delta.parse().map_err(err)?;
                if (cash != 0) != e.cash_account_id.is_some() {
                    return Err("Choose a cash, bank or wallet account for this action.".into());
                }
                if let Some(id) = &e.cash_account_id {
                    crate::cloud_sync_ledger::id(id)?;
                    if id == &self.id {
                        return Err("Choose a different payment account.".into());
                    }
                }
                active.push(e.clone());
            }
            value = e.next_value.clone();
            date = e.date.clone();
            rev = number(&e.revision)?;
        }
        let valuation_date = active
            .iter()
            .rev()
            .find(|e| {
                matches!(
                    e.action,
                    EventAction::Value { .. } | EventAction::Buy { .. }
                )
            })
            .map(|e| e.date.clone())
            .unwrap_or(self.draft.opening_date.clone());
        let mut legs = BTreeMap::new();
        for e in &active {
            let change = effect(self, e)?;
            let cash: i64 = change.cash_delta.parse().map_err(err)?;
            let income: i64 = change.income.parse().map_err(err)?;
            if let Some(bank) = &e.cash_account_id {
                for (role, delta) in [("principal", cash - income), ("interest", income)] {
                    if delta != 0 {
                        legs.insert(
                            format!("{}-{}", e.id, role),
                            Leg {
                                event: e.id.clone(),
                                role,
                                cash: bank.clone(),
                                delta,
                                date: e.date.clone(),
                                description: format!("Tracking: {}", self.draft.name)
                                    .chars()
                                    .take(200)
                                    .collect(),
                                asset: self.id.clone(),
                            },
                        );
                    }
                }
            }
        }
        Ok(Projection {
            value,
            valuation_date,
            active,
            legs,
        })
    }
}
async fn currency(c: &mut SqliteConnection) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(c)
        .await
        .map_err(err)
}
async fn check_date(c: &mut SqliteConnection, date: &str) -> Result<(), String> {
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(c)
        .await
        .map_err(err)?;
    if !valid_date(date) || date > today.as_str() {
        return Err("Enter a valid date that is not in the future.".into());
    }
    Ok(())
}
async fn consent(c: &mut SqliteConnection) -> Result<(), String> {
    let incomplete: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_binding WHERE full_enabled=0)")
            .fetch_one(c)
            .await
            .map_err(err)?;
    if incomplete {
        return Err("Enable complete financial sync before adding Tracking data to this connected database.".into());
    }
    Ok(())
}
pub async fn snapshot(c: &mut SqliteConnection, id: &str) -> Result<Option<Snapshot>, String> {
    let row=sqlx::query("SELECT a.*,t.subtype,t.opening_date,t.debtor,t.due_date,t.linked_loan_id,t.logo_asset_id,t.revision FROM accounts a JOIN tracking_accounts t ON t.account_id=a.id WHERE a.id=? AND a.type='tracking'").bind(id).fetch_optional(&mut *c).await.map_err(err)?;
    let Some(r) = row else { return Ok(None) };
    let mut events = vec![];
    for e in sqlx::query("SELECT * FROM tracking_events WHERE account_id=? ORDER BY revision")
        .persistent(false)
        .bind(id)
        .fetch_all(&mut *c)
        .await
        .map_err(err)?
    {
        events.push(Event {
            id: e.try_get("id").map_err(err)?,
            revision: e.try_get::<i64, _>("revision").map_err(err)?.to_string(),
            date: e.try_get("date").map_err(err)?,
            notes: e.try_get("notes").map_err(err)?,
            previous_value: e
                .try_get::<i64, _>("previous_value")
                .map_err(err)?
                .to_string(),
            next_value: e.try_get::<i64, _>("next_value").map_err(err)?.to_string(),
            action: if let Ok(json) = e.try_get::<String, _>("action_json") {
                serde_json::from_str(&json).map_err(err)?
            } else if e.try_get::<String, _>("kind").map_err(err)? == "value" {
                EventAction::Value {
                    value: e.try_get::<i64, _>("next_value").map_err(err)?.to_string(),
                }
            } else {
                EventAction::Reverse {
                    event_id: e.try_get("reverses_id").map_err(err)?,
                }
            },
            cash_account_id: e.try_get("cash_account_id").unwrap_or(None),
        });
        let event = events.last().unwrap();
        let reversal = match &event.action {
            EventAction::Reverse { event_id } => Some(event_id.as_str()),
            _ => None,
        };
        if e.try_get::<String, _>("kind").map_err(err)? != event.action.kind()
            || e.try_get::<Option<String>, _>("reverses_id")
                .map_err(err)?
                .as_deref()
                != reversal
        {
            return Err("Tracking event action does not match its stored history.".into());
        }
    }
    let s = Snapshot {
        id: id.into(),
        archived: r.try_get("is_archived").map_err(err)?,
        revision: r.try_get::<i64, _>("revision").map_err(err)?.to_string(),
        draft: TrackingDraft {
            name: r.try_get("name").map_err(err)?,
            subtype: serde_json::from_value(serde_json::Value::String(
                r.try_get("subtype").map_err(err)?,
            ))
            .map_err(err)?,
            currency: currency(c).await?.ok_or("Choose the app currency first.")?,
            opening_value: r
                .try_get::<i64, _>("opening_balance")
                .map_err(err)?
                .to_string(),
            opening_date: r.try_get("opening_date").map_err(err)?,
            notes: r.try_get("notes").map_err(err)?,
            logo_asset_id: r.try_get("logo_asset_id").map_err(err)?,
            debtor: r.try_get("debtor").map_err(err)?,
            due_date: r.try_get("due_date").map_err(err)?,
            linked_loan_id: r.try_get("linked_loan_id").map_err(err)?,
        },
        events,
    };
    if s.project()?.value
        != r.try_get::<i64, _>("current_balance")
            .map_err(err)?
            .to_string()
    {
        return Err("Tracking history does not match its balance.".into());
    }
    Ok(Some(s))
}
fn state(s: Snapshot) -> Result<TrackingAccount, String> {
    let p = s.project()?;
    Ok(TrackingAccount {
        id: s.id,
        draft: s.draft,
        revision: s.revision,
        archived: s.archived,
        current_value: p.value,
        latest_valuation_date: p.valuation_date,
        events: s.events,
    })
}
async fn refs(
    c: &mut SqliteConnection,
    s: &Snapshot,
    old: Option<&Snapshot>,
    remote: bool,
) -> Result<(), String> {
    if let Some(id) = &s.draft.linked_loan_id {
        let retain = remote || old.is_some_and(|o| o.draft.linked_loan_id.as_ref() == Some(id));
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='loan' AND (is_archived=0 OR ?))").bind(id).bind(retain).fetch_one(&mut *c).await.map_err(err)?;
        if !valid {
            return Err("Choose an active loan account.".into());
        }
    }
    if let Some(id) = &s.draft.logo_asset_id {
        if !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM logo_assets WHERE id=?)")
            .bind(id)
            .fetch_one(&mut *c)
            .await
            .map_err(err)?
        {
            return Err("The selected logo no longer exists.".into());
        }
    }
    for e in &s.events {
        if let Some(id) = &e.cash_account_id {
            let retain = remote || old.is_some_and(|o| o.events.iter().any(|x| x.id == e.id));
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('cash','bank','wallet') AND (is_archived=0 OR ?))").bind(id).bind(retain).fetch_one(&mut *c).await.map_err(err)?;
            if !valid {
                return Err("Choose an active cash, bank or wallet account.".into());
            }
        }
    }
    Ok(())
}
async fn adjust_cash(c: &mut SqliteConnection, leg: &Leg, invert: bool) -> Result<(), String> {
    crate::transactions::adjust_wide(
        c,
        &leg.cash,
        if invert {
            -i128::from(leg.delta)
        } else {
            i128::from(leg.delta)
        },
        false,
    )
    .await?;
    Ok(())
}
async fn remove_leg(c: &mut SqliteConnection, id: &str, leg: &Leg) -> Result<(), String> {
    let reconciled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transaction_verifications WHERE transaction_id=? AND status='reconciled') OR EXISTS(SELECT 1 FROM reconciliation_entries WHERE transaction_id=?)").bind(id).bind(id).fetch_one(&mut *c).await.map_err(err)?;
    if reconciled {
        return Err(
            "This Tracking action has reconciliation history and cannot be reversed or replaced."
                .into(),
        );
    }
    adjust_cash(c, leg, true).await?;
    sqlx::query("DELETE FROM transactions WHERE id=?")
        .bind(id)
        .execute(&mut *c)
        .await
        .map_err(err)?;
    sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
        .bind(format!("transaction:{id}"))
        .execute(c)
        .await
        .map_err(err)?;
    Ok(())
}
async fn insert_leg(c: &mut SqliteConnection, id: &str, leg: &Leg) -> Result<(), String> {
    if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM transactions WHERE id=?)")
        .bind(id)
        .fetch_one(&mut *c)
        .await
        .map_err(err)?
    {
        return Err("Tracking transaction ID is already in use.".into());
    }
    adjust_cash(c, leg, false).await?;
    sqlx::query("INSERT INTO tracking_entries(transaction_id,event_id,role) VALUES(?,?,?)")
        .bind(id)
        .bind(&leg.event)
        .bind(leg.role)
        .execute(&mut *c)
        .await
        .map_err(err)?;
    let (kind, from, to) = if leg.role == "interest" {
        ("income", leg.cash.as_str(), None)
    } else if leg.delta < 0 {
        ("transfer", leg.cash.as_str(), Some(leg.asset.as_str()))
    } else {
        ("transfer", leg.asset.as_str(), Some(leg.cash.as_str()))
    };
    sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES(?,?,?,?,?,?,?)").bind(id).bind(kind).bind(from).bind(to).bind(leg.delta.unsigned_abs() as i64).bind(&leg.date).bind(&leg.description).execute(&mut *c).await.map_err(err)?;
    sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
        .bind(format!("transaction:{id}"))
        .execute(c)
        .await
        .map_err(err)?;
    Ok(())
}
/// Caller owns a transaction/savepoint and the database-operation guard.
pub async fn apply(c: &mut SqliteConnection, s: &Snapshot, remote: bool) -> Result<(), String> {
    let p = s.project()?;
    let old = snapshot(c, &s.id).await?;
    if let Some(o) = &old {
        s.draft
            .clone()
            .validate_edit(&o.draft, Some(&o.draft.currency))?;
        for e in &s.events {
            if let Some(before) = o.events.iter().find(|x| x.id == e.id) {
                if before != e {
                    return Err("An immutable Tracking event was changed.".into());
                }
            }
        }
    } else if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?)")
        .bind(&s.id)
        .fetch_one(&mut *c)
        .await
        .map_err(err)?
    {
        return Err("Account identity is already in use.".into());
    }
    let configured = currency(c).await?;
    if configured.as_deref().is_some_and(|v| v != s.draft.currency) {
        return Err("Currency changed. Reload before saving.".into());
    }
    if configured.is_none() {
        sqlx::query("UPDATE settings SET currency=? WHERE id=1")
            .bind(&s.draft.currency)
            .execute(&mut *c)
            .await
            .map_err(err)?;
    }
    refs(c, s, old.as_ref(), remote).await?;
    let oldlegs = old
        .as_ref()
        .map(|o| o.project().map(|p| p.legs))
        .transpose()?
        .unwrap_or_default();
    // Names are metadata: renaming never rewrites or invalidates reconciled cash legs.
    let same = |a: &Leg, b: &Leg| {
        a.event == b.event
            && a.role == b.role
            && a.cash == b.cash
            && a.delta == b.delta
            && a.date == b.date
            && a.asset == b.asset
    };
    for (id, leg) in &oldlegs {
        if !p.legs.get(id).is_some_and(|n| same(n, leg)) {
            remove_leg(c, id, leg).await?;
        }
    }
    if old.is_none() {
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance,notes,is_archived) VALUES(?,?,'tracking',?,?,?,?)").bind(&s.id).bind(&s.draft.name).bind(number(&s.draft.opening_value)?).bind(number(&s.draft.opening_value)?).bind(&s.draft.notes).bind(s.archived).execute(&mut *c).await.map_err(err)?;
        let kind = serde_json::to_value(s.draft.subtype).map_err(err)?;
        sqlx::query("INSERT INTO tracking_accounts(account_id,subtype,opening_date) VALUES(?,?,?)")
            .bind(&s.id)
            .bind(kind.as_str().unwrap())
            .bind(&s.draft.opening_date)
            .execute(&mut *c)
            .await
            .map_err(err)?;
    }
    if let Some(o) = &old {
        for e in o.events.iter().rev() {
            if !s.events.iter().any(|n| n.id == e.id) {
                sqlx::query("DELETE FROM tracking_events WHERE id=?")
                    .bind(&e.id)
                    .execute(&mut *c)
                    .await
                    .map_err(err)?;
            }
        }
    }
    for e in &s.events {
        if !old
            .as_ref()
            .is_some_and(|o| o.events.iter().any(|x| x.id == e.id))
        {
            let reversal = if let EventAction::Reverse { event_id } = &e.action {
                Some(event_id)
            } else {
                None
            };
            sqlx::query("INSERT INTO tracking_events(id,account_id,revision,date,kind,previous_value,next_value,notes,reverses_id,action_json,cash_account_id) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(&e.id).bind(&s.id).bind(number(&e.revision)?).bind(&e.date).bind(e.action.kind()).bind(number(&e.previous_value)?).bind(number(&e.next_value)?).bind(&e.notes).bind(reversal).bind(serde_json::to_string(&e.action).map_err(err)?).bind(&e.cash_account_id).execute(&mut *c).await.map_err(err)?;
        }
    }
    for (id, leg) in &p.legs {
        if !oldlegs.get(id).is_some_and(|o| same(o, leg)) {
            insert_leg(c, id, leg).await?;
        }
    }
    sqlx::query("UPDATE accounts SET name=?,notes=?,is_archived=?,current_balance=?,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(&s.draft.name).bind(&s.draft.notes).bind(s.archived).bind(number(&p.value)?).bind(&s.id).execute(&mut *c).await.map_err(err)?;
    sqlx::query("UPDATE tracking_accounts SET debtor=?,due_date=?,linked_loan_id=?,logo_asset_id=?,revision=? WHERE account_id=?").bind(&s.draft.debtor).bind(&s.draft.due_date).bind(&s.draft.linked_loan_id).bind(&s.draft.logo_asset_id).bind(number(&s.revision)?).bind(&s.id).execute(&mut *c).await.map_err(err)?;
    Ok(())
}
pub async fn get(pool: &SqlitePool, id: &str) -> Result<TrackingAccount, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    let s = snapshot(&mut tx, id)
        .await?
        .ok_or("Tracking account not found.")?;
    let result = state(s)?;
    tx.commit().await.map_err(err)?;
    Ok(result)
}
pub async fn create(pool: &SqlitePool, input: TrackingDraft) -> Result<TrackingAccount, String> {
    create_with_logo(pool, input, None).await
}
pub async fn create_with_logo(
    pool: &SqlitePool,
    input: TrackingDraft,
    logo: Option<crate::logos::LogoChange>,
) -> Result<TrackingAccount, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    consent(&mut tx).await?;
    let mut draft = input.validate(currency(&mut tx).await?.as_deref())?;
    check_date(&mut tx, &draft.opening_date).await?;
    if let Some(l) = logo {
        draft.logo_asset_id = crate::logos::asset(&mut tx, &l).await?;
    }
    let id: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    let s = Snapshot {
        id,
        draft,
        archived: false,
        revision: "0".into(),
        events: vec![],
    };
    apply(&mut tx, &s, false).await?;
    tx.commit().await.map_err(err)?;
    state(s)
}
pub async fn edit(
    pool: &SqlitePool,
    id: &str,
    expected: &str,
    input: TrackingDraft,
) -> Result<TrackingAccount, String> {
    edit_with_logo(pool, id, expected, input, None).await
}
pub async fn edit_with_logo(
    pool: &SqlitePool,
    id: &str,
    expected: &str,
    input: TrackingDraft,
    logo: Option<crate::logos::LogoChange>,
) -> Result<TrackingAccount, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    let mut s = snapshot(&mut tx, id)
        .await?
        .ok_or("Tracking account not found.")?;
    if s.archived || s.revision != expected {
        return Err("Tracking account changed or is archived. Reload before saving.".into());
    }
    s.draft = input.validate_edit(&s.draft, currency(&mut tx).await?.as_deref())?;
    if let Some(l) = logo {
        s.draft.logo_asset_id = crate::logos::asset(&mut tx, &l).await?;
    }
    s.revision = number(expected)?
        .checked_add(1)
        .ok_or("Revision overflow.")?
        .to_string();
    apply(&mut tx, &s, false).await?;
    crate::logos::prune(&mut tx).await?;
    tx.commit().await.map_err(err)?;
    state(s)
}
pub async fn record(pool: &SqlitePool, input: EventInput) -> Result<TrackingAccount, String> {
    let _guard = crate::database::operations::operation()?;
    let mut tx = pool.begin().await.map_err(err)?;
    let mut s = snapshot(&mut tx, &input.account_id)
        .await?
        .ok_or("Tracking account not found.")?;
    if input.currency != s.draft.currency {
        return Err("Currency changed. Reload before saving.".into());
    }
    check_date(&mut tx, &input.date).await?;
    let next_revision = number(&input.expected_revision)?
        .checked_add(1)
        .ok_or("Revision overflow.")?
        .to_string();
    if let Some(e) = s.events.iter().find(|e| e.id == input.id) {
        if e.revision != next_revision
            || e.date != input.date
            || e.notes != input.notes
            || e.action != input.action
            || e.cash_account_id != input.cash_account_id
        {
            return Err("This event ID was already used for a different change.".into());
        }
        return state(s);
    }
    if s.archived || s.revision != input.expected_revision {
        return Err("Tracking account changed or is archived. Reload before saving.".into());
    }
    let p = s.project()?;
    let next = if let EventAction::Reverse { event_id } = &input.action {
        let target = p.active.last().ok_or("There is no event to reverse.")?;
        if &target.id != event_id {
            return Err("Reverse later Tracking events first.".into());
        }
        target.previous_value.clone()
    } else {
        calculate(s.draft.subtype, &p.value, false, &input.action.domain()?)?.next_value
    };
    s.events.push(Event {
        id: input.id,
        revision: next_revision.clone(),
        date: input.date,
        notes: input.notes,
        previous_value: p.value,
        next_value: next,
        action: input.action,
        cash_account_id: input.cash_account_id,
    });
    s.revision = next_revision;
    apply(&mut tx, &s, false).await?;
    tx.commit().await.map_err(err)?;
    state(s)
}
pub async fn validate_database(c: &mut SqliteConnection) -> Result<(), String> {
    let orphan:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts a LEFT JOIN tracking_accounts t ON t.account_id=a.id WHERE a.type='tracking' AND t.account_id IS NULL) OR EXISTS(SELECT 1 FROM tracking_accounts t LEFT JOIN accounts a ON a.id=t.account_id WHERE a.id IS NULL OR a.type<>'tracking')").fetch_one(&mut *c).await.map_err(err)?;
    if orphan {
        return Err("Tracking metadata is missing or has an invalid account reference.".into());
    }
    let ids: Vec<String> = sqlx::query_scalar("SELECT account_id FROM tracking_accounts")
        .fetch_all(&mut *c)
        .await
        .map_err(err)?;
    for id in ids {
        let s = snapshot(c, &id)
            .await?
            .ok_or("Tracking account not found.")?;
        refs(c, &s, Some(&s), true).await?;
        let has_entries:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='tracking_entries')").fetch_one(&mut *c).await.map_err(err)?;
        if has_entries {
            let expected = s.project()?.legs;
            let rows=sqlx::query("SELECT t.*,e.role,e.event_id FROM tracking_entries e JOIN tracking_events h ON h.id=e.event_id JOIN transactions t ON t.id=e.transaction_id WHERE h.account_id=?").bind(&id).fetch_all(&mut *c).await.map_err(err)?;
            if rows.len() != expected.len() {
                return Err("Tracking cash history is incomplete.".into());
            }
            for row in rows {
                let id: String = row.try_get("id").map_err(err)?;
                let leg = expected.get(&id).ok_or("Unexpected Tracking cash entry.")?;
                let (kind, source, destination) = if leg.role == "interest" {
                    ("income", leg.cash.as_str(), None)
                } else if leg.delta < 0 {
                    ("transfer", leg.cash.as_str(), Some(leg.asset.as_str()))
                } else {
                    ("transfer", leg.asset.as_str(), Some(leg.cash.as_str()))
                };
                if row.try_get::<String, _>("type").map_err(err)? != kind
                    || row.try_get::<String, _>("account_id").map_err(err)? != source
                    || row
                        .try_get::<Option<String>, _>("destination_account_id")
                        .map_err(err)?
                        .as_deref()
                        != destination
                    || row.try_get::<i64, _>("amount").map_err(err)?.unsigned_abs()
                        != leg.delta.unsigned_abs()
                    || row.try_get::<String, _>("date").map_err(err)? != leg.date
                    || row.try_get::<String, _>("event_id").map_err(err)? != leg.event
                    || row.try_get::<String, _>("role").map_err(err)? != leg.role
                {
                    return Err("Tracking cash history does not match its events.".into());
                }
            }
        }
    }
    Ok(())
}

// Historical migration fixtures and pre-upgrade backup validation may omit these tables.
pub(crate) async fn has_table(c: &mut SqliteConnection, name: &str) -> Result<bool, String> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)")
        .bind(name)
        .fetch_one(c)
        .await
        .map_err(err)
}
