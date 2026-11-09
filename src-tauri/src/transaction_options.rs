//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::transaction_options as service;
pub use heyday_core::services::transaction_options::*;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn save_transaction_option(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveOption,
) -> Result<TransactionOption, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_transaction_option(pool.inner(), input).await
}

#[tauri::command]
pub async fn list_transaction_options(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<TransactionOptions, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::list_transaction_options(pool.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn icons_validate_default_and_survive_rename_and_archive() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        let input = |id, icon, archived| SaveOption {
            logo_change: None,
            kind: OptionKind::Category,
            id,
            name: "Food".into(),
            icon,
            is_archived: archived,
        };
        let initial = save(&pool, input(None, None, false)).await.unwrap();
        assert_eq!(initial.icon.as_deref(), Some("tag"));
        assert!(save(
            &pool,
            input(Some(initial.id.clone()), Some("invalid".into()), false)
        )
        .await
        .is_err());
        let selected = save(
            &pool,
            input(Some(initial.id.clone()), Some("utensils".into()), false),
        )
        .await
        .unwrap();
        assert_eq!(selected.icon.as_deref(), Some("utensils"));
        let archived = save(
            &pool,
            SaveOption {
                logo_change: None,
                name: "Dining".into(),
                ..input(Some(initial.id.clone()), None, true)
            },
        )
        .await
        .unwrap();
        assert_eq!(archived.icon.as_deref(), Some("utensils"));
        assert_eq!(archived.name, "Dining");
        assert!(archived.is_archived);
        let restored = save(&pool, input(Some(initial.id), None, false))
            .await
            .unwrap();
        assert_eq!(restored.icon.as_deref(), Some("utensils"));
        assert!(save(
            &pool,
            SaveOption {
                logo_change: None,
                kind: OptionKind::Payee,
                ..input(None, Some("tag".into()), false)
            }
        )
        .await
        .is_err());
    }
}
