//! Tracking accounting contract. Persistence, IPC commands and sync admission are
//! deliberately separate: these calculations alone do not enable a new account type.
use super::{dates::valid_date, money::fraction_digits};
use serde::{Deserialize, Serialize};

pub const ACCOUNT_TYPE: &str = "tracking";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackingSubtype {
    Receivable,
    Vehicle,
    Property,
    InsuranceCashValue,
    OtherAsset,
}

/// Draft metadata, not a persisted balance or a sync record. The database service
/// must separately resolve logo/loan references and enforce ownership and currency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrackingDraft {
    pub name: String,
    pub subtype: TrackingSubtype,
    pub currency: String,
    pub opening_value: String,
    pub opening_date: String,
    pub notes: Option<String>,
    pub logo_asset_id: Option<String>,
    pub debtor: Option<String>,
    pub due_date: Option<String>,
    pub linked_loan_id: Option<String>,
}

fn amount(value: &str) -> Result<i64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Enter a non-negative integer amount.".into());
    }
    value
        .parse()
        .map_err(|_| "Amount is outside the supported range.".into())
}

fn optional_text(value: Option<String>, max: usize) -> Result<Option<String>, String> {
    let value = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    if value.as_ref().is_some_and(|v| v.chars().count() > max) {
        return Err(format!("Text must be no longer than {max} characters."));
    }
    Ok(value)
}

impl TrackingDraft {
    pub fn validate(mut self, configured_currency: Option<&str>) -> Result<Self, String> {
        if configured_currency != Some(self.currency.as_str())
            || fraction_digits(&self.currency).is_none()
        {
            return Err("Choose the app currency in Settings, then reload before saving.".into());
        }
        self.name = optional_text(Some(self.name), 100)?.ok_or("Enter an account name.")?;
        self.notes = optional_text(self.notes, 1000)?;
        self.debtor = optional_text(self.debtor, 100)?;
        self.logo_asset_id = optional_text(self.logo_asset_id, 200)?;
        self.linked_loan_id = optional_text(self.linked_loan_id, 100)?;
        self.opening_value = amount(&self.opening_value)?.to_string();
        if !valid_date(&self.opening_date)
            || self.due_date.as_deref().is_some_and(|d| !valid_date(d))
        {
            return Err("Enter a valid date.".into());
        }
        // An already overdue receivable may legitimately be entered today.
        if self.subtype != TrackingSubtype::Receivable
            && (self.debtor.is_some() || self.due_date.is_some())
        {
            return Err("Debtor and due date apply only to receivables.".into());
        }
        if !matches!(
            self.subtype,
            TrackingSubtype::Vehicle | TrackingSubtype::Property | TrackingSubtype::OtherAsset
        ) && self.linked_loan_id.is_some()
        {
            return Err("A linked loan applies only to a financed asset.".into());
        }
        Ok(self)
    }

    /// Opening value/date are historical inputs, not editable metadata. Use events
    /// for later changes; storage must additionally reject a changed account type.
    pub fn validate_edit(
        self,
        original: &Self,
        configured_currency: Option<&str>,
    ) -> Result<Self, String> {
        let next = self.validate(configured_currency)?;
        if next.subtype != original.subtype
            || next.currency != original.currency
            || amount(&next.opening_value)? != amount(&original.opening_value)?
            || next.opening_date != original.opening_date
        {
            return Err(
                "Tracking subtype, currency and opening value/date cannot be changed.".into(),
            );
        }
        Ok(next)
    }
}

/// Proposed actions are calculation inputs, not database events. Persistence must
/// provide revision checks, dates, idempotency and atomic linked ledger writes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TrackingAction {
    Value { value: String },
    Lend { principal: String },
    Receive { principal: String, interest: String },
    WriteOff { principal: String },
    Buy { cost: String },
    Sell { proceeds: String },
}

/// Positive cash delta is an inflow. Gains are signed. All serialized amounts are
/// strings, including deltas, so values above JavaScript's safe integer stay exact.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TrackingEffect {
    pub next_value: String,
    pub cash_delta: String,
    pub income: String,
    pub expense: String,
    pub valuation_gain: String,
    pub realized_gain: String,
}

pub fn calculate(
    subtype: TrackingSubtype,
    current_value: &str,
    archived: bool,
    action: &TrackingAction,
) -> Result<TrackingEffect, String> {
    if archived {
        return Err("Restore this account before recording changes.".into());
    }
    let current = i128::from(amount(current_value)?);
    let positive = |v: &str| -> Result<i128, String> {
        let v = amount(v)?;
        if v == 0 {
            return Err("Amount must be greater than zero.".into());
        }
        Ok(i128::from(v))
    };
    let receivable = subtype == TrackingSubtype::Receivable;
    let physical = matches!(
        subtype,
        TrackingSubtype::Vehicle | TrackingSubtype::Property | TrackingSubtype::OtherAsset
    );
    let (next, cash, income, valuation, realized) = match action {
        TrackingAction::Value { value } if !receivable => {
            let next = i128::from(amount(value)?);
            (next, 0, 0, next - current, 0)
        }
        TrackingAction::Lend { principal } if receivable => {
            let p = positive(principal)?;
            (current + p, -p, 0, 0, 0)
        }
        TrackingAction::Receive {
            principal,
            interest,
        } if receivable => {
            let p = i128::from(amount(principal)?);
            let i = i128::from(amount(interest)?);
            if p + i == 0 {
                return Err("Receipt must be greater than zero.".into());
            }
            (current - p, p + i, i, 0, 0)
        }
        TrackingAction::WriteOff { principal } if receivable => {
            let p = positive(principal)?;
            (current - p, 0, 0, 0, -p)
        }
        TrackingAction::Buy { cost } if physical && current == 0 => {
            let cost = positive(cost)?;
            (cost, -cost, 0, 0, 0)
        }
        // A zero-proceeds disposal is valid. Partial disposal and fees need their
        // own allocation design. Insurance surrender is a separate future action.
        TrackingAction::Sell { proceeds } if physical && current > 0 => {
            let proceeds = i128::from(amount(proceeds)?);
            (0, proceeds, 0, 0, proceeds - current)
        }
        _ => return Err("This action is not supported for this Tracking account.".into()),
    };
    if next < 0 {
        return Err("Principal cannot exceed the outstanding receivable.".into());
    }
    if i64::try_from(next).is_err() || i64::try_from(cash).is_err() {
        return Err("Amount is outside the supported range.".into());
    }
    Ok(TrackingEffect {
        next_value: next.to_string(),
        cash_delta: cash.to_string(),
        income: income.to_string(),
        expense: "0".into(),
        valuation_gain: valuation.to_string(),
        realized_gain: realized.to_string(),
    })
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TrackingSummary {
    pub net_worth_asset: String,
    pub spendable_cash: String,
    pub cash_forecast_balance: String,
    /// Informational only: do not add equity to Net Worth as another asset.
    pub equity: Option<String>,
}

/// The loan remains counted by the existing liability totals, exactly once. Its
/// signed balance can be negative (a lender credit); i128 preserves that edge case.
pub fn summarize(
    value: &str,
    archived: bool,
    linked_loan_balance: Option<i64>,
) -> Result<TrackingSummary, String> {
    let value = i128::from(amount(value)?);
    Ok(TrackingSummary {
        net_worth_asset: if archived {
            "0".into()
        } else {
            value.to_string()
        },
        spendable_cash: "0".into(),
        cash_forecast_balance: "0".into(),
        equity: linked_loan_balance.map(|debt| (value - i128::from(debt)).to_string()),
    })
}
