use crate::{
    cloud_crypto::Secret,
    cloud_sync_protocol::{self as protocol, Change, Head, Mutation, UNSAFE},
};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

pub const DATABASE: &str = "Could not update local sync state. Please try again.";
pub const ACCOUNT: &str = "This local database is linked to another cloud account. Sign in to its original account to sync.";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Category {
    pub name: String,
    pub icon: String,
    pub is_archived: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: u8,
    pub entity: String,
    pub id: String,
    pub value: Option<Value>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Category(Category),
    Metadata(crate::cloud_sync_metadata::Metadata),
    Planner(crate::cloud_sync_planner::Override),
    Income(crate::cloud_sync_income::Definition),
    Account(crate::cloud_sync_ledger::Account),
    Transaction(crate::cloud_sync_ledger::TransactionGroup),
}
impl Value {
    fn category_mut(&mut self) -> Result<&mut Category, String> {
        match self {
            Self::Category(v) => Ok(v),
            _ => Err("Only categories can be renamed here.".into()),
        }
    }
}
impl Record {
    pub fn validate(&self) -> Result<(), String> {
        if self.entity == "metadata" {
            crate::cloud_sync_metadata::parse(&self.id)?;
        } else if self.entity == "income-plan" {
            crate::cloud_sync_planner::parts(&self.id)?;
        } else {
            crate::cloud_sync_ledger::id(&self.id)?;
        }
        match (self.schema, self.entity.as_str(), &self.value) {
            (1, "category", Some(Value::Category(value))) => {
                let normalized = value.name.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized.is_empty()
                    || normalized != value.name
                    || value.name.chars().count() > 100
                    || !crate::transaction_options::CATEGORY_ICONS.contains(&value.icon.as_str())
                {
                    return Err(UNSAFE.into());
                }
            }
            (1, "category", None)
            | (2, "account" | "transaction", None)
            | (3, "income" | "transaction", None)
            | (4, "income-plan", None)
            | (5, "metadata" | "transaction", None) => {}
            (5, "metadata", Some(Value::Metadata(value))) => value.validate(&self.id)?,
            (5, "transaction", Some(Value::Transaction(value))) if value.links.is_some() => {
                value.validate(&self.id)?
            }
            (4, "income-plan", Some(Value::Planner(value))) => value.validate(&self.id)?,
            (2, "account", Some(Value::Account(value))) => value.validate()?,
            (2, "transaction", Some(Value::Transaction(value)))
                if value.payroll.is_none() && value.loan.is_none() && value.links.is_none() =>
            {
                value.validate(&self.id)?
            }
            (3, "transaction", Some(Value::Transaction(value)))
                if value.payroll.is_some() && value.loan.is_none() && value.links.is_none() =>
            {
                value.validate(&self.id)?
            }
            (3, "income", Some(Value::Income(value))) => value.validate()?,
            _ => return Err("This cloud data needs a newer app. Update before syncing.".into()),
        }
        Ok(())
    }
    pub fn key(&self) -> String {
        if self.entity == "category" {
            self.id.clone()
        } else {
            format!("{}:{}", self.entity, self.id)
        }
    }
    fn equivalent(&self, other: &Self) -> bool {
        if self.id != other.id || self.entity != other.entity {
            return false;
        }
        match (&self.value, &other.value) {
            (None, None) => true,
            (Some(Value::Transaction(a)), Some(Value::Transaction(b))) => {
                crate::cloud_sync_ledger::same(a, b)
            }
            _ => self == other,
        }
    }
    #[cfg(test)]
    pub fn decode(
        change: &Change,
        root: &Secret,
        dataset: &str,
        key_id: &str,
    ) -> Result<Self, String> {
        Self::decode_epoch(change, root, dataset, key_id, root, key_id)
    }
    pub fn decode_epoch(
        change: &Change,
        root: &Secret,
        dataset: &str,
        key_id: &str,
        identity: &Secret,
        identity_id: &str,
    ) -> Result<Self, String> {
        let bytes = change.request.decrypt(root, dataset, key_id)?;
        let record: Self = serde_json::from_slice(&bytes).map_err(|_| UNSAFE)?;
        record.validate()?;
        if protocol::wire_id(identity, dataset, identity_id, &record.key())?
            != change.request.record_id
            || record.value.is_none() != (change.request.operation == "delete")
            || change.request.protocol != record.schema
        {
            return Err(UNSAFE.into());
        }
        Ok(record)
    }
}
#[derive(sqlx::FromRow)]
pub struct Binding {
    pub origin: String,
    pub account_id: String,
    pub dataset_id: String,
    pub key_id: String,
    pub enabled: bool,
    pub bootstrapped: bool,
    pub financial_enabled: bool,
    pub income_enabled: bool,
    pub planner_enabled: bool,
    pub full_enabled: bool,
    pub sequence: i64,
    pub hash: String,
    pub cursor: Option<String>,
    pub last_success: Option<String>,
}
impl Binding {
    pub fn head(&self) -> Head {
        Head {
            sequence: self.sequence.to_string(),
            hash: self.hash.clone(),
        }
    }
    pub fn owner(&self, origin: &str, account: &str) -> Result<(), String> {
        if self.origin != origin || self.account_id != account {
            Err(ACCOUNT.into())
        } else {
            Ok(())
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub bound: bool,
    pub enabled: bool,
    pub bootstrapped: bool,
    pub financial_enabled: bool,
    pub income_enabled: bool,
    pub planner_enabled: bool,
    pub full_enabled: bool,
    pub pending: i64,
    pub conflict_count: i64,
    pub last_success: Option<String>,
    pub conflicts: Vec<Conflict>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub record_id: String,
    pub entity: String,
    pub local_details: Option<Value>,
    pub remote_details: Option<Value>,
    pub local: Option<Category>,
    pub remote: Option<Category>,
    #[serde(serialize_with = "decimal")]
    pub local_revision: i64,
    #[serde(serialize_with = "decimal")]
    pub remote_version: i64,
}
fn decimal<S: serde::Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}
pub async fn binding(pool: &SqlitePool) -> Result<Option<Binding>, String> {
    sqlx::query_as("SELECT origin,account_id,dataset_id,key_id,enabled,bootstrapped,financial_enabled,income_enabled,planner_enabled,full_enabled,sequence,hash,cursor,last_success FROM cloud_sync_binding WHERE id=1").fetch_optional(pool).await.map_err(|_| DATABASE.into())
}
pub async fn current(conn: &mut SqliteConnection, id: &str) -> Result<Record, String> {
    if let Some((entity, local_id)) = id.split_once(':') {
        if entity == "metadata" {
            return crate::cloud_sync_metadata::current(conn, local_id).await;
        }
        if entity == "income-plan" {
            return crate::cloud_sync_planner::current(conn, local_id).await;
        }
        if entity == "income" {
            return crate::cloud_sync_income::current(conn, local_id).await;
        }
        return crate::cloud_sync_ledger::current(conn, entity, local_id).await;
    }
    let value: Option<(String, String, bool)> =
        sqlx::query_as("SELECT name,icon,is_archived FROM categories WHERE id=?")
            .bind(id)
            .fetch_optional(conn)
            .await
            .map_err(|_| DATABASE)?;
    Ok(Record {
        schema: 1,
        entity: "category".into(),
        id: id.into(),
        value: value.map(|(name, icon, is_archived)| {
            Value::Category(Category {
                name,
                icon,
                is_archived,
            })
        }),
    })
}
pub async fn status(pool: &SqlitePool, origin: &str, account: &str) -> Result<Status, String> {
    let Some(bound) = binding(pool).await? else {
        return Ok(Status {
            bound: false,
            enabled: false,
            bootstrapped: false,
            financial_enabled: false,
            income_enabled: false,
            planner_enabled: false,
            full_enabled: false,
            pending: 0,
            conflict_count: 0,
            last_success: None,
            conflicts: vec![],
        });
    };
    bound.owner(origin, account)?;
    let mut conn = pool.acquire().await.map_err(|_| DATABASE)?;
    let pending = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_dirty")
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
    let conflict_count =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_shadow WHERE conflict=1")
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    let rows:Vec<(String,i64,String,i64)>=sqlx::query_as("SELECT s.record_id,s.version,s.payload,COALESCE(d.revision,0) FROM cloud_sync_shadow s LEFT JOIN cloud_sync_dirty d USING(record_id) WHERE s.conflict=1 ORDER BY s.record_id LIMIT 50").fetch_all(&mut *conn).await.map_err(|_| DATABASE)?;
    let mut conflicts = Vec::new();
    for (record_id, remote_version, payload, local_revision) in rows {
        let remote: Record = serde_json::from_str(&payload).map_err(|_| DATABASE)?;
        let local = current(&mut conn, &record_id).await?;
        conflicts.push(Conflict {
            entity: remote.entity.clone(),
            local: local.value.as_ref().map(crate::cloud_sync_ledger::summary),
            remote: remote.value.as_ref().map(crate::cloud_sync_ledger::summary),
            local_details: local.value,
            remote_details: remote.value,
            record_id,
            local_revision,
            remote_version,
        });
    }
    Ok(Status {
        bound: true,
        enabled: bound.enabled,
        bootstrapped: bound.bootstrapped,
        financial_enabled: bound.financial_enabled,
        income_enabled: bound.income_enabled,
        planner_enabled: bound.planner_enabled,
        full_enabled: bound.full_enabled,
        pending,
        conflict_count,
        last_success: bound.last_success,
        conflicts,
    })
}
pub async fn enable(
    pool: &SqlitePool,
    origin: &str,
    account: &str,
    dataset: &str,
    key_id: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let old: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT origin,account_id,dataset_id,key_id FROM cloud_sync_binding WHERE id=1",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| DATABASE)?;
    if let Some((o, a, d, k)) = old {
        if o != origin || a != account {
            return Err(ACCOUNT.into());
        }
        if d != dataset || k != key_id {
            return Err(UNSAFE.into());
        }
        sqlx::query("UPDATE cloud_sync_binding SET enabled=1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    } else {
        sqlx::query("INSERT INTO cloud_sync_binding(id,origin,account_id,dataset_id,key_id,hash) VALUES(1,?,?,?,?,?)").bind(origin).bind(account).bind(dataset).bind(key_id).bind(protocol::ZERO).execute(&mut *tx).await.map_err(|_| DATABASE)?;
        sqlx::query("INSERT INTO cloud_sync_dirty(record_id) SELECT id FROM categories")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    }
    tx.commit().await.map_err(|_| DATABASE.into())
}
pub async fn pause(pool: &SqlitePool, origin: &str, account: &str) -> Result<(), String> {
    if let Some(bound) = binding(pool).await? {
        bound.owner(origin, account)?;
    }
    sqlx::query("UPDATE cloud_sync_binding SET enabled=0 WHERE id=1")
        .execute(pool)
        .await
        .map_err(|_| DATABASE)?;
    Ok(())
}
fn unchanged_seed(record: &Record) -> bool {
    if record.entity == "metadata" {
        return crate::cloud_sync_metadata_rules::unchanged_seed(record);
    }
    if record.entity != "category" {
        return false;
    }
    let names = [
        "Water",
        "Electricity",
        "Telephone",
        "Internet",
        "Food / household supplies",
        "Transport / fuel",
        "Insurance",
        "Family / dependents",
        "Memberships / subscriptions",
        "Medical / healthcare",
    ];
    let expected = if record.id == "default-fee-interest" {
        Some("Fee/Interest")
    } else {
        record
            .id
            .strip_prefix("planner-expense-")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| names.get(n).copied())
    };
    record
        .value
        .as_ref()
        .is_some_and(|v| matches!(v,Value::Category(v) if Some(v.name.as_str()) == expected && v.icon == "tag" && !v.is_archived))
}
async fn apply(conn: &mut SqliteConnection, record: &Record) -> Result<bool, String> {
    if record.entity != "category" {
        return crate::cloud_sync_ledger::apply(conn, record, false).await;
    }
    let result = if let Some(Value::Category(value)) = &record.value {
        sqlx::query("INSERT INTO categories(id,name,name_key,icon,is_archived) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name,name_key=EXCLUDED.name_key,icon=EXCLUDED.icon,is_archived=EXCLUDED.is_archived")
            .bind(&record.id).bind(&value.name).bind(value.name.to_lowercase()).bind(&value.icon).bind(value.is_archived).execute(conn).await
    } else {
        sqlx::query("DELETE FROM categories WHERE id=?")
            .bind(&record.id)
            .execute(conn)
            .await
    };
    match result {
        Ok(_) => Ok(true),
        Err(e)
            if e.as_database_error()
                .is_some_and(|e| e.is_unique_violation() || e.is_foreign_key_violation()) =>
        {
            Ok(false)
        }
        Err(_) => Err(DATABASE.into()),
    }
}
async fn reconcile(
    conn: &mut SqliteConnection,
    record: &Record,
    bootstrap: bool,
    own: bool,
) -> Result<(), String> {
    let local = current(conn, &record.key()).await?;
    let mut dirty: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_dirty WHERE record_id=?)")
            .bind(&record.key())
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
    // First-contact seeds and compatible ledger metadata upgrades are safe to
    // retry when a dependency arrives later. User-edited values stay dirty.
    let metadata_upgrade = matches!((&local.value,&record.value),
        (Some(Value::Transaction(a)),Some(Value::Transaction(b)))
        if crate::cloud_sync_ledger::same_ledger(a,b)
          && ((a.payroll.is_none() && b.payroll.is_some()) || (a.links.is_none() && b.links.is_some()))
          && (a.payroll.is_none() || a.payroll == b.payroll)
          && (a.loan.is_none() || a.loan == b.loan));
    if dirty && !own && ((bootstrap && unchanged_seed(&local)) || metadata_upgrade) {
        sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
            .bind(record.key())
            .execute(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
        dirty = false;
    }
    let upgrade = matches!((&local.value,&record.value),(Some(Value::Transaction(a)),Some(Value::Transaction(b))) if ((a.payroll.is_some() && b.payroll.is_none()) || (a.links.is_some() && b.links.is_none())) && crate::cloud_sync_ledger::same_ledger(a,b));
    let conflict = if upgrade {
        sqlx::query(
            "INSERT INTO cloud_sync_dirty(record_id) VALUES(?) ON CONFLICT(record_id) DO NOTHING",
        )
        .bind(record.key())
        .execute(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
        false
    } else if own && dirty {
        false
    } else if local.equivalent(record) {
        sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
            .bind(&record.key())
            .execute(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
        false
    } else if dirty && !(bootstrap && unchanged_seed(&local)) {
        true
    } else if apply(conn, record).await? {
        sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
            .bind(&record.key())
            .execute(&mut *conn)
            .await
            .map_err(|_| DATABASE)?;
        false
    } else {
        true
    };
    sqlx::query("UPDATE cloud_sync_shadow SET conflict=? WHERE record_id=?")
        .bind(conflict)
        .bind(&record.key())
        .execute(conn)
        .await
        .map_err(|_| DATABASE)?;
    Ok(())
}
pub async fn apply_page(
    pool: &SqlitePool,
    expected: &Head,
    changes: &[(Change, Record)],
    end: &Head,
    cursor: &str,
    complete: bool,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let (sequence, hash, bootstrapped): (i64, String, bool) =
        sqlx::query_as("SELECT sequence,hash,bootstrapped FROM cloud_sync_binding WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    if sequence.to_string() != expected.sequence || hash != expected.hash {
        return Err(UNSAFE.into());
    }
    sqlx::query("UPDATE cloud_sync_binding SET applying=1 WHERE id=1")
        .execute(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    let financial: bool =
        sqlx::query_scalar("SELECT financial_enabled FROM cloud_sync_binding WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    let income: bool =
        sqlx::query_scalar("SELECT income_enabled FROM cloud_sync_binding WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    let planner: bool =
        sqlx::query_scalar("SELECT planner_enabled FROM cloud_sync_binding WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    let full: bool = sqlx::query_scalar("SELECT full_enabled FROM cloud_sync_binding WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    for (change, record) in changes {
        if record.schema == 5 && !full {
            return Err(crate::cloud_sync_metadata::CONSENT.into());
        }
        if record.schema == 4 && !planner {
            return Err(crate::cloud_sync_planner::CONSENT.into());
        }
        if record.schema == 3 && !income {
            return Err(crate::cloud_sync_income::CONSENT.into());
        }
        if record.entity != "category" && !financial {
            return Err(crate::cloud_sync_ledger::CONSENT.into());
        }
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT version FROM cloud_sync_shadow WHERE record_id=?),0)",
        )
        .bind(&record.key())
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
        if version.to_string() != change.request.base_version {
            return Err(UNSAFE.into());
        }
        let pending: Option<(String, i64, String)> =
            sqlx::query_as("SELECT record_id,revision,request FROM cloud_sync_pending WHERE id=1")
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| DATABASE)?;
        let mut own = false;
        if let Some((id, revision, body)) = pending {
            let sent: Mutation = serde_json::from_str(&body).map_err(|_| DATABASE)?;
            if sent.mutation_id == change.request.mutation_id {
                if sent != change.request || id != record.key() {
                    return Err(UNSAFE.into());
                }
                own = true;
                sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=? AND revision=?")
                    .bind(&id)
                    .bind(revision)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| DATABASE)?;
                sqlx::query("DELETE FROM cloud_sync_pending")
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| DATABASE)?;
            }
        }
        sqlx::query("INSERT INTO cloud_sync_shadow(record_id,version,payload) VALUES(?,?,?) ON CONFLICT(record_id) DO UPDATE SET version=EXCLUDED.version,payload=EXCLUDED.payload")
            .bind(&record.key()).bind(protocol::number(&change.version)?).bind(serde_json::to_string(record).map_err(|_| DATABASE)?).execute(&mut *tx).await.map_err(|_| DATABASE)?;
        if bootstrapped {
            reconcile(
                &mut tx,
                record,
                version == 0 && record.entity == "metadata",
                own,
            )
            .await?;
        }
    }
    if !bootstrapped && complete {
        let records: Vec<String> =
            sqlx::query_scalar("SELECT payload FROM cloud_sync_shadow ORDER BY CASE WHEN record_id='metadata:settings:1' THEN -1 WHEN record_id LIKE 'metadata:%' THEN 4 WHEN record_id LIKE 'income-plan:%' THEN 3 WHEN record_id LIKE 'transaction:%' THEN 2 WHEN record_id LIKE 'income:%' THEN 1 ELSE 0 END,record_id")
                .fetch_all(&mut *tx)
                .await
                .map_err(|_| DATABASE)?;
        for body in records {
            reconcile(
                &mut tx,
                &serde_json::from_str::<Record>(&body).map_err(|_| DATABASE)?,
                true,
                false,
            )
            .await?;
        }
    }
    if complete {
        let mut previous = i64::MAX;
        loop {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_shadow WHERE conflict=1")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| DATABASE)?;
            if count == 0 || count >= previous {
                break;
            }
            previous = count;
            let retry:Vec<String>=sqlx::query_scalar("SELECT s.payload FROM cloud_sync_shadow s LEFT JOIN cloud_sync_dirty d USING(record_id) WHERE s.conflict=1 AND d.record_id IS NULL ORDER BY CASE WHEN s.record_id='metadata:settings:1' THEN -1 WHEN s.record_id LIKE 'metadata:%' THEN 4 WHEN s.record_id LIKE 'income-plan:%' THEN 3 WHEN s.record_id LIKE 'transaction:%' THEN 2 WHEN s.record_id LIKE 'income:%' THEN 1 ELSE 0 END,s.record_id").fetch_all(&mut *tx).await.map_err(|_| DATABASE)?;
            for body in retry {
                reconcile(
                    &mut tx,
                    &serde_json::from_str::<Record>(&body).map_err(|_| DATABASE)?,
                    false,
                    false,
                )
                .await?;
            }
        }
    }
    if complete && full {
        // Audit evidence can arrive after a transaction tombstone during restore.
        // Keep that evidence and make its missing ledger rows visible for review.
        sqlx::query("UPDATE reconciliations SET needs_review=1 WHERE needs_review=0 AND EXISTS(SELECT 1 FROM reconciliation_entries e LEFT JOIN transactions t ON t.id=e.transaction_id WHERE e.reconciliation_id=reconciliations.id AND t.id IS NULL)")
            .execute(&mut *tx).await.map_err(|_| DATABASE)?;
        sqlx::query("UPDATE card_statements SET needs_review=1 WHERE needs_review=0 AND EXISTS(SELECT 1 FROM card_statement_entries e LEFT JOIN transactions t ON t.id=e.transaction_id WHERE e.statement_id=card_statements.id AND t.id IS NULL)")
            .execute(&mut *tx).await.map_err(|_| DATABASE)?;
    }
    sqlx::query("UPDATE cloud_sync_binding SET sequence=?,hash=?,cursor=?,bootstrapped=?,applying=0,last_success=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1")
        .bind(end.number()?).bind(&end.hash).bind(cursor).bind(bootstrapped||complete).execute(&mut *tx).await.map_err(|_| DATABASE)?;
    tx.commit().await.map_err(|_| DATABASE.into())
}
pub async fn pending(pool: &SqlitePool) -> Result<Option<Mutation>, String> {
    let body: Option<String> =
        sqlx::query_scalar("SELECT request FROM cloud_sync_pending WHERE id=1")
            .fetch_optional(pool)
            .await
            .map_err(|_| DATABASE)?;
    body.map(|b| serde_json::from_str(&b).map_err(|_| DATABASE.into()))
        .transpose()
}
pub async fn discard_pending(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM cloud_sync_pending")
        .execute(pool)
        .await
        .map_err(|_| DATABASE)?;
    Ok(())
}
#[cfg(test)]
pub async fn prepare(
    pool: &SqlitePool,
    root: &Secret,
    bound: &Binding,
) -> Result<Option<Mutation>, String> {
    prepare_epoch(pool, root, &bound.key_id, root, bound).await
}
pub async fn prepare_epoch(
    pool: &SqlitePool,
    root: &Secret,
    encryption_id: &str,
    identity: &Secret,
    bound: &Binding,
) -> Result<Option<Mutation>, String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let (id, revision, version, record) = loop {
        let mut cleaned = false;
        let candidates:Vec<(String,i64,i64)>=sqlx::query_as("SELECT d.record_id,d.revision,COALESCE(s.version,0) FROM cloud_sync_dirty d LEFT JOIN cloud_sync_shadow s USING(record_id) WHERE COALESCE(s.conflict,0)=0 ORDER BY CASE WHEN d.record_id='metadata:settings:1' THEN -1 WHEN d.record_id LIKE 'metadata:%' THEN 4 WHEN d.record_id LIKE 'income-plan:%' THEN 3 WHEN d.record_id LIKE 'transaction:%' THEN 2 WHEN d.record_id LIKE 'income:%' THEN 1 ELSE 0 END,d.record_id").fetch_all(&mut *tx).await.map_err(|_| DATABASE)?;
        let mut selected = None;
        'candidate: for (id, revision, version) in candidates {
            let record = current(&mut tx, &id).await?;
            record.validate()?;
            let shadow: Option<String> = sqlx::query_scalar(
                "SELECT payload FROM cloud_sync_shadow WHERE record_id=? AND conflict=0",
            )
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
            if shadow.as_ref().is_some_and(|s| {
                serde_json::from_str::<Record>(s).is_ok_and(|r| record.equivalent(&r))
            }) {
                sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=? AND revision=?")
                    .bind(&id)
                    .bind(revision)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| DATABASE)?;
                cleaned = true;
                continue;
            }
            let mut dependencies = Vec::new();
            match &record.value {
                Some(Value::Metadata(value)) => dependencies
                    .extend(crate::cloud_sync_metadata_rules::dependencies(&mut tx, value).await?),
                Some(Value::Planner(value)) => {
                    let Some(dependency) = value.dependency(&mut tx).await? else {
                        continue 'candidate;
                    };
                    dependencies.push(dependency);
                }
                Some(Value::Income(value)) => dependencies.extend(value.dependencies()),
                Some(Value::Transaction(group)) => {
                    if let Some(loan) = &group.loan {
                        if let Some(id) = &loan.contract_id {
                            dependencies.push(format!("metadata:contract:{id}"));
                        }
                    }
                    if bound.full_enabled {
                        if let Some(payroll) = &group.payroll {
                            dependencies.extend(
                                payroll
                                    .breakdown
                                    .iter()
                                    .filter_map(|d| d.contract_id.as_ref())
                                    .map(|id| format!("metadata:contract:{id}")),
                            );
                        }
                        dependencies.extend(
                            group
                                .entries
                                .iter()
                                .filter_map(|e| e.payee.as_ref())
                                .map(|p| format!("metadata:payee:{}", p.id)),
                        );
                    }
                    dependencies.extend(
                        group
                            .accounts()
                            .into_iter()
                            .map(|id| format!("account:{id}")),
                    );
                    dependencies.extend(group.entries.iter().filter_map(|e| e.category_id.clone()));
                    if let Some(payroll) = &group.payroll {
                        if crate::cloud_sync_income::slot_blocked(&mut tx, payroll).await? {
                            continue 'candidate;
                        }
                        dependencies.extend(payroll.dependencies());
                    } else if bound.income_enabled {
                        for source in group
                            .entries
                            .iter()
                            .filter_map(|e| e.income_source.as_ref())
                        {
                            let exists: bool = sqlx::query_scalar(
                                "SELECT EXISTS(SELECT 1 FROM incomes WHERE id=?)",
                            )
                            .bind(&source.id)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(|_| DATABASE)?;
                            if exists {
                                dependencies.push(format!("income:{}", source.id));
                            }
                        }
                    }
                }
                _ => {}
            }
            for dependency in dependencies {
                let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow s WHERE s.record_id=? AND s.conflict=0 AND json_type(s.payload,'$.value')<>'null' AND NOT EXISTS(SELECT 1 FROM cloud_sync_dirty d WHERE d.record_id=s.record_id))").bind(dependency).fetch_one(&mut *tx).await.map_err(|_|DATABASE)?;
                if !ready {
                    continue 'candidate;
                }
            }
            selected = Some((id, revision, version, record));
            break;
        }
        if let Some(selected) = selected {
            break selected;
        }
        if !cleaned {
            tx.commit().await.map_err(|_| DATABASE)?;
            return Ok(None);
        }
    };
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&record).map_err(|_| DATABASE)?);
    let mutation = protocol::encrypt_with_identity(
        root,
        &bound.dataset_id,
        encryption_id,
        identity,
        &bound.key_id,
        &id,
        version,
        &bound.head(),
        record.value.is_none(),
        &bytes,
        record.schema,
    )?;
    sqlx::query("INSERT INTO cloud_sync_pending(id,record_id,revision,request) VALUES(1,?,?,?)")
        .bind(&id)
        .bind(revision)
        .bind(serde_json::to_string(&mutation).map_err(|_| DATABASE)?)
        .execute(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    tx.commit().await.map_err(|_| DATABASE)?;
    Ok(Some(mutation))
}
pub async fn resolve(
    pool: &SqlitePool,
    id: &str,
    local_revision: i64,
    remote_version: i64,
    choice: &str,
    name: Option<String>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let found:Option<(String,i64,i64)>=sqlx::query_as("SELECT s.payload,s.version,COALESCE(d.revision,0) FROM cloud_sync_shadow s LEFT JOIN cloud_sync_dirty d USING(record_id) WHERE s.record_id=? AND s.conflict=1").bind(id).fetch_optional(&mut *tx).await.map_err(|_| DATABASE)?;
    let (body, version, revision) =
        found.ok_or("This sync conflict has changed. Refresh and try again.")?;
    if version != remote_version || revision != local_revision {
        return Err("This sync conflict has changed. Refresh and try again.".into());
    }
    let mut remote: Record = serde_json::from_str(&body).map_err(|_| DATABASE)?;
    if choice == "local" {
        let local = current(&mut tx, id).await?;
        let incompatible = match (&local.value, &remote.value) {
            (Some(Value::Transaction(a)), Some(Value::Transaction(b))) => {
                !crate::cloud_sync_ledger::same(a, b)
            }
            (Some(Value::Income(a)), Some(Value::Income(b))) => {
                a.kind != b.kind || a.currency != b.currency
            }
            (Some(Value::Account(a)), Some(Value::Account(b))) => {
                a.kind != b.kind
                    || a.opening_balance != b.opening_balance
                    || a.currency != b.currency
            }
            _ => false,
        };
        if incompatible {
            return Err(crate::cloud_sync_ledger::BLOCKED.into());
        }
    }

    sqlx::query("UPDATE cloud_sync_binding SET applying=1 WHERE id=1")
        .execute(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    if choice == "remote" || choice == "rename" {
        if choice == "rename" {
            let value = remote
                .value
                .as_mut()
                .ok_or("A deleted category cannot be renamed.")?
                .category_mut()?;
            value.name = name
                .unwrap_or_default()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            remote.validate()?;
        }
        let applied = if remote.entity == "category" {
            apply(&mut tx, &remote).await?
        } else {
            crate::cloud_sync_ledger::apply(&mut tx, &remote, true).await?
        };
        if !applied {
            if remote.entity != "category" {
                return Err(crate::cloud_sync_ledger::BLOCKED.into());
            }
            return Err("A category name is already used or this category is still referenced. Choose a different cloud name or keep your local version.".into());
        }
        sqlx::query("DELETE FROM cloud_sync_dirty WHERE record_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
    } else if choice != "local" {
        return Err("Choose how to resolve this sync conflict.".into());
    }
    if choice != "remote" {
        sqlx::query("INSERT INTO cloud_sync_dirty(record_id) VALUES(?) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1").bind(id).execute(&mut *tx).await.map_err(|_| DATABASE)?;
    }
    sqlx::query("UPDATE cloud_sync_shadow SET conflict=0 WHERE record_id=?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    sqlx::query("UPDATE cloud_sync_binding SET applying=0 WHERE id=1")
        .execute(&mut *tx)
        .await
        .map_err(|_| DATABASE)?;
    tx.commit().await.map_err(|_| DATABASE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_options::{save, OptionKind, SaveOption};
    use sqlx::sqlite::SqlitePoolOptions;
    use zeroize::Zeroizing;
    const ORIGIN: &str = "https://api.example.test/";
    async fn database() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&pool).await.unwrap();
        enable(&pool, ORIGIN, "alice", "dataset", "key")
            .await
            .unwrap();
        pool
    }
    async fn edit(pool: &SqlitePool, id: Option<String>, name: &str) -> String {
        save(
            pool,
            SaveOption {
                logo_change: None,
                kind: OptionKind::Category,
                id,
                name: name.into(),
                is_archived: false,
                icon: Some("tag".into()),
            },
        )
        .await
        .unwrap()
        .id
    }
    fn accepted(m: Mutation) -> Change {
        Change {
            sequence: (m.previous.number().unwrap() + 1).to_string(),
            version: (protocol::number(&m.base_version).unwrap() + 1).to_string(),
            request: m,
        }
    }
    async fn deliver(pool: &SqlitePool, root: &Secret, changes: &[Change]) -> Result<(), String> {
        let bound = binding(pool).await?.unwrap();
        let mut end = bound.head();
        let mut values = Vec::new();
        for change in changes {
            end = protocol::verify_change(change, &end)?;
            values.push((
                change.clone(),
                Record::decode(change, root, "dataset", "key")?,
            ));
        }
        apply_page(pool, &bound.head(), &values, &end, "test-cursor", true).await
    }
    async fn drain(pool: &SqlitePool, root: &Secret) -> Vec<Change> {
        let mut changes = Vec::new();
        loop {
            let bound = binding(pool).await.unwrap().unwrap();
            let Some(m) = prepare(pool, root, &bound).await.unwrap() else {
                break;
            };
            let change = accepted(m);
            deliver(pool, root, &[change.clone()]).await.unwrap();
            changes.push(change);
        }
        changes
    }
    async fn name(pool: &SqlitePool, id: &str) -> Option<String> {
        current(&mut pool.acquire().await.unwrap(), id)
            .await
            .unwrap()
            .value
            .map(|v| crate::cloud_sync_ledger::summary(&v).name)
    }
    #[tokio::test]
    async fn two_devices_bootstrap_resolve_conflicts_and_preserve_edits_during_upload() {
        let root = Zeroizing::new([7; 32]);
        let a = database().await;
        let b = database().await;
        let id = edit(&a, None, "Travel").await;
        deliver(&a, &root, &[]).await.unwrap();
        let initial = drain(&a, &root).await;
        deliver(&b, &root, &initial).await.unwrap();
        assert_eq!(name(&b, &id).await.as_deref(), Some("Travel"));
        assert_eq!(status(&b, ORIGIN, "alice").await.unwrap().pending, 0);
        edit(&a, Some(id.clone()), "Trips on A").await;
        edit(&b, Some(id.clone()), "Trips on B").await;
        let changes = drain(&a, &root).await;
        deliver(&b, &root, &changes).await.unwrap();
        let state = status(&b, ORIGIN, "alice").await.unwrap();
        assert_eq!(state.conflict_count, 1);
        assert_eq!(name(&b, &id).await.as_deref(), Some("Trips on B"));
        let conflict = &state.conflicts[0];
        assert!(resolve(
            &b,
            &id,
            conflict.local_revision + 1,
            conflict.remote_version,
            "local",
            None
        )
        .await
        .is_err());
        resolve(
            &b,
            &id,
            conflict.local_revision,
            conflict.remote_version,
            "local",
            None,
        )
        .await
        .unwrap();
        deliver(&a, &root, &drain(&b, &root).await).await.unwrap();
        assert_eq!(name(&a, &id).await.as_deref(), Some("Trips on B"));
        edit(&a, Some(id.clone()), "Sent version").await;
        let bound = binding(&a).await.unwrap().unwrap();
        let sent = prepare(&a, &root, &bound).await.unwrap().unwrap();
        // The durable pending request is unchanged when a newer edit is saved.
        edit(&a, Some(id.clone()), "Newer offline edit").await;
        assert_eq!(pending(&a).await.unwrap().unwrap(), sent);
        let first = accepted(sent);
        deliver(&a, &root, &[first.clone()]).await.unwrap();
        assert_eq!(name(&a, &id).await.as_deref(), Some("Newer offline edit"));
        assert_eq!(status(&a, ORIGIN, "alice").await.unwrap().pending, 1);
        let mut changes = vec![first];
        changes.extend(drain(&a, &root).await);
        deliver(&b, &root, &changes).await.unwrap();
        assert_eq!(name(&b, &id).await.as_deref(), Some("Newer offline edit"));
        assert!(enable(&a, ORIGIN, "bob", "other-dataset", "key")
            .await
            .is_err());
        pause(&a, ORIGIN, "alice").await.unwrap();
        edit(&a, Some(id.clone()), "Paused edit").await;
        assert_eq!(status(&a, ORIGIN, "alice").await.unwrap().pending, 1);
        assert!(!status(&a, ORIGIN, "alice").await.unwrap().enabled);
    }
    #[tokio::test]
    async fn failed_page_rolls_back_and_name_collision_needs_explicit_resolution() {
        let root = Zeroizing::new([7; 32]);
        let a = database().await;
        deliver(&a, &root, &[]).await.unwrap();
        drain(&a, &root).await;
        let existing = edit(&a, None, "Same name").await;
        drain(&a, &root).await;
        let bound = binding(&a).await.unwrap().unwrap();
        let record = Record {
            schema: 1,
            entity: "category".into(),
            id: "other-category".into(),
            value: Some(Value::Category(Category {
                name: "Same name".into(),
                icon: "tag".into(),
                is_archived: false,
            })),
        };
        let m = protocol::encrypt(
            &root,
            "dataset",
            "key",
            &record.id,
            0,
            &bound.head(),
            false,
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let change = accepted(m);
        // Version mismatch after a valid first change must roll back the full page.
        let bad_record = Record {
            id: existing.clone(),
            ..record.clone()
        };
        let next = Head {
            sequence: change.sequence.clone(),
            hash: change.request.commit_hash.clone(),
        };
        let bad = accepted(
            protocol::encrypt(
                &root,
                "dataset",
                "key",
                &existing,
                0,
                &next,
                false,
                &serde_json::to_vec(&bad_record).unwrap(),
            )
            .unwrap(),
        );
        assert!(deliver(&a, &root, &[change.clone(), bad]).await.is_err());
        assert_eq!(binding(&a).await.unwrap().unwrap().head(), bound.head());
        assert_eq!(status(&a, ORIGIN, "alice").await.unwrap().conflict_count, 0);
        deliver(&a, &root, &[change]).await.unwrap();
        let state = status(&a, ORIGIN, "alice").await.unwrap();
        assert_eq!(state.conflict_count, 1);
        let c = &state.conflicts[0];
        assert!(resolve(
            &a,
            &c.record_id,
            c.local_revision,
            c.remote_version,
            "remote",
            None
        )
        .await
        .is_err());
        resolve(
            &a,
            &c.record_id,
            c.local_revision,
            c.remote_version,
            "rename",
            Some("Cloud category".into()),
        )
        .await
        .unwrap();
        assert_eq!(name(&a, &existing).await.as_deref(), Some("Same name"));
        assert_eq!(
            name(&a, "other-category").await.as_deref(),
            Some("Cloud category")
        );
        assert_eq!(status(&a, ORIGIN, "alice").await.unwrap().pending, 1);
    }
    #[tokio::test]
    async fn enqueue_is_atomic_and_exact_ciphertext_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join("sync.db"))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&pool).await.unwrap();
        enable(&pool, ORIGIN, "alice", "dataset", "key")
            .await
            .unwrap();
        let root = Zeroizing::new([7; 32]);
        deliver(&pool, &root, &[]).await.unwrap();
        drain(&pool, &root).await;
        sqlx::raw_sql("CREATE TRIGGER simulate_queue_failure BEFORE INSERT ON cloud_sync_dirty BEGIN SELECT RAISE(ABORT,'simulated queue failure'); END;").execute(&pool).await.unwrap();
        assert!(sqlx::query("INSERT INTO categories(id,name,name_key,icon) VALUES('failed','Unsaved','unsaved','tag')").execute(&pool).await.is_err());
        assert_eq!(name(&pool, "failed").await, None);
        assert_eq!(status(&pool, ORIGIN, "alice").await.unwrap().pending, 0);
        sqlx::query("DROP TRIGGER simulate_queue_failure")
            .execute(&pool)
            .await
            .unwrap();
        let id = edit(&pool, None, "Before restart").await;
        let bound = binding(&pool).await.unwrap().unwrap();
        let sent = prepare(&pool, &root, &bound).await.unwrap().unwrap();
        pool.close().await;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        crate::database::MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(pending(&pool).await.unwrap().unwrap(), sent);
        assert_eq!(binding(&pool).await.unwrap().unwrap().head(), bound.head());
        edit(&pool, Some(id.clone()), "After restart").await;
        deliver(&pool, &root, &[accepted(sent)]).await.unwrap();
        assert_eq!(name(&pool, &id).await.as_deref(), Some("After restart"));
        assert_eq!(status(&pool, ORIGIN, "alice").await.unwrap().pending, 1);
        pool.close().await;
    }
    #[tokio::test]
    async fn cloud_deletion_preserves_referenced_categories_until_resolved() {
        let pool = database().await;
        let root = Zeroizing::new([7; 32]);
        deliver(&pool, &root, &[]).await.unwrap();
        drain(&pool, &root).await;
        // Bundled planner items reference this category. A remote deletion must
        // not break local relationships that are outside this sync slice.
        let id = "planner-expense-0";
        let tombstone = Record {
            schema: 1,
            entity: "category".into(),
            id: id.into(),
            value: None,
        };
        let bound = binding(&pool).await.unwrap().unwrap();
        let deleted = protocol::encrypt(
            &root,
            "dataset",
            "key",
            id,
            1,
            &bound.head(),
            true,
            &serde_json::to_vec(&tombstone).unwrap(),
        )
        .unwrap();
        deliver(&pool, &root, &[accepted(deleted)]).await.unwrap();
        assert_eq!(name(&pool, id).await.as_deref(), Some("Water"));
        let state = status(&pool, ORIGIN, "alice").await.unwrap();
        let conflict = &state.conflicts[0];
        assert!(resolve(
            &pool,
            id,
            conflict.local_revision,
            conflict.remote_version,
            "remote",
            None
        )
        .await
        .is_err());
        resolve(
            &pool,
            id,
            conflict.local_revision,
            conflict.remote_version,
            "local",
            None,
        )
        .await
        .unwrap();
        let changes = drain(&pool, &root).await;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].request.base_version, "2");
        assert_eq!(changes[0].version, "3");
        assert_eq!(changes[0].request.operation, "put");
        assert_eq!(
            status(&pool, ORIGIN, "alice").await.unwrap().conflict_count,
            0
        );
    }
}
