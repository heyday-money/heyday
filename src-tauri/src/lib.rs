use serde::Serialize;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use tauri::Manager;
mod accounts;
mod backups;
mod card_limits;
mod institutions;
mod logos;
mod card_billing;
mod cashflow;
mod income_deductions;
mod incomes;
mod installments;
mod loans;
mod planning;
mod reconciliation;
mod reset;
mod selective_defaults;
mod subscriptions;
mod transaction_options;
mod transactions;
mod updates;

#[derive(Serialize, sqlx::FromRow)]
struct Settings {
    currency: Option<String>,
    period_start_day: i64,
}

#[tauri::command]
async fn get_settings(pool: tauri::State<'_, SqlitePool>) -> Result<Settings, String> {
    let _database_operation = crate::backups::operation()?;
    sqlx::query_as::<_, Settings>("SELECT currency, period_start_day FROM settings WHERE id = 1")
        .fetch_one(pool.inner())
        .await
        .map_err(|error| error.to_string())
}

async fn save_period(pool: &SqlitePool, day: i64) -> Result<Settings, String> {
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

#[tauri::command]
async fn update_period(pool: tauri::State<'_, SqlitePool>, day: i64) -> Result<Settings, String> {
    let _database_operation = crate::backups::operation()?;
    save_period(pool.inner(), day).await
}

async fn save_currency(pool: &SqlitePool, currency: &str) -> Result<Settings, String> {
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

#[tauri::command]
async fn update_currency(
    pool: tauri::State<'_, SqlitePool>,
    currency: String,
) -> Result<Settings, String> {
    let _database_operation = crate::backups::operation()?;
    save_currency(pool.inner(), &currency).await
}

fn data_directory(base: std::path::PathBuf, development: bool) -> std::path::PathBuf {
    if development {
        base.join("development")
    } else {
        base
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(backups::BackupState::default())
        .setup(|app| {
            let directory = data_directory(app.path().app_data_dir()?, cfg!(debug_assertions));
            std::fs::create_dir_all(&directory)?;
            let options = SqliteConnectOptions::new()
                .filename(directory.join("heyday.db"))
                .create_if_missing(true)
                .foreign_keys(true);
            let pool = tauri::async_runtime::block_on(async {
                let pool = SqlitePool::connect_with(options).await?;
                sqlx::migrate!("./migrations").run(&pool).await?;
                Ok::<_, Box<dyn std::error::Error>>(pool)
            })?;
            app.manage(pool);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            backups::export_backup,
            backups::preview_backup,
            backups::restore_backup,
            backups::cancel_backup_restore,
            institutions::list_institutions,
            institutions::save_institution,
            institutions::delete_institution,
            card_limits::list_card_limit_groups,
            card_billing::get_card_overview,
            card_limits::save_card_limit_group,
            card_limits::delete_card_limit_group,
            logos::get_logo_asset,
            card_billing::get_card_billing,
            card_billing::save_card_statement,
            card_billing::save_card_payment_plan,
            card_billing::record_card_payment,
            card_billing::remove_card_billing_record,
            updates::check_app_update,
            updates::install_app_update,
            reset::clear_all_data,
            cashflow::get_cashflow_planner,
            cashflow::save_cashflow_planner,
            planning::get_financial_data,
            subscriptions::save_subscription,
            subscriptions::delete_subscription,
            loans::get_loan_account,
            loans::save_loan_facility,
            loans::save_loan_contract,
            loans::record_loan_repayment,
            loans::delete_loan_contract,
            installments::save_installment,
            installments::delete_installment,
            planning::save_payment_plan,
            planning::delete_payment_plan,
            update_period,
            update_currency,
            reconciliation::get_reconciliation,
            reconciliation::set_transaction_verification,
            reconciliation::finish_reconciliation,
            accounts::create_account,
            accounts::update_account,
            accounts::list_accounts,
            accounts::set_account_archived,
            accounts::mark_loan_paid_off,
            selective_defaults::get_selective_default,
            selective_defaults::save_selective_default,
            incomes::create_income,
            incomes::update_income,
            incomes::list_incomes,
            income_deductions::list_income_deductions,
            income_deductions::save_salary_deductions,
            transactions::create_transaction,
            transactions::list_transactions,
            transactions::get_expense_report,
            transactions::delete_transaction,
            transaction_options::list_transaction_options,
            transaction_options::save_transaction_option
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Heyday Money");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn development_storage_never_uses_production_database() {
        let base = std::path::PathBuf::from("app-data/money.heyday.desktop");
        assert_eq!(data_directory(base.clone(), false), base);
        assert_eq!(data_directory(base.clone(), true), base.join("development"));
        assert_ne!(
            data_directory(base.clone(), true).join("heyday.db"),
            base.join("heyday.db")
        );
    }

    #[tokio::test]
    async fn migration_defaults_and_income_integrity() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let settings: Settings = sqlx::query_as("SELECT currency, period_start_day FROM settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(settings.period_start_day, 1);
        assert!(settings.currency.is_none());
        for day in [25, 31, 1] {
            let saved = save_period(&pool, day).await.unwrap();
            assert_eq!(saved.period_start_day, day);
            assert!(saved.currency.is_none());
            let stored: i64 =
                sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(stored, day);
        }
        for day in [-1, 0, 32] {
            assert!(save_period(&pool, day).await.is_err());
        }
        let stored: i64 = sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, 1);
        assert!(sqlx::query("UPDATE settings SET period_start_day = 32")
            .execute(&pool)
            .await
            .is_err());
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Bank', 'bank', 1000)")
            .execute(&pool).await.unwrap();
        let insert = "INSERT INTO incomes (id, name, destination_account_id, type, estimated_amount, recurrence_day_of_month) VALUES (?, 'Salary', ?, 'salary', 5000, 25)";
        assert!(sqlx::query(insert)
            .bind("invalid")
            .bind("missing")
            .execute(&pool)
            .await
            .is_err());
        sqlx::query(insert)
            .bind("salary")
            .bind("bank")
            .execute(&pool)
            .await
            .unwrap();
        let balance: i64 =
            sqlx::query_scalar("SELECT opening_balance FROM accounts WHERE id = 'bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 1000);
        assert!(sqlx::query("DELETE FROM accounts WHERE id = 'bank'")
            .execute(&pool)
            .await
            .is_err());
    }
}
