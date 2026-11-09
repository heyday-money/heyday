use heyday_core::{
    database::{settings, MIGRATOR},
    services::{accounts, incomes, planning, reset, transactions},
};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

#[test]
fn all_migration_versions_and_checksums_match_the_desktop_baseline() {
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/migration-checksums.json")).unwrap();
    let actual: Vec<_> = MIGRATOR.iter().collect();
    assert_eq!(actual.len(), expected.len());
    for (migration, baseline) in actual.iter().zip(expected) {
        assert_eq!(migration.version, baseline["version"].as_i64().unwrap());
        let checksum: String = migration
            .checksum
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            checksum,
            baseline["sha384"].as_str().unwrap(),
            "{}",
            baseline["file"]
        );
    }
}

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

async fn account(pool: &SqlitePool, name: &str, balance: &str) -> accounts::Account {
    accounts::create_account(
        pool,
        serde_json::from_value(json!({
            "name":name,"type":"bank","opening_balance":balance,"currency":"THB"
        }))
        .unwrap(),
        None,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn standalone_services_preserve_exact_balances_atomic_transfers_and_reversal() {
    let pool = db().await;
    let from = account(&pool, "Source", "9007199254740993").await;
    let to = account(&pool, "Destination", "9223372036854775807").await;
    let input = || {
        serde_json::from_value(json!({"type":"transfer","account_id":from.id,
        "destination_account_id":to.id,"amount":"1","date":"2024-01-01","description":"","currency":"THB"})).unwrap()
    };
    assert!(transactions::create_transaction(&pool, input())
        .await
        .is_err());
    let balance: i64 = sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=?")
        .bind(&from.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(balance, 9007199254740993);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("UPDATE accounts SET current_balance=0,opening_balance=0 WHERE id=?")
        .bind(&to.id)
        .execute(&pool)
        .await
        .unwrap();
    let transfer = transactions::create_transaction(&pool, input())
        .await
        .unwrap();
    accounts::change_archive(&pool, &to.id, true).await.unwrap();
    assert!(accounts::remove_archived(&pool, &to.id).await.is_err());
    transactions::remove_confirmed(&pool, &transfer.id, false)
        .await
        .unwrap();
    let balance: i64 = sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=?")
        .bind(&from.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(balance, 9007199254740993);
    accounts::remove_archived(&pool, &to.id).await.unwrap();
}

#[tokio::test]
async fn standalone_income_snapshot_and_reset_use_bundled_schema_and_seeds() {
    let pool = db().await;
    let bank = account(&pool, "Bank", "1000").await;
    incomes::create_income(&pool, serde_json::from_value(json!({
        "name":"Salary","destination_account_id":bank.id,"type":"salary","estimated_amount":"100",
        "currency":"THB","recurrence_frequency":"monthly","recurrence_day_of_month":31,
        "is_auto_create_transaction":false,"is_active":true
    })).unwrap()).await.unwrap();
    let snapshot = planning::get_financial_data(&pool).await.unwrap();
    let serialized = serde_json::to_value(snapshot).unwrap();
    assert_eq!(serialized["accounts"][0]["current_balance"], "1000");
    assert_eq!(serialized["incomes"][0]["estimated_amount"], "100");
    assert!(reset::clear_data(&pool, "wrong").await.is_err());
    reset::clear_data(&pool, "DELETE ALL DATA").await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM institutions")
            .fetch_one(&pool)
            .await
            .unwrap()
            > 0
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subscription_providers")
            .fetch_one(&pool)
            .await
            .unwrap()
            > 0
    );
    MIGRATOR.run(&pool).await.unwrap();
}
