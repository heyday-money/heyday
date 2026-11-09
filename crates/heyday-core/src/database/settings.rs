use serde::Serialize;
use sqlx::SqlitePool;
#[derive(Serialize, sqlx::FromRow)]
pub struct Settings {
    pub currency: Option<String>,
    pub period_start_day: i64,
}

pub async fn save_currency(pool: &SqlitePool, currency: &str) -> Result<Settings, String> {
    if ![
        "THB", "USD", "EUR", "GBP", "JPY", "CNY", "SGD", "AUD", "CAD", "CHF", "INR", "KRW", "VND",
        "KWD", "BHD",
    ]
    .contains(&currency)
    {
        return Err("Choose a supported currency.".into());
    }
    sqlx::query_as::<_, Settings>("UPDATE settings SET currency = ? WHERE id = 1 AND (currency = ? OR (NOT EXISTS (SELECT 1 FROM accounts) AND NOT EXISTS (SELECT 1 FROM incomes))) RETURNING currency, period_start_day")
        .bind(currency).bind(currency).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .ok_or_else(|| "Currency cannot change after accounts or income have been created.".into())
}
pub async fn save_period(pool: &SqlitePool, day: i64) -> Result<Settings, String> {
    if !(1..=31).contains(&day) {
        return Err("Period start day must be between 1 and 31.".into());
    }
    sqlx::query_as::<_, Settings>(
        "UPDATE settings SET period_start_day = ? WHERE id = 1 RETURNING currency, period_start_day",
    )
    .bind(day)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())
}

/// Read persisted settings without choosing application paths or a platform clock.
pub async fn get_settings(pool: &SqlitePool) -> Result<Settings, String> {
    sqlx::query_as::<_, Settings>("SELECT currency, period_start_day FROM settings WHERE id = 1")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())
}
