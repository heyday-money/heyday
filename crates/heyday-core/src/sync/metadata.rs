//! Allowlisted, encrypted domain records. SQL identifiers always come from the
//! compiled schema below; wire values are parameterized and money stays exact.
use crate::{
    cloud_sync_ledger::{self as ledger, BLOCKED},
    cloud_sync_protocol::UNSAFE,
    cloud_sync_store::{Record, Value, DATABASE},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sqlx::{Row as SqlRow, SqliteConnection, SqlitePool};
use std::collections::BTreeMap;

pub const CONSENT: &str = "Enable complete financial sync to download this cloud data.";
pub type Row = BTreeMap<String, Option<String>>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub kind: String,
    pub fields: Row,
    pub children: BTreeMap<String, Vec<Row>>,
}
pub struct Spec {
    pub kind: &'static str,
    pub table: &'static str,
    pub keys: &'static str,
    pub columns: &'static str,
    pub children: &'static [Child],
}
pub struct Child {
    pub name: &'static str,
    pub table: &'static str,
    pub parent: &'static str,
    pub columns: &'static str,
}
// Suffix ?: nullable; types: id, name, text:N, int:min:max, money, signed,
// date, month, enum:a|b, logo (content hash), path, url, timestamp, png, hash.
pub const SPECS: &[Spec] = &[
    Spec{kind:"logo",table:"logo_assets",keys:"content_hash",columns:"content_hash=hash,mime_type=enum:image/png,width=int:1:128,height=int:1:128,data=png",children:&[]},
    Spec{kind:"institution",table:"institutions",keys:"id",columns:"id=id,name=name,short_name=text:100?,bank_code=text:100?,swift_code=text:100?,logo=path?,is_archived=int:0:1,logo_asset_id=logo?,logo_mode=enum:default|custom|none",children:&[]},
    Spec{kind:"payee",table:"payees",keys:"id",columns:"id=id,name=name,is_archived=int:0:1,logo_asset_id=logo?",children:&[]},
    Spec{kind:"provider",table:"subscription_providers",keys:"id",columns:"id=id,name=name,is_archived=int:0:1,builtin_icon=path?,logo_mode=enum:default|custom|none,logo_asset_id=logo?",children:&[]},
    Spec{kind:"account-logo",table:"account_logos",keys:"account_id",columns:"account_id=id,logo_asset_id=logo",children:&[]},
    Spec{kind:"account-link",table:"accounts",keys:"id",columns:"id=id,institution_id=id",children:&[]},
    Spec{kind:"facility",table:"loan_facilities",keys:"account_id",columns:"account_id=id,credit_limit=money",children:&[]},
    Spec{kind:"contract",table:"loan_contracts",keys:"id",columns:"id=id,account_id=id,name=name,principal=int:1:9223372036854775807,basis_principal=money,borrowing_date=date,receiving_account_id=id,payment_account_id=id,interest_rate=int:0:1000000,monthly_amount=int:1:9223372036854775807,installment_count=int:1:600,first_due_date=date,schedule_end=date?,notes=text:2000,borrowing_kind=enum:new|existing,borrowing_transaction_id=id?,needs_review=int:0:1",children:&[]},
    Spec{kind:"installment",table:"installments",keys:"id",columns:"id=id,name=name,account_id=id,debt_account_id=id,monthly_amount=int:1:9223372036854775807,installment_count=int:1:600,first_due_date=date,purchase_kind=enum:existing_purchase|new_purchase,purchase_transaction_id=id?,interest_rate_millis=money?",children:&[]},
    Spec{kind:"subscription",table:"subscriptions",keys:"id",columns:"id=id,name=name,account_id=id,category_id=id?,amount=int:1:9223372036854775807,frequency=enum:monthly|yearly,first_billing_date=date,end_date=date?,is_active=int:0:1,logo_asset_id=logo?,managed_via=enum:apple_app_store|google_play|website|in_app|other?,management_url=url?,provider_id=id?",children:&[]},
    Spec{kind:"payment-plan",table:"payment_plans",keys:"id",columns:"id=id,name=name,type=enum:expense|repayment,account_id=id,destination_account_id=id?,category_id=id?,amount=int:1:9223372036854775807,date=date",children:&[]},
    Spec{kind:"planner-item",table:"planner_items",keys:"id",columns:"id=id,category_id=enum:income|deductions|debt|installments|cards|expenses,name=name,description=text:2000,card_name=text:100,transaction_category_id=id?,schedule_amount=money?,schedule_start=month?,schedule_end=month?",children:&[]},
    Spec{kind:"planner-amount",table:"planner_amounts",keys:"item_id,month",columns:"item_id=id,month=month,amount=money",children:&[]},
    Spec{kind:"debt-amount",table:"planner_debt_amounts",keys:"account_id,month",columns:"account_id=id,month=month,amount=money",children:&[]},
    Spec{kind:"installment-amount",table:"planner_installment_amounts",keys:"installment_id,month",columns:"installment_id=id,month=month,amount=money",children:&[]},
    Spec{kind:"cycle",table:"planner_months",keys:"month",columns:"month=month,status=enum:tracking|forecast|complete",children:&[]},
    Spec{kind:"opening",table:"planner_opening",keys:"id",columns:"id=int:1:1,month=month,amount=signed",children:&[]},
    Spec{kind:"settings",table:"settings",keys:"id",columns:"id=int:1:1,currency=currency?,period_start_day=int:1:31",children:&[]},
    Spec{kind:"card-limit",table:"card_limit_groups",keys:"id",columns:"id=id,name=name,credit_limit=money",children:&[Child{name:"members",table:"card_limit_members",parent:"group_id",columns:"account_id=id"}]},
    Spec{kind:"statement",table:"card_statements",keys:"id",columns:"id=id,account_id=id,start_date=date,end_date=date,due_date=date,amount=money,minimum=money,needs_review=int:0:1",children:&[
        Child{name:"entries",table:"card_statement_entries",parent:"statement_id",columns:"transaction_id=id"},
        Child{name:"installments",table:"card_statement_installments",parent:"statement_id",columns:"installment_id=id,date=date"},
        Child{name:"allocations",table:"card_payment_allocations",parent:"statement_id",columns:"transaction_id=id"},
        Child{name:"plan",table:"card_payment_plans",parent:"statement_id",columns:"account_id=id,date=date,mode=enum:full|minimum|custom,target=money"}]},
    Spec{kind:"default-history",table:"selective_defaults",keys:"account_id",columns:"account_id=id",children:&[Child{name:"intervals",table:"selective_defaults",parent:"account_id",columns:"start_month=month,end_month=month?"}]},
    Spec{kind:"payoff",table:"loan_payoffs",keys:"account_id",columns:"account_id=id,paid_off_on=date",children:&[]},
    Spec{kind:"reconciliation",table:"reconciliations",keys:"sync_id",columns:"sync_id=id,account_id=id,completed_at=timestamp,confirmed_balance=signed,opening_balance=signed,needs_review=int:0:1",children:&[Child{name:"entries",table:"reconciliation_entries",parent:"reconciliation_id",columns:"transaction_id=id,date=date,description=text:200,type=enum:income|expense|transfer|repayment,balance_change=signed"}]},
    Spec{kind:"verification",table:"transaction_verifications",keys:"transaction_id,account_id",columns:"transaction_id=id,account_id=id,status=enum:uncleared|cleared|reconciled,reconciliation_id=id?",children:&[]},
];
pub fn spec(kind: &str) -> Result<&'static Spec, String> {
    SPECS
        .iter()
        .find(|s| s.kind == kind)
        .ok_or_else(|| UNSAFE.into())
}
fn columns(input: &str) -> Vec<(&str, &str)> {
    input
        .split(',')
        .map(|s| s.split_once('=').unwrap())
        .collect()
}
pub fn get<'a>(row: &'a Row, key: &str) -> Result<&'a str, String> {
    row.get(key)
        .and_then(|v| v.as_deref())
        .ok_or_else(|| UNSAFE.into())
}
pub fn optional<'a>(row: &'a Row, key: &str) -> Option<&'a str> {
    row.get(key).and_then(|v| v.as_deref())
}
pub fn number(row: &Row, key: &str) -> Result<i64, String> {
    ledger::amount(get(row, key)?)
}
pub fn key(kind: &str, values: &[&str]) -> String {
    format!("metadata:{}:{}", kind, values.join(":"))
}
pub fn parse(id: &str) -> Result<(&'static Spec, Vec<&str>), String> {
    let mut p = id.split(':');
    let s = spec(p.next().ok_or(UNSAFE)?)?;
    let values: Vec<_> = p.collect();
    let keys: Vec<_> = s.keys.split(',').collect();
    if keys.len() != values.len() {
        return Err(UNSAFE.into());
    }
    for (k, v) in keys.iter().zip(&values) {
        let typ = columns(s.columns)
            .into_iter()
            .find(|(n, _)| n == k)
            .unwrap()
            .1;
        validate_cell(Some(v), typ)?;
    }
    Ok((s, values))
}
fn valid_timestamp(v: &str) -> bool {
    // Native histories use UTC ISO timestamps with optional fractional seconds.
    if !v.is_ascii()
        || v.len() < 20
        || !crate::transactions::valid_date(&v[..10])
        || &v[10..11] != "T"
        || &v[13..14] != ":"
        || &v[16..17] != ":"
        || !v.ends_with('Z')
    {
        return false;
    }
    for (part, max) in [(&v[11..13], 23), (&v[14..16], 59), (&v[17..19], 59)] {
        if !part.bytes().all(|b| b.is_ascii_digit()) || part.parse::<u8>().map_or(true, |n| n > max)
        {
            return false;
        }
    }
    v.len() == 20
        || (v.as_bytes()[19] == b'.'
            && v.len() > 21
            && v[20..v.len() - 1].bytes().all(|b| b.is_ascii_digit()))
}
fn validate_cell(value: Option<&str>, typ: &str) -> Result<(), String> {
    let nullable = typ.ends_with('?');
    let typ = typ.trim_end_matches('?');
    let Some(v) = value else {
        return if nullable { Ok(()) } else { Err(UNSAFE.into()) };
    };
    if v.contains('\0') {
        return Err(UNSAFE.into());
    }
    match typ {
        "id" => ledger::id(v)?,
        "name" => {
            if v.trim() != v || v.is_empty() || v.chars().count() > 100 {
                return Err(UNSAFE.into());
            }
        }
        "money" => {
            if ledger::amount(v)? < 0 {
                return Err(UNSAFE.into());
            }
        }
        "signed" => {
            ledger::amount(v)?;
        }
        "date" => {
            if !crate::transactions::valid_date(v) {
                return Err(UNSAFE.into());
            }
        }
        "month" => {
            crate::cashflow::month_index(v).map_err(|_| UNSAFE)?;
        }
        "currency" => ledger::currency(v)?,
        "hash" | "logo" => {
            if v.len() != 64
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(UNSAFE.into());
            }
        }
        "path" => {
            if v.len() > 100
                || v.is_empty()
                || v.contains("..")
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            {
                return Err(UNSAFE.into());
            }
        }
        "url" => {
            let u = url::Url::parse(v).map_err(|_| UNSAFE)?;
            if v.len() > 2048
                || v.chars().any(char::is_control)
                || u.scheme() != "https"
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
            {
                return Err(UNSAFE.into());
            }
        }
        "timestamp" => {
            if v.len() > 40 || !valid_timestamp(v) {
                return Err(UNSAFE.into());
            }
        }
        "png" => {
            if v.len() > 136536 {
                return Err(UNSAFE.into());
            }
            let bytes = STANDARD.decode(v).map_err(|_| UNSAFE)?;
            if bytes.is_empty() || bytes.len() > 102400 || STANDARD.encode(bytes) != v {
                return Err(UNSAFE.into());
            }
        }
        _ if typ.starts_with("text:") => {
            if v.chars().count() > typ[5..].parse::<usize>().unwrap() {
                return Err(UNSAFE.into());
            }
        }
        _ if typ.starts_with("int:") => {
            let range: Vec<i64> = typ[4..].split(':').map(|v| v.parse().unwrap()).collect();
            let n = ledger::amount(v)?;
            if n < range[0] || n > range[1] {
                return Err(UNSAFE.into());
            }
        }
        _ if typ.starts_with("enum:") => {
            if !typ[5..].split('|').any(|x| x == v) {
                return Err(UNSAFE.into());
            }
        }
        _ => return Err(UNSAFE.into()),
    }
    Ok(())
}
fn validate_row(row: &Row, fields: &str) -> Result<(), String> {
    let c = columns(fields);
    if row.len() != c.len() {
        return Err(UNSAFE.into());
    }
    for (n, t) in c {
        validate_cell(row.get(n).ok_or(UNSAFE)?.as_deref(), t)?;
    }
    Ok(())
}
impl Metadata {
    pub fn validate(&self, id: &str) -> Result<(), String> {
        let (s, values) = parse(id)?;
        if s.kind != self.kind {
            return Err(UNSAFE.into());
        }
        validate_row(&self.fields, s.columns)?;
        for (k, v) in s.keys.split(',').zip(values) {
            if get(&self.fields, k)? != v {
                return Err(UNSAFE.into());
            }
        }
        if self.children.len() != s.children.len() {
            return Err(UNSAFE.into());
        }
        for c in s.children {
            let rows = self.children.get(c.name).ok_or(UNSAFE)?;
            let mut prev = None;
            for r in rows {
                validate_row(r, c.columns)?;
                let encoded = serde_json::to_string(r).map_err(|_| UNSAFE)?;
                if prev.as_ref().is_some_and(|p| p >= &encoded) {
                    return Err(UNSAFE.into());
                }
                prev = Some(encoded);
            }
        }
        crate::cloud_sync_metadata_rules::validate(self)
    }
    pub fn record(&self) -> Record {
        let s = spec(&self.kind).unwrap();
        let ids: Vec<_> = s
            .keys
            .split(',')
            .map(|k| get(&self.fields, k).unwrap())
            .collect();
        Record {
            schema: 5,
            entity: "metadata".into(),
            id: format!("{}:{}", self.kind, ids.join(":")),
            value: Some(Value::Metadata(self.clone())),
        }
    }
}
fn projection(fields: &str) -> String {
    columns(fields)
        .into_iter()
        .map(|(n, t)| match (n, t.trim_end_matches('?')) {
            ("logo_asset_id", _) => {
                "(SELECT content_hash FROM logo_assets WHERE id=t.logo_asset_id) AS logo_asset_id"
                    .into()
            }
            ("basis_principal", _) => {
                "CAST(t.remaining_principal AS TEXT) AS basis_principal".into()
            }
            ("data", _) => "t.data AS data".into(),
            _ => format!("CAST(t.{n} AS TEXT) AS {n}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}
async fn read_rows(
    conn: &mut SqliteConnection,
    table: &str,
    fields: &str,
    clause: &str,
    args: &[&str],
) -> Result<Vec<Row>, String> {
    let query = format!("SELECT {} FROM {table} t {clause}", projection(fields));
    let mut q = sqlx::query(&query);
    for a in args {
        q = q.bind(a);
    }
    let mut rows = Vec::new();
    for raw in q.fetch_all(conn).await.map_err(|_| DATABASE)? {
        let mut r = Row::new();
        for (n, t) in columns(fields) {
            let value = if t == "png" {
                Some(STANDARD.encode(raw.get::<Vec<u8>, _>(n)))
            } else {
                raw.get::<Option<String>, _>(n)
            };
            r.insert(n.into(), value);
        }
        rows.push(r);
    }
    rows.sort_by_cached_key(|r| serde_json::to_string(r).unwrap());
    Ok(rows)
}
pub async fn current(conn: &mut SqliteConnection, id: &str) -> Result<Record, String> {
    let (s, values) = parse(id)?;
    let clause = format!(
        "WHERE {}{}",
        s.keys
            .split(',')
            .map(|k| format!("t.{k}=?"))
            .collect::<Vec<_>>()
            .join(" AND "),
        if s.kind == "account-link" {
            " AND institution_id IS NOT NULL"
        } else {
            ""
        }
    );
    let mut rows = read_rows(conn, s.table, s.columns, &clause, &values).await?;
    let value = if rows.is_empty() {
        None
    } else {
        let mut fields = rows.remove(0);
        let mut children = BTreeMap::new();
        if s.kind == "contract" {
            let basis = crate::cloud_sync_loans::basis(conn, &fields).await?;
            fields.insert("basis_principal".into(), Some(basis.to_string()));
        }
        if s.kind == "verification" {
            if let Some(local) = optional(&fields, "reconciliation_id") {
                let stable: String =
                    sqlx::query_scalar("SELECT sync_id FROM reconciliations WHERE id=?")
                        .bind(local)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|_| DATABASE)?;
                fields.insert("reconciliation_id".into(), Some(stable));
            }
        }
        let local_parent = if s.kind == "reconciliation" {
            sqlx::query_scalar::<_, String>(
                "SELECT CAST(id AS TEXT) FROM reconciliations WHERE sync_id=?",
            )
            .bind(values[0])
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?
        } else {
            values[0].to_string()
        };
        for c in s.children {
            children.insert(
                c.name.into(),
                read_rows(
                    conn,
                    c.table,
                    c.columns,
                    &format!("WHERE t.{}=?", c.parent),
                    &[&local_parent],
                )
                .await?,
            );
        }
        Some(Value::Metadata(Metadata {
            kind: s.kind.into(),
            fields,
            children,
        }))
    };
    Ok(Record {
        schema: 5,
        entity: "metadata".into(),
        id: id.into(),
        value,
    })
}
pub async fn write_row(
    conn: &mut SqliteConnection,
    table: &str,
    fields: &str,
    keys: &str,
    row: &Row,
) -> Result<(), String> {
    let mut columns = columns(fields);
    let mut values = row.clone();
    if [
        "institutions",
        "payees",
        "subscription_providers",
        "card_limit_groups",
    ]
    .contains(&table)
    {
        values.insert("name_key".into(), Some(get(row, "name")?.to_lowercase()));
        columns.push(("name_key", "text:100"));
    }
    let names: Vec<_> = columns.iter().map(|(n, _)| *n).collect();
    let updates = names
        .iter()
        .filter(|n| !keys.split(',').any(|k| k == **n))
        .map(|n| format!("{n}=excluded.{n}"))
        .collect::<Vec<_>>();
    let sql = format!(
        "INSERT INTO {table}({}) VALUES({}) ON CONFLICT({keys}) DO {}",
        names.join(","),
        vec!["?"; names.len()].join(","),
        if updates.is_empty() {
            "NOTHING".into()
        } else {
            format!("UPDATE SET {}", updates.join(","))
        }
    );
    let mut q = sqlx::query(&sql);
    for (n, t) in columns {
        let value = values.get(n).cloned().flatten();
        let t = t.trim_end_matches('?');
        q = if t == "png" {
            q.bind(value.map(|v| STANDARD.decode(v).unwrap()))
        } else if t.starts_with("int:") || ["money", "signed"].contains(&t) {
            q.bind(value.map(|v| v.parse::<i64>().unwrap()))
        } else {
            q.bind(value)
        };
    }
    q.execute(conn).await.map_err(|_| BLOCKED)?;
    Ok(())
}
pub async fn apply(
    conn: &mut SqliteConnection,
    id: &str,
    value: Option<&Metadata>,
    confirmed: bool,
) -> Result<(), String> {
    let (s, keys) = parse(id)?;
    if let Some(v) = value {
        v.validate(id)?;
        crate::cloud_sync_metadata_rules::relationships(conn, v).await?;
        if s.kind == "contract" {
            return crate::cloud_sync_loans::apply_contract(conn, v).await;
        }
        if s.kind == "logo" {
            return crate::cloud_sync_metadata_rules::install_logo(conn, v).await;
        }
        let mut fields = v.fields.clone();
        if let Some(hash) = optional(&fields, "logo_asset_id") {
            let local: String =
                sqlx::query_scalar("SELECT id FROM logo_assets WHERE content_hash=?")
                    .bind(hash)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|_| DATABASE)?
                    .ok_or(BLOCKED)?;
            fields.insert("logo_asset_id".into(), Some(local));
        }
        if s.kind == "verification" {
            if let Some(stable) = optional(&fields, "reconciliation_id") {
                let local: i64 =
                    sqlx::query_scalar("SELECT id FROM reconciliations WHERE sync_id=?")
                        .bind(stable)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(|_| DATABASE)?
                        .ok_or(BLOCKED)?;
                fields.insert("reconciliation_id".into(), Some(local.to_string()));
            }
        }
        if s.kind == "account-link" {
            let updated = sqlx::query("UPDATE accounts SET institution_id=?,institution=(SELECT name FROM institutions WHERE id=?) WHERE id=?").bind(get(&fields,"institution_id")?).bind(get(&fields,"institution_id")?).bind(keys[0]).execute(&mut *conn).await.map_err(|_|BLOCKED)?;
            if updated.rows_affected() != 1 {
                return Err(BLOCKED.into());
            }
        } else if s.kind != "default-history" {
            write_row(conn, s.table, s.columns, s.keys, &fields).await?;
        }
        let parent = if s.kind == "reconciliation" {
            sqlx::query_scalar::<_, String>(
                "SELECT CAST(id AS TEXT) FROM reconciliations WHERE sync_id=?",
            )
            .bind(keys[0])
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| DATABASE)?
        } else {
            keys[0].to_string()
        };
        for c in s.children {
            sqlx::query(&format!("DELETE FROM {} WHERE {}=?", c.table, c.parent))
                .bind(&parent)
                .execute(&mut *conn)
                .await
                .map_err(|_| BLOCKED)?;
            for r in &v.children[c.name] {
                let mut row = r.clone();
                row.insert(c.parent.into(), Some(parent.clone()));
                let fields = format!(
                    "{},{}={}",
                    c.columns,
                    c.parent,
                    if s.kind == "reconciliation" {
                        "signed"
                    } else {
                        "id"
                    }
                );
                // Children have already been removed; plain INSERT avoids inventing a conflict key.
                insert_child(conn, c.table, &fields, &row).await?;
            }
        }
    } else {
        crate::cloud_sync_metadata_rules::before_delete(conn, s, &keys, confirmed).await?;
        if s.kind == "account-link" {
            sqlx::query("UPDATE accounts SET institution_id=NULL,institution=NULL WHERE id=?")
                .bind(keys[0])
                .execute(&mut *conn)
                .await
                .map_err(|_| BLOCKED)?;
        } else {
            let sql = format!(
                "DELETE FROM {} WHERE {}",
                s.table,
                s.keys
                    .split(',')
                    .map(|k| format!("{k}=?"))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            );
            let mut q = sqlx::query(&sql);
            for k in keys {
                q = q.bind(k);
            }
            q.execute(&mut *conn).await.map_err(|_| BLOCKED)?;
        }
    }
    Ok(())
}
async fn insert_child(
    conn: &mut SqliteConnection,
    table: &str,
    fields: &str,
    row: &Row,
) -> Result<(), String> {
    let c = columns(fields);
    let sql = format!(
        "INSERT INTO {table}({}) VALUES({})",
        c.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(","),
        vec!["?"; c.len()].join(",")
    );
    let mut q = sqlx::query(&sql);
    for (n, t) in c {
        let v = row.get(n).cloned().flatten();
        q = if t == "signed" || t == "money" {
            q.bind(v.map(|v| v.parse::<i64>().unwrap()))
        } else {
            q.bind(v)
        };
    }
    q.execute(conn).await.map_err(|_| BLOCKED)?;
    Ok(())
}
pub async fn enable(pool: &SqlitePool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|_| DATABASE)?;
    let old: bool = sqlx::query_scalar(
        "SELECT full_enabled FROM cloud_sync_binding WHERE id=1 AND planner_enabled=1",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| DATABASE)?;
    if !old {
        sqlx::query("UPDATE cloud_sync_binding SET full_enabled=1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(|_| DATABASE)?;
        for s in SPECS {
            let expression = s
                .keys
                .split(',')
                .map(|k| format!("CAST({k} AS TEXT)"))
                .collect::<Vec<_>>()
                .join("||':'||");
            let sql=format!("INSERT INTO cloud_sync_dirty(record_id) SELECT DISTINCT 'metadata:{}:'||{} FROM {} WHERE {} ON CONFLICT(record_id) DO UPDATE SET revision=revision+1",s.kind,expression,s.table,if s.kind=="account-link"{"institution_id IS NOT NULL"}else{"true"});
            sqlx::query(&sql)
                .execute(&mut *tx)
                .await
                .map_err(|_| DATABASE)?;
        }
        sqlx::query("INSERT INTO cloud_sync_dirty(record_id) SELECT DISTINCT 'transaction:'||group_id FROM ledger_sync_members m WHERE EXISTS(SELECT 1 FROM transactions t WHERE t.id=m.transaction_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1").execute(&mut *tx).await.map_err(|_|DATABASE)?;
    }
    tx.commit().await.map_err(|_| DATABASE.into())
}
