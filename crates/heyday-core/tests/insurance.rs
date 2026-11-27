use heyday_core::{
    database::MIGRATOR,
    services::{insurance as ins, reset, tracking},
};
use serde_json::json;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
async fn db() -> SqlitePool {
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&p).await.unwrap();
    sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000000,10000000),('card','Card','credit_card',0,0);").execute(&p).await.unwrap();
    p
}
fn input() -> ins::Save {
    serde_json::from_value(json!({"id":"policy","expected_revision":null,"currency":"THB","details":{"name":"Car cover","insurer":"Insurer","policy_type":"motor","number_suffix":"1234","coverage_start":"2024-01-01","coverage_end":null,"renewal_date":"2027-01-01","coverage_amount":"70000000","deductible":"500000","notes":"","status":"active","status_date":"2024-01-01","asset_ids":[],"cash_value_id":null,"replaces":null},"schedule":{"id":"schedule","effective_from":"2024-01-01","first_date":"2024-01-31","end_date":null,"frequency":"monthly","amount":"100000","account_id":"bank","category_id":"planner-expense-6"}})).unwrap()
}
fn pay(id: &str, revision: &str) -> ins::RecordPayment {
    ins::RecordPayment {
        id: id.into(),
        policy_id: "policy".into(),
        expected_revision: revision.into(),
        schedule_id: "schedule".into(),
        due_date: "2024-02-29".into(),
        date: "2024-02-29".into(),
        notes: "Paid".into(),
    }
}
async fn balance(p: &SqlitePool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=?")
        .bind(id)
        .fetch_one(p)
        .await
        .unwrap()
}
#[test]
fn anchors_clamp_independently_and_once_is_not_recurring() {
    let mut s = input().schedule.unwrap();
    assert!(ins::scheduled(&s, "2024-02-29"));
    assert!(ins::scheduled(&s, "2024-03-31"));
    assert!(!ins::scheduled(&s, "2024-03-29"));
    s.first_date = "2024-02-29".into();
    s.frequency = "yearly".into();
    assert!(ins::scheduled(&s, "2025-02-28"));
    assert!(ins::scheduled(&s, "2028-02-29"));
    s.frequency = "once".into();
    assert!(ins::scheduled(&s, "2024-02-29"));
    assert!(!ins::scheduled(&s, "2025-02-28"));
    s.frequency = "quarterly".into();
    assert!(ins::scheduled(&s, "2024-05-29"));
    assert!(!ins::scheduled(&s, "2024-03-29"));
}
#[tokio::test]
async fn premiums_are_exact_idempotent_expenses_and_reversible() {
    let p = db().await;
    ins::save(&p, input()).await.unwrap();
    assert_eq!(balance(&p, "bank").await, 10000000);
    ins::pay(&p, pay("payment", "1")).await.unwrap();
    ins::pay(&p, pay("payment", "1")).await.unwrap();
    assert_eq!(balance(&p, "bank").await, 9900000);
    assert!(ins::pay(&p, pay("duplicate", "2")).await.is_err());
    assert_eq!(balance(&p, "bank").await, 9900000);
    assert!(
        heyday_core::services::transactions::remove_confirmed(&p, "payment", true)
            .await
            .is_err()
    );
    let reverse = || ins::Reverse {
        id: "reverse".into(),
        policy_id: "policy".into(),
        payment_id: "payment".into(),
        expected_revision: "2".into(),
    };
    ins::reverse(&p, reverse()).await.unwrap();
    ins::reverse(&p, reverse()).await.unwrap();
    assert_eq!(balance(&p, "bank").await, 10000000);
    ins::pay(&p, pay("replacement", "3")).await.unwrap();
    ins::validate_database(&mut *p.acquire().await.unwrap())
        .await
        .unwrap();
}
#[tokio::test]
async fn insurance_never_changes_asset_value_and_cash_value_link_is_unique() {
    let p = db().await;
    let asset=tracking::create(&p,serde_json::from_value(json!({"name":"Cash value","subtype":"insurance_cash_value","currency":"THB","opening_value":"60000000","opening_date":"2024-01-01"})).unwrap()).await.unwrap();
    let mut i = input();
    i.details.cash_value_id = Some(asset.id.clone());
    ins::save(&p, i).await.unwrap();
    ins::pay(&p, pay("payment", "1")).await.unwrap();
    assert_eq!(
        tracking::get(&p, &asset.id).await.unwrap().current_value,
        "60000000"
    );
    let mut duplicate = input();
    duplicate.id = "second-policy".into();
    duplicate.details.cash_value_id = Some(asset.id.clone());
    assert!(ins::save(&p, duplicate).await.is_err());
    assert_eq!(ins::list(&p).await.unwrap().len(), 1);
    reset::clear_data(&p, "DELETE ALL DATA").await.unwrap();
    assert!(ins::list(&p).await.unwrap().is_empty());
}
#[tokio::test]
async fn stale_schedule_edits_and_cancelled_occurrences_do_not_post() {
    let p = db().await;
    let mut i = input();
    i.details.status = "cancelled".into();
    i.details.status_date = "2024-02-01".into();
    ins::save(&p, i).await.unwrap();
    assert!(ins::pay(&p, pay("payment", "1")).await.is_err());
    assert_eq!(balance(&p, "bank").await, 10000000);
    assert!(ins::save(&p, input()).await.is_err());
    let mut edit = input();
    edit.expected_revision = Some("1".into());
    edit.schedule = None;
    edit.details.status = "archived".into();
    ins::save(&p, edit).await.unwrap();
    assert!(ins::pay(&p, pay("payment", "2")).await.is_err());
}
#[tokio::test]
async fn reconciled_premiums_cannot_be_reversed_and_bad_cash_rolls_back() {
    let p = db().await;
    ins::save(&p, input()).await.unwrap();
    ins::pay(&p, pay("payment", "1")).await.unwrap();
    sqlx::raw_sql("INSERT INTO reconciliations(id,account_id,completed_at,confirmed_balance,opening_balance) VALUES(1,'bank','2024-02-29T12:00:00.000Z',9900000,10000000); UPDATE transaction_verifications SET status='reconciled',reconciliation_id=1 WHERE transaction_id='payment' AND account_id='bank';").execute(&p).await.unwrap();
    assert!(ins::reverse(
        &p,
        ins::Reverse {
            id: "undo".into(),
            policy_id: "policy".into(),
            payment_id: "payment".into(),
            expected_revision: "2".into()
        }
    )
    .await
    .is_err());
    assert_eq!(balance(&p, "bank").await, 9900000);
    sqlx::query("UPDATE accounts SET current_balance=-9223372036854775808 WHERE id='bank'")
        .execute(&p)
        .await
        .unwrap();
    let mut next = pay("overflow", "2");
    next.due_date = "2024-03-31".into();
    assert!(ins::pay(&p, next).await.is_err());
    assert_eq!(ins::list(&p).await.unwrap()[0].revision, "2");
}
#[tokio::test]
async fn replacement_is_explicit_unique_and_preserves_source() {
    let p = db().await;
    sqlx::query("INSERT INTO subscriptions(id,name,account_id,amount,frequency,first_billing_date,is_active) VALUES('sub','Cover','bank',100000,'monthly','2024-01-31',1)").execute(&p).await.unwrap();
    let mut i = input();
    i.details.replaces = Some(ins::Source {
        kind: "subscription".into(),
        id: "sub".into(),
    });
    ins::save(&p, i).await.unwrap();
    let mut other = input();
    other.id = "other".into();
    other.details.replaces = Some(ins::Source {
        kind: "subscription".into(),
        id: "sub".into(),
    });
    assert!(ins::save(&p, other).await.is_err());
    assert!(heyday_core::services::subscriptions::remove(&p, "sub")
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subscriptions")
            .fetch_one(&p)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn schedule_versions_preserve_payments_and_corrupt_backups_are_rejected() {
    let p = db().await;
    let original = ins::save(&p, input()).await.unwrap();
    ins::pay(&p, pay("receipt", "1")).await.unwrap();
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&p)
        .await
        .unwrap();
    let mut edit = input();
    edit.expected_revision = Some("2".into());
    let s = edit.schedule.as_mut().unwrap();
    s.id = "new-schedule".into();
    s.effective_from = today.clone();
    s.amount = "120000".into();
    let saved = ins::save(&p, edit).await.unwrap();
    assert_eq!(saved.schedules[0], original.schedules[0]);
    assert_eq!(saved.payments.len(), 1);
    assert!(!saved.payable(&saved.schedules[0], &today));
    ins::validate_database(&mut *p.acquire().await.unwrap())
        .await
        .unwrap();
    sqlx::query("DELETE FROM insurance_links WHERE policy_id='policy'")
        .execute(&p)
        .await
        .unwrap();
    assert!(ins::validate_database(&mut *p.acquire().await.unwrap())
        .await
        .is_err());
}
#[tokio::test]
async fn credit_card_premium_increases_debt_once_and_forecasts_do_not_post() {
    let p = db().await;
    let mut i = input();
    i.schedule.as_mut().unwrap().account_id = "card".into();
    ins::save(&p, i).await.unwrap();
    assert_eq!(balance(&p, "card").await, 0);
    ins::pay(&p, pay("card-payment", "1")).await.unwrap();
    assert_eq!(balance(&p, "card").await, 100000);
    let mut future = pay("future", "2");
    future.date = "2999-01-01".into();
    future.due_date = "2024-03-31".into();
    assert!(ins::pay(&p, future).await.is_err());
    assert_eq!(balance(&p, "card").await, 100000);
}
