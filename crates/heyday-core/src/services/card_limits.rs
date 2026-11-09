use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub current_balance: String,
    pub is_archived: bool,
    pub group_id: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub credit_limit: String,
}
#[derive(Serialize)]
pub struct Directory {
    pub groups: Vec<Group>,
    pub cards: Vec<Card>,
}
#[derive(Deserialize)]
pub struct SaveGroup {
    pub id: Option<String>,
    pub name: String,
    pub credit_limit: String,
    pub currency: String,
    pub account_ids: Vec<String>,
}

pub async fn list(pool: &SqlitePool) -> Result<Directory, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let groups = sqlx::query_as("SELECT id,name,CAST(credit_limit AS TEXT) AS credit_limit FROM card_limit_groups ORDER BY name COLLATE NOCASE,id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    // Archived cards still consume the facility and must remain in the calculation.
    let cards = sqlx::query_as("SELECT a.id,a.name,CAST(a.current_balance AS TEXT) AS current_balance,a.is_archived,m.group_id FROM accounts a LEFT JOIN card_limit_members m ON m.account_id=a.id WHERE a.type='credit_card' ORDER BY a.name COLLATE NOCASE,a.id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Directory { groups, cards })
}

pub async fn save(pool: &SqlitePool, input: SaveGroup) -> Result<(), String> {
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

pub fn save_error(error: sqlx::Error) -> String {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        "A shared limit with this name already exists.".into()
    } else {
        error.to_string()
    }
}
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
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
pub async fn list_card_limit_groups(pool: &SqlitePool) -> Result<Directory, String> {
    list(pool).await
}
pub async fn save_card_limit_group(pool: &SqlitePool, input: SaveGroup) -> Result<(), String> {
    save(pool, input).await
}
pub async fn delete_card_limit_group(pool: &SqlitePool, id: String) -> Result<(), String> {
    remove(pool, &id).await
}
