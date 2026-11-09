//! Per-source, per-cycle forecast overrides. These never post ledger entries.
use crate::{
    cloud_sync_ledger::{self as ledger, BLOCKED},
    cloud_sync_protocol::UNSAFE,
    cloud_sync_store::{Record, Value, DATABASE},
};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

pub const CONSENT: &str = "Enable income forecast sync to download this cloud data.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub source_type: String,
    pub source_id: String,
    pub month: String,
    pub currency: String,
    pub amount: String,
}

// A fixed month prefix makes the composite key unambiguous even for IDs with
// underscores. The source keeps the existing 128-byte ID limit.
pub fn parts(id: &str) -> Result<(&str, &str, &str), String> {
    let mut parts = id.splitn(3, '_');
    let kind = parts.next().ok_or(UNSAFE)?;
    let month = parts.next().ok_or(UNSAFE)?;
    let source = parts.next().ok_or(UNSAFE)?;
    if !matches!(kind, "income" | "deduction") {
        return Err(UNSAFE.into());
    }
    crate::cashflow::month_index(month).map_err(|_| UNSAFE)?;
    ledger::id(source)?;
    Ok((kind, month, source))
}
impl Override {
    pub fn validate(&self, id: &str) -> Result<(), String> {
        let (kind, month, source) = parts(id)?;
        if kind != self.source_type
            || month != self.month
            || source != self.source_id
            || self.currency != "THB"
            || ledger::amount(&self.amount)? < 0
        {
            return Err(UNSAFE.into());
        }
        Ok(())
    }
    pub async fn dependency(&self, conn: &mut SqliteConnection) -> Result<Option<String>, String> {
        let owner: Option<String> = if self.source_type == "income" {
            sqlx::query_scalar("SELECT id FROM incomes WHERE id=?")
                .bind(&self.source_id)
                .fetch_optional(conn)
                .await
        } else {
            sqlx::query_scalar("SELECT income_id FROM income_deductions WHERE id=?")
                .bind(&self.source_id)
                .fetch_optional(conn)
                .await
        }
        .map_err(|_| DATABASE)?;
        Ok(owner.map(|id| format!("income:{id}")))
    }
}
fn table(kind: &str) -> (&'static str, &'static str) {
    if kind == "income" {
        ("planner_income_amounts", "income_id")
    } else {
        ("planner_deduction_amounts", "deduction_id")
    }
}
pub async fn current(conn: &mut SqliteConnection, id: &str) -> Result<Record, String> {
    let (kind, month, source) = parts(id)?;
    let (table, column) = table(kind);
    let amount: Option<String> = sqlx::query_scalar(&format!(
        "SELECT CAST(amount AS TEXT) FROM {table} WHERE {column}=? AND month=?"
    ))
    .bind(source)
    .bind(month)
    .fetch_optional(conn)
    .await
    .map_err(|_| DATABASE)?;
    Ok(Record {
        schema: 4,
        entity: "income-plan".into(),
        id: id.into(),
        value: amount.map(|amount| {
            Value::Planner(Override {
                source_type: kind.into(),
                source_id: source.into(),
                month: month.into(),
                currency: "THB".into(),
                amount,
            })
        }),
    })
}
pub async fn apply(
    conn: &mut SqliteConnection,
    id: &str,
    value: Option<&Override>,
) -> Result<(), String> {
    let (kind, month, source) = parts(id)?;
    let (table, column) = table(kind);
    if let Some(v) = value {
        v.validate(id)?;
        let dependency = v.dependency(conn).await?.ok_or(BLOCKED)?;
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND conflict=1)",
        )
        .bind(dependency)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
        let currency: Option<String> =
            sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
        if blocked || currency.as_deref() != Some("THB") {
            return Err(BLOCKED.into());
        }
        sqlx::query(&format!("INSERT INTO {table}({column},month,amount) VALUES(?,?,?) ON CONFLICT({column},month) DO UPDATE SET amount=excluded.amount"))
            .bind(source).bind(month).bind(ledger::amount(&v.amount)?).execute(conn).await.map_err(|_| BLOCKED)?;
    } else {
        sqlx::query(&format!("DELETE FROM {table} WHERE {column}=? AND month=?"))
            .bind(source)
            .bind(month)
            .execute(conn)
            .await
            .map_err(|_| BLOCKED)?;
    }
    Ok(())
}

