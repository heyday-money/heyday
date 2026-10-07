use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Provider {
    id: String,
    name: String,
    is_archived: bool,
    builtin_icon: Option<String>,
    logo_mode: String,
    logo_asset_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveProvider {
    id: Option<String>,
    name: String,
    is_archived: bool,
    logo_change: Option<crate::logos::LogoChange>,
}
async fn save(pool: &SqlitePool, input: SaveProvider) -> Result<(), String> {
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
#[tauri::command]
pub async fn list_subscription_providers(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Provider>, String> {
    let _operation = crate::backups::operation()?;
    sqlx::query_as("SELECT id,name,is_archived,builtin_icon,logo_mode,logo_asset_id FROM subscription_providers ORDER BY name COLLATE NOCASE,id")
        .fetch_all(pool.inner()).await.map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn save_subscription_provider(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveProvider,
) -> Result<(), String> {
    let _operation = crate::backups::operation()?;
    save(pool.inner(), input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn providers_validate_names_preserve_links_and_roll_back_bad_logos() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let input = |id, name: &str, archived| SaveProvider {
            id,
            name: name.into(),
            is_archived: archived,
            logo_change: None,
        };
        assert!(save(&pool, input(None, " NETFLIX ", false)).await.is_err());
        assert!(save(&pool, input(None, "  ", false)).await.is_err());
        save(&pool, input(None, " My   Service ", false))
            .await
            .unwrap();
        let (id, name): (String, String) = sqlx::query_as(
            "SELECT id,name FROM subscription_providers WHERE name_key='my service'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(name, "My Service");
        let mut bad = input(Some(id.clone()), "Changed", false);
        bad.logo_change = Some(crate::logos::LogoChange::Custom {
            data: "invalid".into(),
        });
        assert!(save(&pool, bad).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT name FROM subscription_providers WHERE id=?")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "My Service"
        );
        save(&pool, input(Some(id.clone()), "My Service", true))
            .await
            .unwrap();
        assert!(save(&pool, input(None, "my service", false)).await.is_err());
        save(&pool, input(Some(id), "Renamed Service", false))
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
}
