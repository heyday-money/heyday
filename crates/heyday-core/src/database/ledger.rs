//! Canonical ledger entry types shared by sync and financial services.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const DATABASE: &str = "Could not update local sync state. Please try again.";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub destination_account_id: Option<String>,
    pub amount: String,
    pub date: String,
    pub description: String,
    pub payee: Option<Label>,
    pub category_id: Option<String>,
    pub income_source: Option<Label>,
    pub loan_account_id: Option<String>,
    pub salary_role: Option<String>,
    pub loan_component: Option<String>,
}

pub type Entries = BTreeMap<String, (bool, Entry)>;
