//! Canonical schema and coordination shared by application adapters.
pub mod ledger;
pub mod operations;
pub mod settings;
pub use settings::Settings;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
pub const INSTITUTIONS: &str = include_str!("../../data/institutions.sql");
pub const SUBSCRIPTION_PROVIDERS: &str = include_str!("../../data/subscription-providers.sql");
pub const SYNC_METADATA_SEEDS: &str = include_str!("../../data/sync-metadata-seeds.json");
