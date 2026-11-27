use heyday_core::{
    database::{settings, MIGRATOR},
    domain::tracking::{TrackingDraft, TrackingSubtype},
    services::{
        reset,
        tracking::{self, EventAction, EventInput},
        transactions,
    },
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::borrow::Cow;
async fn db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    settings::save_currency(&pool, "THB").await.unwrap();
    pool
}
fn draft() -> TrackingDraft {
    serde_json::from_value(serde_json::json!({"name":"Car","subtype":"vehicle","currency":"THB","opening_value":"60000000","opening_date":"2024-01-01"})).unwrap()
}
fn event(id: &str, account: &str, revision: &str, value: &str) -> EventInput {
    EventInput {
        cash_account_id: None,
        id: id.into(),
        account_id: account.into(),
        expected_revision: revision.into(),
        currency: "THB".into(),
        date: "2024-02-01".into(),
        notes: "Appraisal".into(),
        action: EventAction::Value {
            value: value.into(),
        },
    }
}
#[tokio::test]
async fn upgrade_preserves_accounts_children_triggers_and_reopens_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("upgrade.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    let old = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|m| m.version < 12)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run(&pool).await.unwrap();
    sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',9007199254740993,9007199254740993); INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('tx','income','bank',1,'2024-01-01','Existing');").execute(&pool).await.unwrap();
    let before: Vec<(String, String)> =
        sqlx::query_as("SELECT name,sql FROM sqlite_schema WHERE type='trigger' ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    for (name, mut sql) in before {
        if name.starts_with("ledger_sync_account_") {
            let row = if name.ends_with("delete") {
                "OLD"
            } else {
                "NEW"
            };
            sql = sql.replacen(
                "WHEN EXISTS(",
                &format!("WHEN {row}.type<>'tracking' AND EXISTS("),
                1,
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?"
            )
            .bind(name)
            .fetch_one(&pool)
            .await
            .unwrap(),
            sql
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions WHERE id='tx'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        9007199254740993
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    pool.close().await;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA writable_schema")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        tracking::create(&pool, draft())
            .await
            .unwrap()
            .current_value,
        "60000000"
    );
}
#[tokio::test]
async fn valuations_are_atomic_idempotent_and_reversible_with_stale_checks() {
    let pool = db().await;
    let asset = tracking::create(&pool, draft()).await.unwrap();
    let valued = tracking::record(&pool, event("first", &asset.id, "0", "55000000"))
        .await
        .unwrap();
    assert_eq!(
        (
            valued.current_value.as_str(),
            valued.revision.as_str(),
            valued.latest_valuation_date.as_str()
        ),
        ("55000000", "1", "2024-02-01")
    );
    assert_eq!(
        tracking::record(&pool, event("first", &asset.id, "0", "55000000"))
            .await
            .unwrap()
            .revision,
        "1"
    );
    assert!(
        tracking::record(&pool, event("first", &asset.id, "0", "54000000"))
            .await
            .is_err()
    );
    assert!(
        tracking::record(&pool, event("stale", &asset.id, "0", "54000000"))
            .await
            .is_err()
    );
    let mut backdated = event("backdated", &asset.id, "1", "54000000");
    backdated.date = "2024-01-15".into();
    assert!(tracking::record(&pool, backdated).await.is_err());
    tracking::record(&pool, event("second", &asset.id, "1", "52000000"))
        .await
        .unwrap();
    let mut reverse = event("reverse", &asset.id, "2", "0");
    reverse.action = EventAction::Reverse {
        event_id: "first".into(),
    };
    assert!(tracking::record(&pool, reverse).await.is_err());
    let mut reverse = event("reverse", &asset.id, "2", "0");
    reverse.action = EventAction::Reverse {
        event_id: "second".into(),
    };
    assert_eq!(
        tracking::record(&pool, reverse)
            .await
            .unwrap()
            .current_value,
        "55000000"
    );
    let mut reverse = event("reverse-first", &asset.id, "3", "0");
    reverse.action = EventAction::Reverse {
        event_id: "first".into(),
    };
    let state = tracking::record(&pool, reverse).await.unwrap();
    assert_eq!(
        (
            state.current_value.as_str(),
            state.latest_valuation_date.as_str()
        ),
        ("60000000", "2024-01-01")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::query("UPDATE tracking_events SET notes='tampered'")
        .execute(&pool)
        .await
        .is_err());
}
#[tokio::test]
async fn write_failure_rolls_back_event_and_balance() {
    let pool = db().await;
    let asset = tracking::create(&pool, draft()).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_projection BEFORE UPDATE OF current_balance ON accounts BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").execute(&pool).await.unwrap();
    assert!(
        tracking::record(&pool, event("failed", &asset.id, "0", "1"))
            .await
            .is_err()
    );
    let state = tracking::get(&pool, &asset.id).await.unwrap();
    assert_eq!(
        (state.current_value.as_str(), state.revision.as_str()),
        ("60000000", "0")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracking_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
async fn bind(pool: &SqlitePool) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
    sqlx::query("INSERT INTO cloud_sync_binding(id,origin,account_id,dataset_id,key_id,hash) VALUES(1,'https://example.test','user','dataset','key','hash')").execute(pool).await
}
#[tokio::test]
async fn incomplete_sync_consent_and_ordinary_transactions_are_blocked() {
    let pool = db().await;
    bind(&pool).await.unwrap();
    assert!(tracking::create(&pool, draft()).await.is_err());
    sqlx::query("DELETE FROM cloud_sync_binding")
        .execute(&pool)
        .await
        .unwrap();
    let asset = tracking::create(&pool, draft()).await.unwrap();
    bind(&pool).await.unwrap();
    let input=serde_json::from_value(serde_json::json!({"type":"income","account_id":asset.id,"amount":"100","date":"2024-02-01","description":"invalid","currency":"THB"})).unwrap();
    assert!(transactions::create_transaction(&pool, input)
        .await
        .is_err());
    assert_eq!(
        tracking::get(&pool, &asset.id).await.unwrap().current_value,
        "60000000"
    );
    sqlx::query("DELETE FROM cloud_sync_binding")
        .execute(&pool)
        .await
        .unwrap();
    let mut receivable = draft();
    receivable.subtype = TrackingSubtype::Receivable;
    let receivable = tracking::create(&pool, receivable).await.unwrap();
    assert!(
        tracking::record(&pool, event("value", &receivable.id, "0", "1"))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn metadata_edits_preserve_opening_and_check_references_and_archives() {
    let pool = db().await;
    let asset = tracking::create(&pool, draft()).await.unwrap();
    let mut input = draft();
    input.name = "Renamed".into();
    input.linked_loan_id = Some("missing".into());
    assert!(tracking::edit(&pool, &asset.id, "0", input.clone())
        .await
        .is_err());
    input.linked_loan_id = None;
    input.logo_asset_id = Some("missing".into());
    assert!(tracking::edit(&pool, &asset.id, "0", input.clone())
        .await
        .is_err());
    input.logo_asset_id = None;
    assert_eq!(
        tracking::edit(&pool, &asset.id, "0", input.clone())
            .await
            .unwrap()
            .revision,
        "1"
    );
    assert!(tracking::edit(&pool, &asset.id, "0", input).await.is_err());
    let mut input = draft();
    input.opening_value = "1".into();
    assert!(tracking::edit(&pool, &asset.id, "1", input).await.is_err());
    sqlx::query("UPDATE accounts SET is_archived=1 WHERE id=?")
        .bind(&asset.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        tracking::record(&pool, event("archived", &asset.id, "1", "1"))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn reset_removes_history_and_reversal_references_without_foreign_key_damage() {
    let pool = db().await;
    let asset = tracking::create(&pool, draft()).await.unwrap();
    tracking::record(&pool, event("first", &asset.id, "0", "55000000"))
        .await
        .unwrap();
    let mut reverse = event("reverse", &asset.id, "1", "0");
    reverse.action = EventAction::Reverse {
        event_id: "first".into(),
    };
    tracking::record(&pool, reverse).await.unwrap();
    reset::clear_data(&pool, "DELETE ALL DATA").await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracking_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn restore_validation_rejects_projection_and_history_corruption() {
    let pool = db().await;
    let asset = tracking::create(&pool, draft()).await.unwrap();
    tracking::record(&pool, event("valuation", &asset.id, "0", "55000000"))
        .await
        .unwrap();
    tracking::validate_database(&mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    sqlx::query("UPDATE accounts SET current_balance=1 WHERE id=?")
        .bind(&asset.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        tracking::validate_database(&mut pool.acquire().await.unwrap())
            .await
            .is_err()
    );
    sqlx::query("UPDATE accounts SET current_balance=55000000 WHERE id=?")
        .bind(&asset.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "DROP TRIGGER tracking_event_immutable; UPDATE tracking_events SET date='2024-02-31';",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        tracking::validate_database(&mut pool.acquire().await.unwrap())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cash_actions_rollback_overpayment_and_preserve_reconciled_history() {
    let pool = db().await;
    sqlx::raw_sql("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',2000000,2000000);").execute(&pool).await.unwrap();
    let mut d = draft();
    d.subtype = TrackingSubtype::Receivable;
    d.opening_value = "0".into();
    let asset = tracking::create(&pool, d).await.unwrap();
    let mut e = event("lend", &asset.id, "0", "0");
    e.action = EventAction::Lend {
        principal: "1000000".into(),
    };
    e.cash_account_id = Some("bank".into());
    tracking::record(&pool, e).await.unwrap();
    let mut e = event("overpay", &asset.id, "1", "0");
    e.action = EventAction::Receive {
        principal: "1000001".into(),
        interest: "5000".into(),
    };
    e.cash_account_id = Some("bank".into());
    assert!(tracking::record(&pool, e).await.is_err());
    assert_eq!(tracking::get(&pool, &asset.id).await.unwrap().revision, "1");
    let mut e = event("receive", &asset.id, "1", "0");
    e.action = EventAction::Receive {
        principal: "200000".into(),
        interest: "5000".into(),
    };
    e.cash_account_id = Some("bank".into());
    assert_eq!(
        tracking::record(&pool, e).await.unwrap().current_value,
        "800000"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1205000
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(amount) FROM transactions WHERE type='income'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        5000
    );
    let mut e = event("writeoff", &asset.id, "2", "0");
    e.action = EventAction::WriteOff {
        principal: "100000".into(),
    };
    assert_eq!(
        tracking::record(&pool, e).await.unwrap().current_value,
        "700000"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions WHERE type='expense'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut e = event("undo-loss", &asset.id, "3", "0");
    e.action = EventAction::Reverse {
        event_id: "writeoff".into(),
    };
    tracking::record(&pool, e).await.unwrap();
    sqlx::query("INSERT INTO reconciliations(id,account_id,completed_at,confirmed_balance,opening_balance) VALUES(1,'bank','2024-02-01T12:00:00.000Z',1205000,2000000)").execute(&pool).await.unwrap();
    sqlx::query("UPDATE transaction_verifications SET status='reconciled',reconciliation_id=1 WHERE transaction_id='receive-principal' AND account_id='bank'").execute(&pool).await.unwrap();
    let mut e = event("undo-receive", &asset.id, "4", "0");
    e.action = EventAction::Reverse {
        event_id: "receive".into(),
    };
    assert!(tracking::record(&pool, e)
        .await
        .unwrap_err()
        .contains("reconciliation"));
    assert_eq!(tracking::get(&pool, &asset.id).await.unwrap().revision, "4");
    tracking::validate_database(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn disposal_reversal_and_cash_overflow_are_atomic() {
    let pool = db().await;
    sqlx::raw_sql("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',9223372036854775807,9223372036854775807);").execute(&pool).await.unwrap();
    let asset = tracking::create(&pool, draft()).await.unwrap();
    let mut e = event("sale", &asset.id, "0", "0");
    e.action = EventAction::Sell {
        proceeds: "65000000".into(),
    };
    e.cash_account_id = Some("bank".into());
    assert!(tracking::record(&pool, e).await.is_err());
    assert_eq!(tracking::get(&pool, &asset.id).await.unwrap().revision, "0");
    sqlx::query("UPDATE accounts SET current_balance=0,opening_balance=0 WHERE id='bank'")
        .execute(&pool)
        .await
        .unwrap();
    let mut e = event("sale", &asset.id, "0", "0");
    e.action = EventAction::Sell {
        proceeds: "65000000".into(),
    };
    e.cash_account_id = Some("bank".into());
    assert_eq!(tracking::record(&pool, e).await.unwrap().current_value, "0");
    assert!(
        tracking::record(&pool, event("resurrect", &asset.id, "1", "500"))
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions WHERE type='income'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut e = event("undo-sale", &asset.id, "1", "0");
    e.action = EventAction::Reverse {
        event_id: "sale".into(),
    };
    assert_eq!(
        tracking::record(&pool, e).await.unwrap().current_value,
        "60000000"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    tracking::validate_database(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn valuation_milestone_upgrades_with_reversal_history_intact() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|m| m.version <= 12)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('car','Car','tracking',60000000,60000000); INSERT INTO tracking_accounts(account_id,subtype,opening_date,revision) VALUES('car','vehicle','2024-01-01',2); INSERT INTO tracking_events(id,account_id,revision,date,kind,previous_value,next_value) VALUES('value','car',1,'2024-02-01','value',60000000,55000000); INSERT INTO tracking_events(id,account_id,revision,date,kind,previous_value,next_value,reverses_id) VALUES('undo','car',2,'2024-02-01','reversal',55000000,60000000,'value');").execute(&pool).await.unwrap();
    tracking::validate_database(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let s = tracking::get(&pool, "car").await.unwrap();
    assert_eq!(s.current_value, "60000000");
    assert_eq!(s.events.len(), 2);
    assert_eq!(s.revision, "2");
    tracking::validate_database(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}
