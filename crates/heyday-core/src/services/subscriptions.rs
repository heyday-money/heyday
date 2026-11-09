use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub account_id: String,
    pub account_name: String,
    pub account_type: String,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub amount: String,
    pub frequency: String,
    pub first_billing_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
    pub logo_asset_id: Option<String>,
    pub provider_id: Option<String>,
    pub provider_icon: Option<String>,
    pub managed_via: Option<String>,
    pub management_url: Option<String>,
}
pub const SELECT: &str = "SELECT s.id, s.name, s.account_id, a.name AS account_name, a.type AS account_type, s.category_id, c.name AS category_name, CAST(s.amount AS TEXT) AS amount, s.frequency, s.first_billing_date, s.end_date, s.is_active, COALESCE(s.logo_asset_id,p.logo_asset_id) AS logo_asset_id, s.provider_id, CASE WHEN p.logo_mode='default' THEN p.builtin_icon END AS provider_icon, s.managed_via, s.management_url FROM subscriptions s JOIN accounts a ON a.id = s.account_id LEFT JOIN categories c ON c.id = s.category_id LEFT JOIN subscription_providers p ON p.id=s.provider_id ORDER BY s.name COLLATE NOCASE, s.id";

#[derive(Deserialize)]
pub struct SaveSubscription {
    pub id: Option<String>,
    pub name: String,
    pub account_id: String,
    pub category_id: Option<String>,
    pub amount: String,
    pub frequency: String,
    pub first_billing_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
    pub currency: String,
    pub logo_change: Option<crate::logos::LogoChange>,
    pub provider_id: Option<String>,
    pub managed_via: Option<String>,
    pub management_url: Option<String>,
}
pub fn management_url(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let error = "Enter an HTTPS management link without login credentials (up to 2048 characters).";
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(error.into());
    }
    let url = url::Url::parse(value).map_err(|_| error.to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.as_str().len() > 2048
    {
        return Err(error.into());
    }
    Ok(Some(url.to_string()))
}

pub async fn save(pool: &SqlitePool, input: SaveSubscription) -> Result<(), String> {
    let management_url = management_url(input.management_url.as_deref())?;
    if input.managed_via.as_deref().is_some_and(|value| {
        ![
            "apple_app_store",
            "google_play",
            "website",
            "in_app",
            "other",
        ]
        .contains(&value)
    }) {
        return Err("Choose a supported subscription management platform.".into());
    }
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter a subscription name between 1 and 100 characters.".into());
    }
    let amount = input
        .amount
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if amount <= 0 {
        return Err("Amount must be greater than zero.".into());
    }
    if !["monthly", "yearly"].contains(&input.frequency.as_str()) {
        return Err("Choose monthly or yearly billing.".into());
    }
    if !crate::transactions::valid_date(&input.first_billing_date) {
        return Err("Enter a valid first billing date.".into());
    }
    if input.end_date.as_ref().is_some_and(|date| {
        !crate::transactions::valid_date(date) || date < &input.first_billing_date
    }) {
        return Err("End date must be valid and no earlier than the first billing date.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload before saving the subscription.".into());
    }
    let account: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank', 'wallet', 'credit_card'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    // A paused existing schedule can keep its unavailable account so it can be disabled.
    let unchanged_account: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id = ? AND account_id = ?)",
    )
    .bind(&input.id)
    .bind(&input.account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if !account && (input.is_active || !unchanged_account) {
        return Err("Choose an active cash, bank, digital wallet, or credit card account.".into());
    }
    if let Some(id) = &input.category_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE id = ? AND (is_archived = 0 OR EXISTS(SELECT 1 FROM subscriptions WHERE id = ? AND category_id = categories.id)))").bind(id).bind(&input.id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if !valid {
            return Err("Choose an active category.".into());
        }
    }
    if let Some(provider) = &input.provider_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscription_providers p WHERE p.id=? AND (p.is_archived=0 OR EXISTS(SELECT 1 FROM subscriptions s WHERE s.id=? AND s.provider_id=p.id)))")
            .bind(provider).bind(&input.id).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
        if !valid {
            return Err("Choose an active subscription provider.".into());
        }
    }
    let result = if let Some(id) = &input.id {
        sqlx::query_scalar::<_, String>("UPDATE subscriptions SET name = ?, account_id = ?, category_id = ?, amount = ?, frequency = ?, first_billing_date = ?, end_date = ?, is_active = ?, managed_via = ?, management_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING id")
            .bind(name).bind(input.account_id).bind(input.category_id).bind(amount).bind(input.frequency).bind(input.first_billing_date).bind(input.end_date).bind(input.is_active).bind(&input.managed_via).bind(&management_url).bind(id).fetch_optional(&mut *tx).await
    } else {
        sqlx::query_scalar::<_, String>("INSERT INTO subscriptions (id, name, account_id, category_id, amount, frequency, first_billing_date, end_date, is_active, managed_via, management_url) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
            .bind(name).bind(input.account_id).bind(input.category_id).bind(amount).bind(input.frequency).bind(input.first_billing_date).bind(input.end_date).bind(input.is_active).bind(&input.managed_via).bind(&management_url).fetch_optional(&mut *tx).await
    }.map_err(|e| e.to_string())?;
    let id = result.ok_or("Subscription no longer exists. Reload to continue.")?;
    sqlx::query("UPDATE subscriptions SET provider_id=? WHERE id=?")
        .bind(&input.provider_id)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(change) = input.logo_change {
        let asset_id = crate::logos::asset(&mut tx, &change).await?;
        sqlx::query("UPDATE subscriptions SET logo_asset_id=? WHERE id=?")
            .bind(asset_id)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        crate::logos::prune(&mut tx).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = sqlx::query("DELETE FROM subscriptions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Subscription no longer exists. Reload to continue.".into());
    }
    crate::logos::prune(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn save_subscription(pool: &SqlitePool, input: SaveSubscription) -> Result<(), String> {
    save(pool, input).await
}
pub async fn delete_subscription(pool: &SqlitePool, id: String) -> Result<(), String> {
    remove(pool, &id).await
}

/// Resolve and validate a saved link; opening a browser remains application-owned.
pub async fn saved_management_url(pool: &SqlitePool, id: &str) -> Result<String, String> {
    let saved: Option<String> =
        sqlx::query_scalar("SELECT management_url FROM subscriptions WHERE id=?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .flatten();
    management_url(saved.as_deref())?
        .ok_or_else(|| "Add a management link in Edit subscription first.".into())
}
