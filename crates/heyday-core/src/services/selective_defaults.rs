use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, Deserialize, sqlx::FromRow, PartialEq, Debug)]
pub struct Exclusion {
    pub account_id: String,
    pub start_month: String,
    pub end_month: Option<String>,
}
pub async fn periods(conn: &mut SqliteConnection) -> Result<Vec<Exclusion>, sqlx::Error> {
    sqlx::query_as("SELECT account_id,start_month,end_month FROM selective_defaults ORDER BY account_id,start_month").fetch_all(conn).await
}
#[derive(Serialize)]
pub struct Status {
    pub periods: Vec<Exclusion>,
    pub period_start_day: i64,
    pub linked_items: Vec<LinkedItem>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct LinkedItem {
    pub kind: String,
    pub name: String,
}
pub async fn snapshot(conn: &mut SqliteConnection, account_id: &str) -> Result<Status, String> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card'))",
    )
    .bind(account_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    if !valid {
        return Err("Choose a loan or credit card account.".into());
    }
    let periods = periods(conn)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|p| p.account_id == account_id)
        .collect();
    let period_start_day = sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id=1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let linked_items = sqlx::query_as("SELECT 'Loan schedule' AS kind,name FROM accounts WHERE id=?1 AND type='loan' AND monthly_installment IS NOT NULL UNION ALL SELECT 'Contract',name FROM loan_contracts WHERE account_id=?1 UNION ALL SELECT 'Installments',name FROM installments WHERE debt_account_id=?1 UNION ALL SELECT 'Payment plan',name FROM payment_plans WHERE destination_account_id=?1 UNION ALL SELECT 'Statement payment plan',s.end_date FROM card_payment_plans p JOIN card_statements s ON s.id=p.statement_id WHERE s.account_id=?1 UNION ALL SELECT 'Saved cycle override',month FROM planner_debt_amounts WHERE account_id=?1 UNION ALL SELECT 'Saved cycle override',p.month FROM planner_installment_amounts p JOIN installments i ON i.id=p.installment_id WHERE i.debt_account_id=?1")
        .bind(account_id).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?;
    Ok(Status {
        periods,
        period_start_day,
        linked_items,
    })
}
pub async fn get_selective_default(
    pool: &SqlitePool,
    account_id: String,
) -> Result<Status, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = snapshot(&mut tx, &account_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub account_id: String,
    pub action: String,
    pub month: String,
    pub expected_periods: Vec<Exclusion>,
    pub confirmed: bool,
}
pub fn valid_month(month: &str) -> bool {
    month.len() == 7 && crate::transactions::valid_date(&format!("{month}-01"))
}
pub async fn save(pool: &SqlitePool, input: Change) -> Result<(), String> {
    if !input.confirmed {
        return Err("Confirm the Selective Default change.".into());
    }
    if !valid_month(&input.month) {
        return Err("Choose a valid cycle month.".into());
    }
    if !["exclude", "resume"].contains(&input.action.as_str()) {
        return Err("Choose exclusion or resumption.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card') AND is_archived=0)").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
    if !valid {
        return Err("Choose an active loan or credit card account.".into());
    }
    let history: Vec<_> = periods(&mut tx)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|p| p.account_id == input.account_id)
        .collect();
    if history != input.expected_periods {
        return Err("Selective Default changed. Reload before saving.".into());
    }
    let open = history.iter().find(|p| p.end_month.is_none());
    if input.action == "exclude" {
        if open.is_some()
            || history.iter().any(|p| {
                p.end_month.as_ref().is_some_and(|end| end > &input.month)
                    || p.start_month == input.month
            })
        {
            return Err("Choose a start cycle after the previous exclusion period.".into());
        }
        sqlx::query("INSERT INTO selective_defaults(account_id,start_month) VALUES(?,?)")
            .bind(&input.account_id)
            .bind(&input.month)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let open = open.ok_or("This account has no open exclusion period.")?;
        if input.month < open.start_month {
            return Err("Resume from the start cycle or a later cycle.".into());
        }
        sqlx::query("UPDATE selective_defaults SET end_month=? WHERE account_id=? AND start_month=? AND end_month IS NULL")
            .bind(&input.month).bind(&input.account_id).bind(&open.start_month).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn save_selective_default(pool: &SqlitePool, input: Change) -> Result<(), String> {
    save(pool, input).await
}
