//! Financial SQL services. Callers acquire database::operations::operation for
//! the entire top-level operation, including imported-history checks and snapshot decoration.
//! Connection-level functions participate in the caller's transaction.
pub mod accounts;
pub mod card_billing;
pub mod card_limits;
pub mod cashflow;
pub mod income_deductions;
pub mod incomes;
pub mod installments;
pub mod institutions;
pub mod loans;
pub mod logos;
pub mod planning;
pub mod reconciliation;
pub mod reset;
pub mod salary_payments;
pub mod selective_defaults;
pub mod subscription_providers;
pub mod subscriptions;
pub mod transaction_options;
pub mod transactions;
