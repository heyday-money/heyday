use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, Deserialize, sqlx::FromRow, PartialEq, Debug)]
pub struct Exclusion {
    account_id: String,
    start_month: String,
    end_month: Option<String>,
}
pub async fn periods(conn: &mut SqliteConnection) -> Result<Vec<Exclusion>, sqlx::Error> {
    sqlx::query_as("SELECT account_id,start_month,end_month FROM selective_defaults ORDER BY account_id,start_month").fetch_all(conn).await
}
#[derive(Serialize)]
pub struct Status {
    periods: Vec<Exclusion>,
    period_start_day: i64,
    linked_items: Vec<LinkedItem>,
}
#[derive(Serialize, sqlx::FromRow)]
struct LinkedItem { kind: String, name: String }
async fn snapshot(conn: &mut SqliteConnection, account_id: &str) -> Result<Status, String> {
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card'))").bind(account_id).fetch_one(&mut *conn).await.map_err(|e|e.to_string())?;
    if !valid { return Err("Choose a loan or credit card account.".into()); }
    let periods = periods(conn).await.map_err(|e|e.to_string())?.into_iter().filter(|p|p.account_id==account_id).collect();
    let period_start_day = sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id=1").fetch_one(&mut *conn).await.map_err(|e|e.to_string())?;
    let linked_items = sqlx::query_as("SELECT 'Loan schedule' AS kind,name FROM accounts WHERE id=?1 AND type='loan' AND monthly_installment IS NOT NULL UNION ALL SELECT 'Contract',name FROM loan_contracts WHERE account_id=?1 UNION ALL SELECT 'Installments',name FROM installments WHERE debt_account_id=?1 UNION ALL SELECT 'Payment plan',name FROM payment_plans WHERE destination_account_id=?1 UNION ALL SELECT 'Statement payment plan',s.end_date FROM card_payment_plans p JOIN card_statements s ON s.id=p.statement_id WHERE s.account_id=?1 UNION ALL SELECT 'Saved cycle override',month FROM planner_debt_amounts WHERE account_id=?1 UNION ALL SELECT 'Saved cycle override',p.month FROM planner_installment_amounts p JOIN installments i ON i.id=p.installment_id WHERE i.debt_account_id=?1")
        .bind(account_id).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?;
    Ok(Status { periods, period_start_day, linked_items })
}
#[tauri::command]
pub async fn get_selective_default(pool: tauri::State<'_,SqlitePool>, account_id: String) -> Result<Status,String> {
    let _operation = crate::backups::operation()?;
    let mut tx = pool.begin().await.map_err(|e|e.to_string())?;
    let result = snapshot(&mut tx,&account_id).await?;
    tx.commit().await.map_err(|e|e.to_string())?;
    Ok(result)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    account_id: String,
    action: String,
    month: String,
    expected_periods: Vec<Exclusion>,
    confirmed: bool,
}
fn valid_month(month: &str) -> bool {
    month.len()==7 && crate::transactions::valid_date(&format!("{month}-01"))
}
async fn save(pool: &SqlitePool, input: Change) -> Result<(),String> {
    if !input.confirmed { return Err("Confirm the Selective Default change.".into()); }
    if !valid_month(&input.month) { return Err("Choose a valid cycle month.".into()); }
    if !["exclude","resume"].contains(&input.action.as_str()) { return Err("Choose exclusion or resumption.".into()); }
    let mut tx = pool.begin().await.map_err(|e|e.to_string())?;
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type IN ('loan','credit_card') AND is_archived=0)").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
    if !valid { return Err("Choose an active loan or credit card account.".into()); }
    let history: Vec<_> = periods(&mut tx).await.map_err(|e|e.to_string())?.into_iter().filter(|p|p.account_id==input.account_id).collect();
    if history != input.expected_periods { return Err("Selective Default changed. Reload before saving.".into()); }
    let open = history.iter().find(|p|p.end_month.is_none());
    if input.action=="exclude" {
        if open.is_some() || history.iter().any(|p|p.end_month.as_ref().is_some_and(|end|end>&input.month) || p.start_month==input.month) {
            return Err("Choose a start cycle after the previous exclusion period.".into());
        }
        sqlx::query("INSERT INTO selective_defaults(account_id,start_month) VALUES(?,?)").bind(&input.account_id).bind(&input.month).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    } else {
        let open = open.ok_or("This account has no open exclusion period.")?;
        if input.month<open.start_month { return Err("Resume from the start cycle or a later cycle.".into()); }
        sqlx::query("UPDATE selective_defaults SET end_month=? WHERE account_id=? AND start_month=? AND end_month IS NULL")
            .bind(&input.month).bind(&input.account_id).bind(&open.start_month).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    }
    tx.commit().await.map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn save_selective_default(pool: tauri::State<'_,SqlitePool>, input: Change) -> Result<(),String> {
    let _operation = crate::backups::operation()?;
    save(pool.inner(),input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn database() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance,monthly_installment) VALUES('loan','Loan','loan','mortgage',10000,10000,100),('card','Card','credit_card',NULL,2000,2000,NULL),('bank','Bank','bank',NULL,50000,50000,NULL); INSERT INTO planner_debt_amounts VALUES('loan','2026-10',999); INSERT INTO payment_plans(id,name,type,account_id,destination_account_id,amount,date) VALUES('plan','Loan payment','repayment','bank','loan',123,'2026-10-25');").execute(&pool).await.unwrap();
        pool
    }
    async fn change(pool: &SqlitePool, id: &str, action: &str, month: &str) -> Change {
        Change { account_id:id.into(), action:action.into(), month:month.into(), confirmed:true,
            expected_periods: periods(&mut *pool.acquire().await.unwrap()).await.unwrap().into_iter().filter(|p|p.account_id==id).collect() }
    }
    #[tokio::test]
    async fn exclude_resume_preserve_balances_plans_and_history_and_reject_stale_changes() {
        let pool=database().await;
        let before: Vec<(String,i64)> = sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id").fetch_all(&pool).await.unwrap();
        let stale=change(&pool,"loan","exclude","2026-10").await;
        save(&pool,change(&pool,"loan","exclude","2026-10").await).await.unwrap();
        assert!(save(&pool,stale).await.unwrap_err().contains("Reload"));
        assert!(save(&pool,change(&pool,"loan","resume","2026-09").await).await.is_err());
        save(&pool,change(&pool,"loan","resume","2027-01").await).await.unwrap();
        assert!(save(&pool,change(&pool,"loan","exclude","2026-12").await).await.is_err());
        save(&pool,change(&pool,"loan","exclude","2027-02").await).await.unwrap();
        save(&pool,change(&pool,"card","exclude","2026-11").await).await.unwrap();
        let status=snapshot(&mut *pool.acquire().await.unwrap(),"loan").await.unwrap();
        assert_eq!(status.periods.len(),2);
        assert_eq!(status.periods[0].end_month.as_deref(),Some("2027-01"));
        assert!(status.linked_items.iter().any(|i|i.kind=="Saved cycle override" && i.name=="2026-10"));
        assert!(status.linked_items.iter().any(|i|i.name=="Loan payment"));
        let after: Vec<(String,i64)> = sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(before,after);
        let amount: i64=sqlx::query_scalar("SELECT amount FROM planner_debt_amounts").fetch_one(&pool).await.unwrap();
        assert_eq!(amount,999);
        let count: i64=sqlx::query_scalar("SELECT COUNT(*) FROM transactions").fetch_one(&pool).await.unwrap();
        assert_eq!(count,0);
    }
    #[tokio::test]
    async fn rejects_invalid_accounts_cycles_actions_and_unconfirmed_changes() {
        let pool=database().await;
        for id in ["bank","missing"] { assert!(save(&pool,change(&pool,id,"exclude","2026-10").await).await.is_err()); }
        for month in ["0000-01","2026-00","2026-13","2026-1","2026-10-01","invalid"] { assert!(save(&pool,change(&pool,"loan","exclude",month).await).await.is_err()); }
        assert!(save(&pool,change(&pool,"loan","remove","2026-10").await).await.is_err());
        let mut draft=change(&pool,"loan","exclude","2026-10").await; draft.confirmed=false;
        assert!(save(&pool,draft).await.is_err());
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='loan'").execute(&pool).await.unwrap();
        assert!(save(&pool,change(&pool,"loan","exclude","2026-10").await).await.is_err());
        assert!(periods(&mut *pool.acquire().await.unwrap()).await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn same_cycle_resumption_cancels_future_exclusion_without_deleting_history() {
        let pool=database().await;
        save(&pool,change(&pool,"loan","exclude","2030-12").await).await.unwrap();
        save(&pool,change(&pool,"loan","resume","2030-12").await).await.unwrap();
        let history=periods(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(history[0].end_month.as_deref(),Some("2030-12"));
        assert!(save(&pool,change(&pool,"loan","exclude","2030-12").await).await.is_err());
    }
}
