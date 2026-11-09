use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const CATEGORY_ICONS: &[&str] = &[
    "tag",
    "utensils",
    "coffee",
    "shopping-cart",
    "bus",
    "fuel",
    "house",
    "plug-zap",
    "heart-pulse",
    "shopping-bag",
    "clapperboard",
    "graduation-cap",
    "plane",
    "gift",
    "paw-print",
    "baby",
    "shirt",
    "dumbbell",
    "smartphone",
    "wifi",
    "shield-check",
    "wrench",
    "scissors",
    "hand-heart",
    "briefcase-business",
    "volleyball",
    "users",
    "spray-can",
    "receipt-text",
];

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
    #[sqlx(default)]
    pub icon: Option<String>,
    #[sqlx(default)]
    pub logo_asset_id: Option<String>,
}
#[derive(Serialize)]
pub struct TransactionOptions {
    pub payees: Vec<TransactionOption>,
    pub categories: Vec<TransactionOption>,
}
#[derive(Deserialize)]
pub struct SaveOption {
    pub logo_change: Option<crate::logos::LogoChange>,
    pub kind: OptionKind,
    pub id: Option<String>,
    pub name: String,
    pub is_archived: bool,
    pub icon: Option<String>,
}

pub async fn save(pool: &SqlitePool, input: SaveOption) -> Result<TransactionOption, String> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a name between 1 and 100 characters.".into());
    }
    let key = name.to_lowercase();
    // Table names come only from the enum; all user-supplied values are bound.
    let table = input.kind.table();
    let category = matches!(input.kind, OptionKind::Category);
    if let Some(icon) = input.icon.as_deref() {
        if !category || !CATEGORY_ICONS.contains(&icon) {
            return Err("Choose a supported category icon.".into());
        }
    }
    if category && input.logo_change.is_some() {
        return Err("Custom logos apply only to payees and institutions.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = if category {
        if let Some(id) = input.id {
            sqlx::query_as::<_, TransactionOption>("UPDATE categories SET name=?,name_key=?,is_archived=?,icon=COALESCE(?,icon) WHERE id=? RETURNING id,name,is_archived,icon")
                .bind(name).bind(key).bind(input.is_archived).bind(input.icon).bind(id).fetch_optional(&mut *tx).await
        } else {
            sqlx::query_as::<_, TransactionOption>("INSERT INTO categories(id,name,name_key,is_archived,icon) VALUES(lower(hex(randomblob(16))),?,?,?,COALESCE(?,'tag')) RETURNING id,name,is_archived,icon")
                .bind(name).bind(key).bind(input.is_archived).bind(input.icon).fetch_optional(&mut *tx).await
        }
    } else if let Some(id) = input.id {
        sqlx::query_as::<_, TransactionOption>(&format!("UPDATE {table} SET name = ?, name_key = ?, is_archived = ? WHERE id = ? RETURNING id, name, is_archived"))
            .bind(name).bind(key).bind(input.is_archived).bind(id).fetch_optional(&mut *tx).await
    } else {
        sqlx::query_as::<_, TransactionOption>(&format!("INSERT INTO {table} (id, name, name_key, is_archived) VALUES (lower(hex(randomblob(16))), ?, ?, ?) RETURNING id, name, is_archived"))
            .bind(name).bind(key).bind(input.is_archived).fetch_optional(&mut *tx).await
    };
    let mut saved = result.map_err(|error| {
        if error.as_database_error().is_some_and(|error| error.is_unique_violation()) {
            "That name already exists, including archived entries. Rename or restore the existing entry.".into()
        } else { error.to_string() }
    })?.ok_or_else(|| "This entry no longer exists. Reload Settings.".to_string())?;
    if !category {
        if let Some(change) = input.logo_change {
            let id = crate::logos::asset(&mut tx, &change).await?;
            sqlx::query("UPDATE payees SET logo_asset_id=? WHERE id=?")
                .bind(&id)
                .bind(&saved.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            crate::logos::prune(&mut tx).await?;
        }
        saved.logo_asset_id = sqlx::query_scalar("SELECT logo_asset_id FROM payees WHERE id=?")
            .bind(&saved.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(saved)
}

pub async fn save_transaction_option(
    pool: &SqlitePool,
    input: SaveOption,
) -> Result<TransactionOption, String> {
    save(pool, input).await
}
pub async fn list_transaction_options(pool: &SqlitePool) -> Result<TransactionOptions, String> {
    let payees = sqlx::query_as(
        "SELECT id, name, is_archived, logo_asset_id FROM payees ORDER BY is_archived, name COLLATE NOCASE, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let categories = sqlx::query_as("SELECT id, name, is_archived, icon FROM categories ORDER BY is_archived, name COLLATE NOCASE, id").fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(TransactionOptions { payees, categories })
}
