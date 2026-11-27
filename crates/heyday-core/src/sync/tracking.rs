//! One encrypted record owns a Tracking account and all of its immutable events.
use super::{
    metadata::{get, Metadata, Row},
    store::{Record, Value},
};
use crate::services::tracking::{self, Snapshot};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn decode(v: &Metadata) -> Result<Snapshot, String> {
    let s: Snapshot = serde_json::from_str(get(&v.fields, "payload")?).map_err(err)?;
    if s.id != get(&v.fields, "account_id")? {
        return Err("Tracking record identity mismatch.".into());
    }
    s.validate()?;
    Ok(s)
}
pub fn dependencies(v: &Metadata) -> Result<Vec<String>, String> {
    let s = decode(v)?;
    let mut deps = vec![];
    if let Some(loan) = s.draft.linked_loan_id {
        deps.push(format!("account:{loan}"))
    }
    if let Some(logo) = s.draft.logo_asset_id {
        deps.push(format!("metadata:logo:{logo}"))
    }
    for e in s.events {
        if let Some(cash) = e.cash_account_id {
            deps.push(format!("account:{cash}"))
        }
    }
    deps.sort();
    deps.dedup();
    Ok(deps)
}
pub async fn current(c: &mut SqliteConnection, id: &str) -> Result<Record, String> {
    let s = tracking::snapshot(c, id).await?;
    let value = if let Some(mut s) = s {
        if let Some(logo) = &s.draft.logo_asset_id {
            s.draft.logo_asset_id = Some(
                sqlx::query_scalar("SELECT content_hash FROM logo_assets WHERE id=?")
                    .bind(logo)
                    .fetch_one(&mut *c)
                    .await
                    .map_err(err)?,
            );
        }
        let fields: Row = BTreeMap::from([
            ("account_id".into(), Some(id.into())),
            (
                "payload".into(),
                Some(serde_json::to_string(&s).map_err(err)?),
            ),
        ]);
        Some(Value::Metadata(Metadata {
            kind: "tracking".into(),
            fields,
            children: BTreeMap::new(),
        }))
    } else {
        None
    };
    Ok(Record {
        schema: 6,
        entity: "metadata".into(),
        id: format!("tracking:{id}"),
        value,
    })
}
pub async fn apply(c: &mut SqliteConnection, id: &str, v: Option<&Metadata>) -> Result<(), String> {
    if let Some(v) = v {
        let mut s = decode(v)?;
        if let Some(hash) = &s.draft.logo_asset_id {
            s.draft.logo_asset_id = Some(
                sqlx::query_scalar("SELECT id FROM logo_assets WHERE content_hash=?")
                    .bind(hash)
                    .fetch_optional(&mut *c)
                    .await
                    .map_err(err)?
                    .ok_or("Tracking logo dependency is missing.")?,
            );
        }
        tracking::apply(c, &s, true).await
    } else {
        if let Some(s) = tracking::snapshot(c, id).await? {
            if !s.archived || !s.events.is_empty() {
                return Err("Archive an empty Tracking account before deleting it.".into());
            }
            super::metadata_rules::guard_references(c, &format!("metadata:tracking:{id}")).await?;
            sqlx::query("DELETE FROM tracking_accounts WHERE account_id=?")
                .bind(id)
                .execute(&mut *c)
                .await
                .map_err(err)?;
            sqlx::query("DELETE FROM accounts WHERE id=?")
                .bind(id)
                .execute(c)
                .await
                .map_err(err)?;
        }
        Ok(())
    }
}
