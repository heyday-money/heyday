//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::subscription_providers as service;
pub use heyday_core::services::subscription_providers::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn list_subscription_providers(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Provider>, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_subscription_providers(pool.inner()).await
}

#[tauri::command]
pub async fn save_subscription_provider(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveProvider,
) -> Result<(), String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_subscription_provider(pool.inner(), input).await
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
