use super::{
    metadata::{get, Metadata, Row},
    store::{Record, Value},
};
use crate::services::insurance::{self, Policy};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;
pub fn decode(v: &Metadata) -> Result<Policy, String> {
    let p: Policy = serde_json::from_str(get(&v.fields, "payload")?).map_err(|e| e.to_string())?;
    if p.id != get(&v.fields, "id")? {
        return Err("Insurance identity mismatch.".into());
    }
    p.validate()?;
    Ok(p)
}
pub fn dependencies(v: &Metadata) -> Result<Vec<String>, String> {
    let p = decode(v)?;
    let mut deps = vec![];
    for a in p
        .details
        .asset_ids
        .iter()
        .chain(p.details.cash_value_id.iter())
    {
        deps.push(format!("metadata:tracking:{a}"))
    }
    for s in &p.schedules {
        deps.push(format!("account:{}", s.account_id));
        deps.push(s.category_id.clone());
    }
    if let Some(s) = p.details.replaces {
        deps.push(format!("metadata:{}:{}", s.kind, s.id))
    }
    deps.sort();
    deps.dedup();
    Ok(deps)
}
pub async fn current(c: &mut SqliteConnection, id: &str) -> Result<Record, String> {
    let value = insurance::get(c, id)
        .await?
        .map(|p| {
            serde_json::to_string(&p).map(|json| {
                Value::Metadata(Metadata {
                    kind: "insurance".into(),
                    fields: BTreeMap::from([
                        ("id".into(), Some(id.into())),
                        ("payload".into(), Some(json)),
                    ]) as Row,
                    children: BTreeMap::new(),
                })
            })
        })
        .transpose()
        .map_err(|e| e.to_string())?;
    Ok(Record {
        schema: 7,
        entity: "metadata".into(),
        id: format!("insurance:{id}"),
        value,
    })
}
pub async fn apply(c: &mut SqliteConnection, id: &str, v: Option<&Metadata>) -> Result<(), String> {
    if let Some(v) = v {
        insurance::apply(c, &decode(v)?, true).await
    } else {
        if let Some(p) = insurance::get(c, id).await? {
            if p.details.status != "archived" || !p.payments.is_empty() {
                return Err("Archive an unused policy before deleting it.".into());
            }
        }
        sqlx::query("DELETE FROM insurance_policies WHERE id=?")
            .bind(id)
            .execute(c)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
