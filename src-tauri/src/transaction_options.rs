use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    Payee,
    Category,
}
impl OptionKind {
    fn table(self) -> &'static str {
        match self {
            Self::Payee => "payees",
            Self::Category => "categories",
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TransactionOption {
    pub id: String,
    pub name: String,
    pub is_archived: bool,
}
#[derive(Serialize)]
pub struct TransactionOptions {
    payees: Vec<TransactionOption>,
    categories: Vec<TransactionOption>,
}
#[derive(Deserialize)]
pub struct SaveOption {
    pub kind: OptionKind,
    pub id: Option<String>,
    pub name: String,
    pub is_archived: bool,
}

pub async fn save(pool: &SqlitePool, input: SaveOption) -> Result<TransactionOption, String> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a name between 1 and 100 characters.".into());
    }
    let key = name.to_lowercase();
    // Table names come only from the enum; all user-supplied values are bound.
    let table = input.kind.table();
    let result = if let Some(id) = input.id {
        sqlx::query_as::<_, TransactionOption>(&format!("UPDATE {table} SET name = ?, name_key = ?, is_archived = ? WHERE id = ? RETURNING id, name, is_archived"))
            .bind(name).bind(key).bind(input.is_archived).bind(id).fetch_optional(pool).await
    } else {
        sqlx::query_as::<_, TransactionOption>(&format!("INSERT INTO {table} (id, name, name_key, is_archived) VALUES (lower(hex(randomblob(16))), ?, ?, ?) RETURNING id, name, is_archived"))
            .bind(name).bind(key).bind(input.is_archived).fetch_optional(pool).await
    };
    result.map_err(|error| {
        if error.as_database_error().is_some_and(|error| error.is_unique_violation()) {
            "That name already exists, including archived entries. Rename or restore the existing entry.".into()
        } else { error.to_string() }
    })?.ok_or_else(|| "This entry no longer exists. Reload Settings.".into())
}

#[tauri::command]
pub async fn save_transaction_option(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveOption,
) -> Result<TransactionOption, String> {
    save(pool.inner(), input).await
}
#[tauri::command]
pub async fn list_transaction_options(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<TransactionOptions, String> {
    let payees = sqlx::query_as(
        "SELECT id, name, is_archived FROM payees ORDER BY is_archived, name COLLATE NOCASE, id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;
    let categories = sqlx::query_as("SELECT id, name, is_archived FROM categories ORDER BY is_archived, name COLLATE NOCASE, id").fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;
    Ok(TransactionOptions { payees, categories })
}
