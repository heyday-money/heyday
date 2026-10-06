use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Card {
    id: String,
    name: String,
    current_balance: String,
    is_archived: bool,
    group_id: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Group {
    id: String,
    name: String,
    credit_limit: String,
}
#[derive(Serialize)]
pub struct Directory {
    groups: Vec<Group>,
    cards: Vec<Card>,
}
#[derive(Deserialize)]
pub struct SaveGroup {
    id: Option<String>,
    name: String,
    credit_limit: String,
    currency: String,
    account_ids: Vec<String>,
}

async fn list(pool: &SqlitePool) -> Result<Directory, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let groups = sqlx::query_as("SELECT id,name,CAST(credit_limit AS TEXT) AS credit_limit FROM card_limit_groups ORDER BY name COLLATE NOCASE,id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    // Archived cards still consume the facility and must remain in the calculation.
    let cards = sqlx::query_as("SELECT a.id,a.name,CAST(a.current_balance AS TEXT) AS current_balance,a.is_archived,m.group_id FROM accounts a LEFT JOIN card_limit_members m ON m.account_id=a.id WHERE a.type='credit_card' ORDER BY a.name COLLATE NOCASE,a.id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Directory { groups, cards })
}

async fn save(pool: &SqlitePool, input: SaveGroup) -> Result<(), String> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a group name between 1 and 100 characters.".into());
    }
    let limit = input
        .credit_limit
        .parse::<i64>()
        .map_err(|_| "Enter a valid credit limit.")?;
    if limit < 0 {
        return Err("Credit limit cannot be negative.".into());
    }
    if input.account_ids.is_empty() {
        return Err("Select at least one credit card.".into());
    }
    let mut unique = std::collections::HashSet::new();
    if !input.account_ids.iter().all(|id| unique.insert(id)) {
        return Err("Select each card only once.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(&input.currency) {
        return Err("Currency changed. Reload Accounts.".into());
    }
    let id: String = if let Some(id) = input.id {
        let result =
            sqlx::query("UPDATE card_limit_groups SET name=?,name_key=?,credit_limit=? WHERE id=?")
                .bind(&name)
                .bind(name.to_lowercase())
                .bind(limit)
                .bind(&id)
                .execute(&mut *tx)
                .await
                .map_err(save_error)?;
        if result.rows_affected() == 0 {
            return Err("This shared limit no longer exists. Reload Accounts.".into());
        }
        id
    } else {
        sqlx::query_scalar("INSERT INTO card_limit_groups(id,name,name_key,credit_limit) VALUES(lower(hex(randomblob(16))),?,?,?) RETURNING id")
            .bind(&name).bind(name.to_lowercase()).bind(limit).fetch_one(&mut *tx).await.map_err(save_error)?
    };
    for card in &input.account_ids {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts a LEFT JOIN card_limit_members m ON m.account_id=a.id WHERE a.id=? AND a.type='credit_card' AND (a.is_archived=0 OR m.group_id=?) AND (m.group_id IS NULL OR m.group_id=?))")
            .bind(card).bind(&id).bind(&id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if !valid {
            return Err("A selected card is unavailable or belongs to another shared limit. Reload Accounts.".into());
        }
    }
    sqlx::query("DELETE FROM card_limit_members WHERE group_id=?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for card in &input.account_ids {
        sqlx::query("INSERT INTO card_limit_members(account_id,group_id) VALUES(?,?)")
            .bind(card)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

fn save_error(error: sqlx::Error) -> String {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        "A shared limit with this name already exists.".into()
    } else {
        error.to_string()
    }
}
async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM card_limit_groups WHERE id=?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("This shared limit no longer exists. Reload Accounts.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn list_card_limit_groups(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Directory, String> {
    let _database_operation = crate::backups::operation()?;
    list(pool.inner()).await
}
#[tauri::command]
pub async fn save_card_limit_group(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveGroup,
) -> Result<(), String> {
    let _database_operation = crate::backups::operation()?;
    save(pool.inner(), input).await
}
#[tauri::command]
pub async fn delete_card_limit_group(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    let _database_operation = crate::backups::operation()?;
    remove(pool.inner(), &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn database() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='THB';
        INSERT INTO accounts(id,name,type,opening_balance,current_balance,credit_limit,statement_day,payment_due_day) VALUES('a','Card A','credit_card',2000000,2000000,5000000,10,30),('b','Card B','credit_card',1500000,1500000,6000000,25,15),('cash','Cash','bank',10000000,10000000,NULL,NULL,NULL);").execute(&pool).await.unwrap();
        pool
    }
    fn input(id: Option<String>, cards: &[&str]) -> SaveGroup {
        SaveGroup {
            id,
            name: "Shared bank".into(),
            credit_limit: "10000000".into(),
            currency: "THB".into(),
            account_ids: cards.iter().map(|s| s.to_string()).collect(),
        }
    }
    async fn available(pool: &SqlitePool) -> i128 {
        let data = list(pool).await.unwrap();
        let group = &data.groups[0];
        group.credit_limit.parse::<i128>().unwrap()
            - data
                .cards
                .iter()
                .filter(|c| c.group_id.as_ref() == Some(&group.id))
                .map(|c| c.current_balance.parse::<i128>().unwrap())
                .sum::<i128>()
    }
    #[tokio::test]
    async fn shared_limits_preserve_billing_and_follow_transactions_and_reversal() {
        let pool = database().await;
        save(&pool, input(None, &["a", "b"])).await.unwrap();
        assert_eq!(available(&pool).await, 6500000);
        let new = |kind: &str, source: &str, destination: Option<&str>, amount: &str| {
            serde_json::from_value::<crate::transactions::NewTransaction>(serde_json::json!({"type":kind,"account_id":source,"destination_account_id":destination,"amount":amount,"date":"2026-01-01","description":"","currency":"THB"})).unwrap()
        };
        crate::transactions::insert(&pool, new("repayment", "cash", Some("a"), "4000000"))
            .await
            .unwrap();
        assert_eq!(available(&pool).await, 10500000); // Overpayment exceeds the shared limit.
        crate::transactions::insert(&pool, new("expense", "b", None, "500000"))
            .await
            .unwrap();
        assert_eq!(available(&pool).await, 10000000);
        let id: String = sqlx::query_scalar("SELECT id FROM transactions WHERE type='expense'")
            .fetch_one(&pool)
            .await
            .unwrap();
        crate::transactions::remove(&pool, &id).await.unwrap();
        assert_eq!(available(&pool).await, 10500000);
        crate::transactions::insert(&pool, new("transfer", "a", Some("b"), "100000"))
            .await
            .unwrap();
        assert_eq!(available(&pool).await, 10500000); // Within-group transfers cancel.
        let schedules: Vec<(String,i64,i64,i64)> = sqlx::query_as("SELECT id,credit_limit,statement_day,payment_due_day FROM accounts WHERE type='credit_card' ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(
            schedules,
            vec![("a".into(), 5000000, 10, 30), ("b".into(), 6000000, 25, 15)]
        );
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='a'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(available(&pool).await, 10500000);
        let group = list(&pool).await.unwrap().groups.remove(0);
        save(
            &pool,
            SaveGroup {
                name: "Renamed".into(),
                ..input(Some(group.id.clone()), &["a", "b"])
            },
        )
        .await
        .unwrap();
        remove(&pool, &group.id).await.unwrap();
        assert!(list(&pool)
            .await
            .unwrap()
            .cards
            .iter()
            .all(|c| c.group_id.is_none()));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
    #[tokio::test]
    async fn migration_and_restart_preserve_balances_statements_and_membership() {
        let path = std::env::temp_dir().join(format!(
            "heyday-card-limits-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 35)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("UPDATE settings SET currency='THB';
        INSERT INTO accounts(id,name,type,opening_balance,current_balance,statement_day,payment_due_day) VALUES('a','Card','credit_card',9007199254740993,9007199254740994,10,30);
        INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('t','expense','a',1,'2026-01-01','');
        INSERT INTO card_statements(id,account_id,start_date,end_date,due_date,amount,minimum) VALUES('s','a','2026-01-01','2026-01-10','2026-01-30',100,10);").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0035_shared_card_limits.sql"))
            .execute(&pool)
            .await
            .unwrap();
        save(&pool, input(None, &["a"])).await.unwrap();
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        let data = list(&reopened).await.unwrap();
        assert_eq!(data.cards[0].current_balance, "9007199254740994");
        assert_eq!(
            data.cards[0].group_id.as_deref(),
            Some(data.groups[0].id.as_str())
        );
        let preserved:(i64,i64,i64)=sqlx::query_as("SELECT a.opening_balance,t.amount,s.amount FROM accounts a JOIN transactions t ON t.account_id=a.id JOIN card_statements s ON s.account_id=a.id").fetch_one(&reopened).await.unwrap();
        assert_eq!(preserved, (9007199254740993, 1, 100));
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn validates_members_limits_currency_and_rolls_back_failed_updates() {
        let pool = database().await;
        for cards in [vec![], vec!["a", "a"], vec!["cash"], vec!["missing"]] {
            assert!(save(&pool, input(None, &cards)).await.is_err());
        }
        for limit in ["-1", "9223372036854775808", "1.5"] {
            assert!(save(
                &pool,
                SaveGroup {
                    credit_limit: limit.into(),
                    ..input(None, &["a"])
                }
            )
            .await
            .is_err());
        }
        assert!(save(
            &pool,
            SaveGroup {
                currency: "USD".into(),
                ..input(None, &["a"])
            }
        )
        .await
        .is_err());
        assert!(list(&pool).await.unwrap().groups.is_empty());
        save(
            &pool,
            SaveGroup {
                credit_limit: "9223372036854775807".into(),
                ..input(None, &["a", "b"])
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE accounts SET current_balance=9223372036854775807 WHERE type='credit_card'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(available(&pool).await, -9223372036854775807);
        assert!(save(
            &pool,
            SaveGroup {
                name: "Other".into(),
                ..input(None, &["b"])
            }
        )
        .await
        .is_err());
        let id = list(&pool).await.unwrap().groups.remove(0).id;
        assert!(save(
            &pool,
            SaveGroup {
                name: "Changed".into(),
                ..input(Some(id.clone()), &["cash"])
            }
        )
        .await
        .is_err());
        assert_eq!(list(&pool).await.unwrap().groups[0].name, "Shared bank");
        save(
            &pool,
            SaveGroup {
                credit_limit: "0".into(),
                ..input(Some(id), &["a"])
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='b'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(save(
            &pool,
            SaveGroup {
                name: "Other".into(),
                ..input(None, &["b"])
            }
        )
        .await
        .is_err());
        assert!(remove(&pool, "missing").await.is_err());
    }
}
