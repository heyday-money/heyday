use sqlx::SqlitePool;

// Child-first order preserves foreign-key enforcement throughout the reset.
pub const DATA_TABLES: &[&str] = &[
    "cloud_sync_binding",
    "cloud_sync_pending",
    "cloud_sync_dirty",
    "cloud_sync_shadow",
    "ledger_sync_imports",
    "ledger_sync_members",
    "salary_payment_transactions",
    "salary_payments",
    "selective_defaults",
    "loan_payoffs",
    "card_limit_members",
    "card_limit_groups",
    "card_statement_installments",
    "card_payment_allocations",
    "card_payment_plans",
    "card_statement_entries",
    "card_statements",
    "loan_sync_reservations",
    "loan_payment_parts",
    "loan_contracts",
    "loan_facilities",
    "transaction_verifications",
    "reconciliation_entries",
    "reconciliations",
    "planner_amounts",
    "planner_income_amounts",
    "planner_deduction_amounts",
    "planner_debt_amounts",
    "planner_installment_amounts",
    "planner_months",
    "planner_opening",
    "planner_items",
    "subscriptions",
    "subscription_providers",
    "payment_plans",
    "installments",
    "transactions",
    "income_deductions",
    "incomes",
    "accounts",
    "payees",
    "categories",
];

pub async fn clear_data(pool: &SqlitePool, confirmation: &str) -> Result<(), String> {
    if confirmation != "DELETE ALL DATA" {
        return Err("Type DELETE ALL DATA to confirm.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for table in DATA_TABLES {
        // Identifiers are a fixed internal allowlist, never user input.
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("DELETE FROM institutions")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM logo_assets")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for statement in crate::database::INSTITUTIONS.lines() {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    for statement in crate::database::SUBSCRIPTION_PROVIDERS.lines() {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE settings SET currency = NULL, period_start_day = 1 WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}
