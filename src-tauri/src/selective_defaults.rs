//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::selective_defaults as service;
pub use heyday_core::services::selective_defaults::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_selective_default(pool: tauri::State<'_,SqlitePool>, account_id: String) -> Result<Status,String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_selective_default(pool.inner(), account_id).await
}

#[tauri::command]
pub async fn save_selective_default(pool: tauri::State<'_,SqlitePool>, input: Change) -> Result<(),String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_selective_default(pool.inner(), input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn database() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