// Do not cascade away a competing local forecast or a still-live cloud forecast
// when a remote definition disappears. Its tombstones must be reconciled first.
pub async fn protect_source(
    conn: &mut SqliteConnection,
    kind: &str,
    source: &str,
) -> Result<(), String> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT planner_enabled FROM cloud_sync_binding WHERE id=1),0)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| DATABASE)?;
    if !enabled {
        return Ok(());
    }
    let (table, column) = table(kind);
    let blocked: bool = sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM {table} p WHERE p.{column}=? AND (EXISTS(SELECT 1 FROM cloud_sync_dirty d WHERE d.record_id='income-plan:'||?||'_'||p.month||'_'||p.{column}) OR EXISTS(SELECT 1 FROM cloud_sync_shadow s WHERE s.record_id='income-plan:'||?||'_'||p.month||'_'||p.{column} AND (s.conflict=1 OR json_type(s.payload,'$.value')<>'null'))))"))
        .bind(source).bind(kind).bind(kind).fetch_one(conn).await.map_err(|_| DATABASE)?;
    if blocked {
        Err(BLOCKED.into())
    } else {
        Ok(())
    }
}

pub async fn enable(pool: &SqlitePool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let enabled: bool = sqlx::query_scalar(
        "SELECT planner_enabled FROM cloud_sync_binding WHERE id=1 AND income_enabled=1",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| DATABASE)?;
    if !enabled {
        sqlx::query("UPDATE cloud_sync_binding SET planner_enabled=1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
        for (kind, table, column) in [
            ("income", "planner_income_amounts", "income_id"),
            ("deduction", "planner_deduction_amounts", "deduction_id"),
        ] {
            sqlx::query(&format!("INSERT INTO cloud_sync_dirty(record_id) SELECT 'income-plan:{kind}_'||month||'_'||{column} FROM {table} WHERE true ON CONFLICT(record_id) DO UPDATE SET revision=revision+1"))
                .execute(&mut *tx).await.map_err(|_| DATABASE)?;
        }
    }
    // An older pending request must keep its exact ciphertext across opt-in.
    tx.commit().await.map_err(|_| DATABASE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud_sync_income,
        cloud_sync_protocol::{self as protocol, Change},
        cloud_sync_store as store,
    };
    use zeroize::Zeroizing;
    const ORIGIN: &str = "https://example.test";
    async fn db(planner: bool) -> SqlitePool {
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
        store::enable(&p, ORIGIN, "alice", "dataset", "key")
            .await
            .unwrap();
        ledger::enable(&p).await.unwrap();
        cloud_sync_income::enable(&p).await.unwrap();
        if planner {
            enable(&p).await.unwrap();
        }
        p
    }
    async fn seed(p: &SqlitePool) {
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',12345,12345); INSERT INTO incomes(id,name,type,destination_account_id,estimated_amount,recurrence_frequency,recurrence_day_of_month) VALUES('salary','Salary','salary','bank',100000,'monthly',25); INSERT INTO income_deductions(id,income_id,name,amount) VALUES('tax','salary','Tax',1000),('fee','salary','Fee',1000);").execute(p).await.unwrap();
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
        store::apply_page(p, &head, &decoded, &end, "test", true).await
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
            let c = Change {
                sequence: (m.previous.number().unwrap() + 1).to_string(),
                version: (protocol::number(&m.base_version).unwrap() + 1).to_string(),
                request: m,
            };
            deliver(p, &[c.clone()]).await.unwrap();
            result.push(c);
            assert!(result.len() < 100);
        }
        result
    }
    async fn shared() -> (SqlitePool, SqlitePool) {
        let a = db(true).await;
        seed(&a).await;
        deliver(&a, &[]).await.unwrap();
        let b = db(true).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        (a, b)
    }
    async fn edit(p: &SqlitePool, kind: &str, source: &str, month: &str, amount: Option<i64>) {
        let (table, col) = table(kind);
        if let Some(amount) = amount {
            sqlx::query(&format!("INSERT INTO {table} VALUES(?,?,?) ON CONFLICT({col},month) DO UPDATE SET amount=excluded.amount")).bind(source).bind(month).bind(amount).execute(p).await.unwrap();
        } else {
            sqlx::query(&format!("DELETE FROM {table} WHERE {col}=? AND month=?"))
                .bind(source)
                .bind(month)
                .execute(p)
                .await
                .unwrap();
        }
    }
    async fn amount(p: &SqlitePool, id: &str) -> Option<String> {
        let mut c = p.acquire().await.unwrap();
        current(&mut c, id).await.unwrap().value.map(|v| match v {
            Value::Planner(v) => v.amount,
            _ => panic!(),
        })
    }
    async fn choose(p: &SqlitePool, id: &str, choice: &str) {
        let c = store::status(p, ORIGIN, "alice")
            .await
            .unwrap()
            .conflicts
            .into_iter()
            .find(|c| c.record_id == id)
            .unwrap();
        store::resolve(p, id, c.local_revision, c.remote_version, choice, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn forecasts_round_trip_exact_zero_clear_and_months_without_posting_money() {
        let (a, b) = shared().await;
        edit(&a, "income", "salary", "2026-12", Some(9007199254740993)).await;
        edit(&a, "deduction", "tax", "2026-12", Some(0)).await;
        edit(&a, "deduction", "tax", "2027-01", Some(123)).await;
        let changes = drain(&a).await;
        assert_eq!(changes.len(), 3);
        assert!(changes.iter().all(|c| c.request.protocol == 4));
        deliver(&b, &changes).await.unwrap();
        assert_eq!(
            amount(&b, "income_2026-12_salary").await.as_deref(),
            Some("9007199254740993")
        );
        assert_eq!(
            amount(&b, "deduction_2026-12_tax").await.as_deref(),
            Some("0")
        );
        edit(&b, "deduction", "tax", "2026-12", None).await;
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(amount(&a, "deduction_2026-12_tax").await, None);
        assert_eq!(
            amount(&a, "deduction_2027-01_tax").await.as_deref(),
            Some("123")
        );
        for p in [&a, &b] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT current_balance FROM accounts WHERE id='bank'"
                )
                .fetch_one(p)
                .await
                .unwrap(),
                12345
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions")
                    .fetch_one(p)
                    .await
                    .unwrap(),
                0
            );
            assert_eq!(store::status(p, ORIGIN, "alice").await.unwrap().pending, 0);
        }
    }
    #[tokio::test]
    async fn consent_preserves_legacy_ciphertext_and_atomic_download_checkpoint() {
        let a = db(false).await;
        seed(&a).await;
        deliver(&a, &[]).await.unwrap();
        drain(&a).await;
        edit(&a, "income", "salary", "2026-11", Some(0)).await;
        assert_eq!(store::status(&a, ORIGIN, "alice").await.unwrap().pending, 0);
        sqlx::query("UPDATE incomes SET name='New salary' WHERE id='salary'")
            .execute(&a)
            .await
            .unwrap();
        let bound = store::binding(&a).await.unwrap().unwrap();
        let pending = store::prepare(&a, &Zeroizing::new([9; 32]), &bound)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pending.protocol, 3);
        enable(&a).await.unwrap();
        enable(&a).await.unwrap();
        assert_eq!(store::pending(&a).await.unwrap().unwrap(), pending);
        // A fresh device must explicitly consent before even staging a v4 page.
        store::discard_pending(&a).await.unwrap();
        let changes = drain(&a).await;
        let b = db(false).await;
        let v4 = changes
            .into_iter()
            .find(|c| c.request.protocol == 4)
            .unwrap();
        let r = Record::decode(&v4, &Zeroizing::new([9; 32]), "dataset", "key").unwrap();
        let head = store::binding(&b).await.unwrap().unwrap().head();
        assert_eq!(
            store::apply_page(&b, &head, &[(v4, r)], &head, "test", true)
                .await
                .unwrap_err(),
            CONSENT
        );
        assert_eq!(store::binding(&b).await.unwrap().unwrap().head(), head);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM cloud_sync_shadow")
                .fetch_one(&b)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn concurrent_months_merge_same_month_and_clear_need_explicit_choice() {
        let (a, b) = shared().await;
        edit(&a, "income", "salary", "2026-11", Some(100)).await;
        edit(&b, "income", "salary", "2026-12", Some(200)).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(
            store::status(&a, ORIGIN, "alice")
                .await
                .unwrap()
                .conflict_count,
            0
        );
        edit(&a, "income", "salary", "2026-11", None).await;
        edit(&b, "income", "salary", "2026-11", Some(300)).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(
            amount(&b, "income_2026-11_salary").await.as_deref(),
            Some("300")
        );
        assert_eq!(
            store::status(&b, ORIGIN, "alice")
                .await
                .unwrap()
                .conflict_count,
            1
        );
        assert!(drain(&b).await.is_empty());
        choose(&b, "income-plan:income_2026-11_salary", "local").await;
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(
            amount(&a, "income_2026-11_salary").await.as_deref(),
            Some("300")
        );
        edit(&a, "income", "salary", "2026-11", Some(400)).await;
        edit(&b, "income", "salary", "2026-11", None).await;
        deliver(&a, &drain(&b).await).await.unwrap();
        choose(&a, "income-plan:income_2026-11_salary", "remote").await;
        assert_eq!(amount(&a, "income_2026-11_salary").await, None);
    }
    #[tokio::test]
    async fn deleting_deduction_cannot_cascade_away_an_offline_override() {
        let (a, b) = shared().await;
        edit(&a, "deduction", "tax", "2026-11", Some(100)).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        edit(&b, "deduction", "tax", "2026-11", Some(200)).await;
        sqlx::query("DELETE FROM income_deductions WHERE id='tax'")
            .execute(&a)
            .await
            .unwrap();
        let changes = drain(&a).await;
        assert_eq!(changes.len(), 2);
        deliver(&b, &changes).await.unwrap();
        assert_eq!(
            amount(&b, "deduction_2026-11_tax").await.as_deref(),
            Some("200")
        );
        assert_eq!(
            store::status(&b, ORIGIN, "alice")
                .await
                .unwrap()
                .conflict_count,
            2
        );
        choose(&b, "income-plan:deduction_2026-11_tax", "remote").await;
        deliver(&b, &[]).await.unwrap();
        assert_eq!(
            store::status(&b, ORIGIN, "alice")
                .await
                .unwrap()
                .conflict_count,
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM income_deductions WHERE id='tax'")
                .fetch_one(&b)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn source_removal_can_be_rejected_without_losing_its_forecast() {
        let (a, b) = shared().await;
        edit(&a, "deduction", "tax", "2026-11", Some(100)).await;
        deliver(&b, &drain(&a).await).await.unwrap();
        edit(&b, "deduction", "tax", "2026-11", Some(200)).await;
        sqlx::query("DELETE FROM income_deductions WHERE id='tax'")
            .execute(&a)
            .await
            .unwrap();
        deliver(&b, &drain(&a).await).await.unwrap();
        choose(&b, "income-plan:deduction_2026-11_tax", "local").await;
        assert!(drain(&b).await.is_empty()); // parent definition still conflicted
        choose(&b, "income:salary", "local").await;
        deliver(&a, &drain(&b).await).await.unwrap();
        assert_eq!(
            amount(&a, "deduction_2026-11_tax").await.as_deref(),
            Some("200")
        );
    }
    #[tokio::test]
    async fn bootstrap_dependencies_and_invalid_payloads_are_safe() {
        let a = db(true).await;
        seed(&a).await;
        deliver(&a, &[]).await.unwrap();
        edit(&a, "deduction", "tax", "0001-01", Some(0)).await;
        let changes = drain(&a).await;
        let b = db(true).await;
        deliver(&b, &changes).await.unwrap();
        assert_eq!(
            amount(&b, "deduction_0001-01_tax").await.as_deref(),
            Some("0")
        );
        let mut c = b.acquire().await.unwrap();
        let record = current(&mut c, "deduction_0001-01_tax").await.unwrap();
        for (field, value) in [
            ("amount", "-1"),
            ("amount", "01"),
            ("amount", "9223372036854775808"),
            ("currency", "USD"),
            ("month", "0000-01"),
            ("source_id", "fee"),
        ] {
            let mut bad = serde_json::to_value(&record).unwrap();
            bad["value"][field] = value.into();
            assert!(serde_json::from_value::<Record>(bad)
                .unwrap()
                .validate()
                .is_err());
        }
        for id in [
            "income_2026-13_salary",
            "income_2026-1_salary",
            "income_2026-01_",
            "other_2026-01_salary",
        ] {
            assert!(parts(id).is_err());
        }
        assert!(parts(&format!("deduction_9999-12_{}", "a".repeat(128))).is_ok());
        let mut missing = record.clone();
        missing.id = "deduction_0001-01_missing".into();
        if let Some(Value::Planner(v)) = &mut missing.value {
            v.source_id = "missing".into();
        }
        assert!(!ledger::apply(&mut c, &missing, false).await.unwrap());
        assert!(current(&mut c, "deduction_0001-01_missing")
            .await
            .unwrap()
            .value
            .is_none());
    }
    #[tokio::test]
    async fn editing_during_upload_preserves_new_value_and_dirty_revision() {
        let (a, b) = shared().await;
        edit(&a, "income", "salary", "2026-11", Some(100)).await;
        let bound = store::binding(&a).await.unwrap().unwrap();
        let m = store::prepare(&a, &Zeroizing::new([9; 32]), &bound)
            .await
            .unwrap()
            .unwrap();
        edit(&a, "income", "salary", "2026-11", Some(200)).await;
        let change = Change {
            sequence: (m.previous.number().unwrap() + 1).to_string(),
            version: "1".into(),
            request: m,
        };
        deliver(&a, &[change.clone()]).await.unwrap();
        deliver(&b, &[change]).await.unwrap();
        assert_eq!(
            amount(&a, "income_2026-11_salary").await.as_deref(),
            Some("200")
        );
        deliver(&b, &drain(&a).await).await.unwrap();
        assert_eq!(
            amount(&b, "income_2026-11_salary").await.as_deref(),
            Some("200")
        );
    }
}
