use heyday_core::services::insurance::{self, Policy, RecordPayment, Reverse, Save};
use sqlx::SqlitePool;
#[tauri::command]
pub async fn list_insurance(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Policy>, String> {
    insurance::list(pool.inner()).await
}
#[tauri::command]
pub async fn save_insurance(
    pool: tauri::State<'_, SqlitePool>,
    input: Save,
) -> Result<Policy, String> {
    insurance::save(pool.inner(), input).await
}
#[tauri::command]
pub async fn pay_insurance(
    pool: tauri::State<'_, SqlitePool>,
    input: RecordPayment,
) -> Result<Policy, String> {
    insurance::pay(pool.inner(), input).await
}
#[tauri::command]
pub async fn reverse_insurance(
    pool: tauri::State<'_, SqlitePool>,
    input: Reverse,
) -> Result<Policy, String> {
    insurance::reverse(pool.inner(), input).await
}
