use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub is_archived: bool,
    pub builtin_icon: Option<String>,
    pub logo_mode: String,
    pub logo_asset_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveProvider {
    pub id: Option<String>,
    pub name: String,
    pub is_archived: bool,
    pub logo_change: Option<crate::logos::LogoChange>,
}
pub async fn save(pool: &SqlitePool, input: SaveProvider) -> Result<(), String> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a provider name between 1 and 100 characters.".into());
    }
    let key = name.to_lowercase();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscription_providers WHERE name_key=? AND id<>COALESCE(?,''))")
        .bind(&key).bind(&input.id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if duplicate {
        return Err(
            "A subscription provider with this name already exists, including archived providers."
                .into(),
        );
    }
    let id = if let Some(id) = &input.id {
        sqlx::query_scalar::<_, String>("UPDATE subscription_providers SET name=?,name_key=?,is_archived=?,updated_at=CURRENT_TIMESTAMP WHERE id=? RETURNING id")
            .bind(&name).bind(&key).bind(input.is_archived).bind(id).fetch_optional(&mut *tx).await
    } else {
        sqlx::query_scalar::<_, String>("INSERT INTO subscription_providers(id,name,name_key,is_archived) VALUES(lower(hex(randomblob(16))),?,?,?) RETURNING id")
            .bind(&name).bind(&key).bind(input.is_archived).fetch_optional(&mut *tx).await
    }.map_err(|e| e.to_string())?.ok_or("Subscription provider no longer exists. Reload to continue.")?;
    if let Some(change) = input.logo_change {
        let asset = crate::logos::asset(&mut tx, &change).await?;
        let mode = match change {
            crate::logos::LogoChange::Custom { .. } => "custom",
            crate::logos::LogoChange::Default => "default",
            crate::logos::LogoChange::None => "none",
        };
        sqlx::query("UPDATE subscription_providers SET logo_mode=?,logo_asset_id=? WHERE id=?")
            .bind(mode)
            .bind(asset)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        crate::logos::prune(&mut tx).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn list_subscription_providers(pool: &SqlitePool) -> Result<Vec<Provider>, String> {
    sqlx::query_as("SELECT id,name,is_archived,builtin_icon,logo_mode,logo_asset_id FROM subscription_providers ORDER BY name COLLATE NOCASE,id")
        .fetch_all(pool).await.map_err(|e|e.to_string())
}
pub async fn save_subscription_provider(
    pool: &SqlitePool,
    input: SaveProvider,
) -> Result<(), String> {
    save(pool, input).await
}
