use heyday_core::{
    domain::tracking::TrackingDraft,
    services::{
        logos::LogoChange,
        tracking::{self, EventInput, TrackingAccount},
    },
};
use sqlx::SqlitePool;
#[tauri::command]
pub async fn get_tracking_account(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
) -> Result<TrackingAccount, String> {
    tracking::get(pool.inner(), &account_id).await
}
#[tauri::command]
pub async fn create_tracking_account(
    pool: tauri::State<'_, SqlitePool>,
    input: TrackingDraft,
    logo_change: Option<LogoChange>,
) -> Result<TrackingAccount, String> {
    tracking::create_with_logo(pool.inner(), input, logo_change).await
}
#[tauri::command]
pub async fn edit_tracking_account(
    pool: tauri::State<'_, SqlitePool>,
    account_id: String,
    expected_revision: String,
    input: TrackingDraft,
    logo_change: Option<LogoChange>,
) -> Result<TrackingAccount, String> {
    tracking::edit_with_logo(
        pool.inner(),
        &account_id,
        &expected_revision,
        input,
        logo_change,
    )
    .await
}
#[tauri::command]
pub async fn record_tracking_event(
    pool: tauri::State<'_, SqlitePool>,
    input: EventInput,
) -> Result<TrackingAccount, String> {
    tracking::record(pool.inner(), input).await
}
