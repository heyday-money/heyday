use super::dates::valid_date;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct NewTransaction {
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub destination_account_id: Option<String>,
    pub amount: String,
    pub date: String,
    pub description: String,
    pub currency: String,
    pub payee_id: Option<String>,
    pub category_id: Option<String>,
    pub income_source_id: Option<String>,
    #[serde(default)]
    pub cleared_account_ids: Vec<String>,
}

/// Validate input-only rules; references, currency and the clock stay with the caller.
pub fn validate(input: &NewTransaction) -> Result<i64, String> {
    if !["income", "expense", "transfer", "repayment"].contains(&input.kind.as_str()) {
        return Err("Choose a transaction type.".into());
    }
    let amount = input
        .amount
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if amount <= 0 {
        return Err("Amount must be greater than zero.".into());
    }
    if !valid_date(&input.date) {
        return Err("Enter a valid date.".into());
    }
    let description = input.description.trim();
    if description.chars().count() > 200 {
        return Err("Description must be no longer than 200 characters.".into());
    }
    let paired = ["transfer", "repayment"].contains(&input.kind.as_str());
    if paired != input.destination_account_id.is_some()
        || input.destination_account_id.as_ref() == Some(&input.account_id)
    {
        return Err("Transfers and repayments need two different accounts.".into());
    }
    Ok(amount)
}
