use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Deduction {
    pub id: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
    pub interest: String,
    pub fee: String,
    pub contract_id: Option<String>,
}
#[derive(Deserialize)]
pub struct PaymentInput {
    pub income_id: String,
    pub destination_account_id: String,
    pub occurrence: String,
    pub date: String,
    pub gross: String,
    pub currency: String,
    pub deductions: Vec<Deduction>,
    pub confirmed: bool,
}
/// Inputs are validated nonnegative minor-unit amounts.
pub fn principal(value: i64, interest: i64, fee: i64) -> Result<i64, String> {
    value
        .checked_sub(interest)
        .and_then(|v| v.checked_sub(fee))
        .filter(|v| *v >= 0)
        .ok_or_else(|| "Interest and fees cannot exceed the deduction.".into())
}
pub fn add_deduction(total: i64, value: i64) -> Result<i64, String> {
    total
        .checked_add(value)
        .ok_or_else(|| "Deductions exceed the supported range.".into())
}
pub fn net(gross: i64, total: i64) -> Result<i64, String> {
    gross
        .checked_sub(total)
        .filter(|v| *v >= 0)
        .ok_or_else(|| "Total deductions cannot exceed gross salary.".into())
}

/// Validate confirmation, dates and gross before opening a database transaction.
pub fn validate(input: &PaymentInput) -> Result<i64, String> {
    if !input.confirmed {
        return Err("Confirm the salary payment before saving.".into());
    }
    if !super::dates::valid_date(&format!("{}-01", input.occurrence))
        || !super::dates::valid_date(&input.date)
    {
        return Err("Choose a valid salary month and payment date.".into());
    }
    let gross = super::money::nonnegative_amount(&input.gross)?;
    Ok(gross)
}
