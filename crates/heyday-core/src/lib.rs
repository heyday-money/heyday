//! Platform-independent financial rules for Heyday.Money.
//! Shared database services with desktop-owned commands and platform adapters.

pub mod domain;

pub mod database;
pub mod services;
pub use database::Settings;
// Preserve cross-service paths while extracting existing implementations.
pub(crate) use services::{
    accounts, card_billing, income_deductions, incomes, installments, institutions, loans, logos,
    reconciliation, salary_payments, selective_defaults, subscriptions, transaction_options,
    transactions,
};

pub mod crypto;
pub mod sync;
pub(crate) use crypto::encryption as cloud_crypto;
pub(crate) use crypto::key_protocol as cloud_key_protocol;
pub(crate) use services::cashflow;
pub(crate) use sync::income as cloud_sync_income;
pub(crate) use sync::ledger as cloud_sync_ledger;
pub(crate) use sync::loans as cloud_sync_loans;
pub(crate) use sync::metadata as cloud_sync_metadata;
pub(crate) use sync::metadata_rules as cloud_sync_metadata_rules;
pub(crate) use sync::planner as cloud_sync_planner;
pub(crate) use sync::protocol as cloud_sync_protocol;
pub(crate) use sync::store as cloud_sync_store;
