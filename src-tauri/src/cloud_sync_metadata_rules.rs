use super::cloud_sync_metadata::{self as meta, get, number, optional, Metadata, Spec};
use crate::{
    cloud_sync_ledger::BLOCKED,
    cloud_sync_protocol::UNSAFE,
    cloud_sync_store::{Record, Value, DATABASE},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

pub fn validate(v: &Metadata) -> Result<(), String> {
    let f = &v.fields;
    let bad = || Err(UNSAFE.to_string());
    match v.kind.as_str() {
        "institution" | "provider" => {
            if (get(f, "logo_mode")? == "custom") != optional(f, "logo_asset_id").is_some() {
                return bad();
            }
        }
        "contract" => {
            if number(f, "basis_principal")? > number(f, "principal")?
                || get(f, "first_due_date")? < get(f, "borrowing_date")?
                || optional(f, "schedule_end")
                    .is_some_and(|e| e < get(f, "first_due_date").unwrap())
                || get(f, "account_id")? == get(f, "receiving_account_id")?
                || get(f, "account_id")? == get(f, "payment_account_id")?
            {
                return bad();
            }
        }
        "installment" => {
            if get(f, "account_id")? == get(f, "debt_account_id")? {
                return bad();
            }
        }
        "subscription" => {
            if optional(f, "end_date").is_some_and(|e| e < get(f, "first_billing_date").unwrap()) {
                return bad();
            }
        }
        "payment-plan" => {
            if get(f, "type")? == "expense" {
                if optional(f, "destination_account_id").is_some() {
                    return bad();
                }
            } else if optional(f, "destination_account_id").is_none()
                || optional(f, "category_id").is_some()
                || optional(f, "destination_account_id") == optional(f, "account_id")
            {
                return bad();
            }
        }
        "planner-item" => {
            if optional(f, "schedule_amount").is_some() {
                if optional(f, "schedule_start").is_none()
                    || optional(f, "schedule_end").is_none()
                    || get(f, "schedule_start")? > get(f, "schedule_end")?
                {
                    return bad();
                }
            } else if optional(f, "schedule_start").is_some()
                || optional(f, "schedule_end").is_some()
            {
                return bad();
            }
        }
        "statement" => {
            if get(f, "start_date")? > get(f, "end_date")?
                || get(f, "end_date")? >= get(f, "due_date")?
                || number(f, "minimum")? > number(f, "amount")?
                || v.children["plan"].len() > 1
            {
                return bad();
            }
            for p in &v.children["plan"] {
                if get(p, "date")? <= get(f, "end_date")?
                    || number(p, "target")? > number(f, "amount")?
                    || get(p, "mode")? == "full" && number(p, "target")? != number(f, "amount")?
                    || get(p, "mode")? == "minimum" && number(p, "target")? != number(f, "minimum")?
                {
                    return bad();
                }
            }
        }
        "default-history" => {
            let mut intervals = v.children["intervals"].iter().collect::<Vec<_>>();
            intervals.sort_by_key(|r| get(r, "start_month").unwrap());
            let mut end = None;
            let mut first = true;
            for r in intervals {
                let start = get(r, "start_month")?;
                if !first && end.is_none_or(|e| e > start)
                    || optional(r, "end_month").is_some_and(|e| e < start)
                {
                    return bad();
                }
                first = false;
                end = optional(r, "end_month");
            }
        }
        "verification" => {
            if (get(f, "status")? == "reconciled") != optional(f, "reconciliation_id").is_some() {
                return bad();
            }
        }
        "logo" => {
            let bytes = STANDARD.decode(get(f, "data")?).map_err(|_| UNSAFE)?;
            if format!("{:x}", Sha256::digest(&bytes)) != get(f, "content_hash")? {
                return bad();
            }
            let mut decoder = png::Decoder::new_with_limits(
                std::io::Cursor::new(bytes),
                png::Limits { bytes: 1_048_576 },
            );
            decoder
                .set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
            let mut reader = decoder.read_info().map_err(|_| UNSAFE)?;
            if i64::from(reader.info().width) != number(f, "width")?
                || i64::from(reader.info().height) != number(f, "height")?
                || reader.info().animation_control.is_some()
            {
                return bad();
            }
            let size = reader
                .output_buffer_size()
                .filter(|n| *n <= 128 * 128 * 4)
                .ok_or(UNSAFE)?;
            reader.next_frame(&mut vec![0; size]).map_err(|_| UNSAFE)?;
        }
        _ => {}
    }
    Ok(())
}
pub async fn transaction_dependency(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<String, String> {
    let group:Option<String>=sqlx::query_scalar("SELECT m.group_id FROM ledger_sync_members m JOIN transactions t ON t.id=m.transaction_id WHERE t.id=?").bind(id).fetch_optional(conn).await.map_err(|_|DATABASE)?;
    Ok(format!("transaction:{}", group.as_deref().unwrap_or(id)))
}
pub async fn dependencies(
    conn: &mut SqliteConnection,
    v: &Metadata,
) -> Result<Vec<String>, String> {
    let f = &v.fields;
    let mut result = vec![];
    let mut add = |column: &str, entity: &str| {
        if let Some(id) = optional(f, column) {
            result.push(format!("{entity}:{id}"));
        }
    };
    if v.kind != "logo" {
        add("logo_asset_id", "metadata:logo");
    }
    match v.kind.as_str() {
        "account-link" => {
            add("id", "account");
            add("institution_id", "metadata:institution");
        }
        "account-logo" | "facility" | "default-history" | "payoff" | "reconciliation" | "debt-amount" => {
            add("account_id", "account")
        }
        "contract" => {
            add("account_id", "metadata:facility");
            add("account_id", "account");
            add("receiving_account_id", "account");
            add("payment_account_id", "account");
        }
        "installment" => {
            add("account_id", "account");
            add("debt_account_id", "account");
        }
        "subscription" => {
            add("account_id", "account");
            add("category_id", "");
            add("provider_id", "metadata:provider");
        }
        "payment-plan" => {
            add("account_id", "account");
            add("destination_account_id", "account");
            add("category_id", "");
        }
        "planner-item" => add("transaction_category_id", ""),
        "planner-amount" => add("item_id", "metadata:planner-item"),
        "installment-amount" => add("installment_id", "metadata:installment"),
        "verification" => {
            add("account_id", "account");
            add("reconciliation_id", "metadata:reconciliation");
        }
        "statement" => add("account_id", "account"),
        _ => {}
    }
    for dep in &mut result {
        if dep.starts_with(':') {
            dep.remove(0);
        }
    }
    for field in ["borrowing_transaction_id", "purchase_transaction_id"] {
        if let Some(id) = optional(f, field) {
            result.push(transaction_dependency(conn, id).await?);
        }
    }
    if v.kind == "verification" {
        result.push(transaction_dependency(conn, get(f, "transaction_id")?).await?);
    }
    if v.kind == "card-limit" {
        for r in &v.children["members"] {
            result.push(format!("account:{}", get(r, "account_id")?));
        }
    }
    if v.kind == "statement" {
        for r in &v.children["installments"] {
            result.push(format!(
                "metadata:installment:{}",
                get(r, "installment_id")?
            ));
        }
        for r in &v.children["allocations"] {
            result.push(transaction_dependency(conn, get(r, "transaction_id")?).await?);
        }
        for r in &v.children["plan"] {
            result.push(format!("account:{}", get(r, "account_id")?));
        }
    }
    Ok(result)
}
async fn account_type(conn: &mut SqliteConnection, id: &str, kinds: &[&str]) -> Result<(), String> {
    let kind: Option<String> = sqlx::query_scalar("SELECT type FROM accounts WHERE id=?")
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(|_| DATABASE)?;
    if !kind.as_deref().is_some_and(|k| kinds.contains(&k)) {
        return Err(BLOCKED.into());
    }
    Ok(())
}
pub async fn relationships(conn: &mut SqliteConnection, v: &Metadata) -> Result<(), String> {
    for dep in dependencies(conn, v).await? {
        let conflict: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cloud_sync_shadow WHERE record_id=? AND conflict=1)",
        )
        .bind(&dep)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| DATABASE)?;
        if conflict {
            return Err(BLOCKED.into());
        }
    }
    let f = &v.fields;
    match v.kind.as_str() {
        "account-link" => {
            account_type(
                conn,
                get(f, "id")?,
                &["bank", "wallet", "credit_card", "loan", "investment"],
            )
            .await?
        }
        "account-logo" => { account_type(conn, get(f, "account_id")?, &["cash"]).await?; }
        "facility" => {
            let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='loan' AND loan_type='personal_loan')").bind(get(f,"account_id")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !ok {
                return Err(BLOCKED.into());
            }
        }
        "contract" => {
            account_type(conn, get(f, "receiving_account_id")?, &["cash", "bank"]).await?;
            account_type(conn, get(f, "payment_account_id")?, &["cash", "bank"]).await?;
        }
        "installment" => {
            account_type(conn, get(f, "account_id")?, &["cash", "bank"]).await?;
            account_type(conn, get(f, "debt_account_id")?, &["credit_card", "loan"]).await?;
            if let Some(id) = optional(f, "purchase_transaction_id") {
                let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE id=? AND type='expense' AND account_id=?)").bind(id).bind(get(f,"debt_account_id")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
                if !ok {
                    return Err(BLOCKED.into());
                }
            }
        }
        "subscription" => {
            account_type(
                conn,
                get(f, "account_id")?,
                &["cash", "bank", "wallet", "credit_card"],
            )
            .await?
        }
        "payment-plan" => {
            account_type(conn, get(f, "account_id")?, &["cash", "bank", "wallet"]).await?;
            if let Some(id) = optional(f, "destination_account_id") {
                account_type(conn, id, &["loan", "credit_card"]).await?;
            }
        }
        "debt-amount" | "default-history" => {
            account_type(conn, get(f, "account_id")?, &["loan", "credit_card"]).await?
        }
        "card-limit" => {
            for r in &v.children["members"] {
                account_type(conn, get(r, "account_id")?, &["credit_card"]).await?;
            }
        }
        "settings" => {
            let local: Option<String> =
                sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|_| DATABASE)?;
            if local.as_deref() != optional(f, "currency") {
                let any: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM accounts) OR EXISTS(SELECT 1 FROM incomes)",
                )
                .fetch_one(&mut *conn)
                .await
                .map_err(|_| DATABASE)?;
                if any || local.is_some() {
                    return Err(BLOCKED.into());
                }
            }
        }
        "payoff" => {
            let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts a WHERE id=? AND type='loan' AND current_balance=0 AND is_archived=1 AND NOT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=a.id AND (remaining_principal<>0 OR needs_review=1)) AND NOT EXISTS(SELECT 1 FROM income_deductions WHERE debt_account_id=a.id))").bind(get(f,"account_id")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !ok {
                return Err(BLOCKED.into());
            }
        }
        "statement" => {
            account_type(conn, get(f, "account_id")?, &["credit_card"]).await?;
            let overlap:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM card_statements WHERE account_id=? AND id<>? AND start_date<=? AND end_date>=?)").bind(get(f,"account_id")?).bind(get(f,"id")?).bind(get(f,"end_date")?).bind(get(f,"start_date")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if overlap {
                return Err(BLOCKED.into());
            }
            let mut paid = 0i128;
            for r in &v.children["allocations"] {
                let found:Option<(i64,String,String,String)>=sqlx::query_as("SELECT t.amount,t.date,t.destination_account_id,a.type FROM transactions t JOIN accounts a ON a.id=t.account_id WHERE t.id=? AND t.type IN ('transfer','repayment')").bind(get(r,"transaction_id")?).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
                let (n, date, card, kind) = found.ok_or(BLOCKED)?;
                if date.as_str() <= get(f, "end_date")?
                    || card != get(f, "account_id")?
                    || !["cash", "bank", "wallet"].contains(&kind.as_str())
                {
                    return Err(BLOCKED.into());
                }
                paid += i128::from(n);
            }
            if paid > i128::from(number(f, "amount")?) {
                return Err(BLOCKED.into());
            }
            for r in &v.children["plan"] {
                account_type(conn, get(r, "account_id")?, &["cash", "bank", "wallet"]).await?;
            }
            for r in &v.children["installments"] {
                let row:Option<(String,String,i64)>=sqlx::query_as("SELECT debt_account_id,first_due_date,installment_count FROM installments WHERE id=?").bind(get(r,"installment_id")?).fetch_optional(&mut *conn).await.map_err(|_|DATABASE)?;
                let (card, first, count) = row.ok_or(BLOCKED)?;
                let date = get(r, "date")?;
                if card != get(f, "account_id")?
                    || date < get(f, "start_date")?
                    || date > get(f, "end_date")?
                    || !(0..count).any(|i| crate::card_billing::scheduled_date(&first, i) == date)
                {
                    return Err(BLOCKED.into());
                }
            }
        }
        "reconciliation" => {
            account_type(
                conn,
                get(f, "account_id")?,
                &["bank", "wallet", "credit_card", "loan"],
            )
            .await?;
            let existing =
                meta::current(conn, &format!("reconciliation:{}", get(f, "sync_id")?)).await?;
            if let Some(Value::Metadata(old)) = existing.value {
                let mut a = old.fields;
                a.remove("needs_review");
                let mut b = f.clone();
                b.remove("needs_review");
                if a != b || old.children != v.children {
                    return Err(BLOCKED.into());
                }
            }
        }
        "verification" => {
            let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE id=? AND (account_id=? OR destination_account_id=?))").bind(get(f,"transaction_id")?).bind(get(f,"account_id")?).bind(get(f,"account_id")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
            if !exists {
                return Err(BLOCKED.into());
            }
            account_type(
                conn,
                get(f, "account_id")?,
                &["bank", "wallet", "credit_card", "loan"],
            )
            .await?;
            if let Some(id) = optional(f, "reconciliation_id") {
                let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM reconciliations r JOIN reconciliation_entries e ON e.reconciliation_id=r.id WHERE r.sync_id=? AND r.account_id=? AND e.transaction_id=?)").bind(id).bind(get(f,"account_id")?).bind(get(f,"transaction_id")?).fetch_one(&mut *conn).await.map_err(|_|DATABASE)?;
                if !ok {
                    return Err(BLOCKED.into());
                }
            }
        }
        _ => {}
    }
    Ok(())
}
pub async fn before_delete(
    conn: &mut SqliteConnection,
    s: &Spec,
    keys: &[&str],
    _confirmed: bool,
) -> Result<(), String> {
    if s.kind == "settings" || s.kind == "reconciliation" {
        return Err(BLOCKED.into());
    }
    guard_references(conn, &meta::key(s.kind, keys)).await
}
pub async fn guard_references(conn: &mut SqliteConnection, target: &str) -> Result<(), String> {
    // Preserve referenced/competing planner and statement edits during cascades.
    let records:Vec<String>=sqlx::query_scalar("SELECT record_id FROM cloud_sync_shadow WHERE record_id LIKE 'metadata:%' AND json_type(payload,'$.value')<>'null' UNION SELECT record_id FROM cloud_sync_dirty WHERE record_id LIKE 'metadata:%'").fetch_all(&mut *conn).await.map_err(|_|DATABASE)?;
    for record in records {
        if record == target {
            continue;
        }
        if let Some(Value::Metadata(local)) =
            meta::current(conn, record.strip_prefix("metadata:").ok_or(UNSAFE)?)
                .await?
                .value
        {
            if dependencies(conn, &local)
                .await?
                .iter()
                .any(|dep| dep == target)
            {
                return Err(BLOCKED.into());
            }
        }
    }
    Ok(())
}
pub async fn install_logo(conn: &mut SqliteConnection, v: &Metadata) -> Result<(), String> {
    let f = &v.fields;
    let hash = get(f, "content_hash")?;
    sqlx::query("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES(?,?,'image/png',?,?,?) ON CONFLICT(content_hash) DO NOTHING").bind(format!("sync-{hash}")).bind(hash).bind(number(f,"width")?).bind(number(f,"height")?).bind(STANDARD.decode(get(f,"data")?).map_err(|_|UNSAFE)?).execute(conn).await.map_err(|_|BLOCKED)?;
    Ok(())
}

pub fn unchanged_seed(record: &Record) -> bool {
    let Ok(seeds) =
        serde_json::from_str::<Vec<Metadata>>(include_str!("../data/sync-metadata-seeds.json"))
    else {
        return false;
    };
    seeds.iter().any(|v| v.record() == *record)
}
