use heyday_core::domain::tracking::{
    calculate, summarize, TrackingAction as Action, TrackingDraft, TrackingSubtype as Kind,
};
use serde_json::json;

fn draft(kind: Kind) -> TrackingDraft {
    TrackingDraft {
        name: " Car ".into(),
        subtype: kind,
        currency: "THB".into(),
        opening_value: "60000000".into(),
        opening_date: "2026-11-15".into(),
        notes: None,
        logo_asset_id: None,
        debtor: None,
        due_date: None,
        linked_loan_id: None,
    }
}

#[test]
fn metadata_currency_and_immutable_fields() {
    let original = draft(Kind::Vehicle).validate(Some("THB")).unwrap();
    assert_eq!(original.name, "Car");
    assert!(draft(Kind::Vehicle).validate(None).is_err());
    assert!(draft(Kind::Vehicle).validate(Some("USD")).is_err());
    let mut edit = original.clone();
    edit.name = "Renamed".into();
    edit.notes = Some("Updated notes".into());
    assert!(edit.validate_edit(&original, Some("THB")).is_ok());
    for change in 0..4 {
        let mut edit = original.clone();
        match change {
            0 => edit.subtype = Kind::Property,
            1 => edit.opening_value = "1".into(),
            2 => edit.opening_date = "2026-11-14".into(),
            _ => edit.currency = "USD".into(),
        }
        assert!(edit.validate_edit(&original, Some("THB")).is_err());
    }
}

#[test]
fn validate_subtype_fields_and_dates() {
    let mut input = draft(Kind::Receivable);
    input.debtor = Some("Friend".into());
    input.due_date = Some("2026-10-01".into()); // Existing overdue debt is allowed.
    assert!(input.clone().validate(Some("THB")).is_ok());
    input.subtype = Kind::Vehicle;
    assert!(input.validate(Some("THB")).is_err());
    for kind in [Kind::Receivable, Kind::InsuranceCashValue] {
        let mut input = draft(kind);
        input.linked_loan_id = Some("loan-id".into());
        assert!(input.validate(Some("THB")).is_err());
    }
    for date in ["2026-02-29", "2026-11-31", "2026-1-01"] {
        let mut input = draft(Kind::Vehicle);
        input.opening_date = date.into();
        assert!(input.validate(Some("THB")).is_err());
    }
}

#[test]
fn lending_and_partial_repayment_preserve_net_worth() {
    let lend = calculate(
        Kind::Receivable,
        "0",
        false,
        &Action::Lend {
            principal: "1000000".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            lend.next_value.as_str(),
            lend.cash_delta.as_str(),
            lend.income.as_str()
        ),
        ("1000000", "-1000000", "0")
    );
    let repay = calculate(
        Kind::Receivable,
        &lend.next_value,
        false,
        &Action::Receive {
            principal: "200000".into(),
            interest: "0".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            repay.next_value.as_str(),
            repay.cash_delta.as_str(),
            repay.income.as_str()
        ),
        ("800000", "200000", "0")
    );
    let opening_cash = 2_000_000i64;
    assert_eq!(
        opening_cash
            + lend.cash_delta.parse::<i64>().unwrap()
            + repay.cash_delta.parse::<i64>().unwrap()
            + repay.next_value.parse::<i64>().unwrap(),
        opening_cash
    );
}

#[test]
fn interest_is_income_and_writeoff_is_a_separate_loss() {
    let repay = calculate(
        Kind::Receivable,
        "1000",
        false,
        &Action::Receive {
            principal: "200".into(),
            interest: "50".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            repay.next_value.as_str(),
            repay.cash_delta.as_str(),
            repay.income.as_str()
        ),
        ("800", "250", "50")
    );
    let loss = calculate(
        Kind::Receivable,
        "800",
        false,
        &Action::WriteOff {
            principal: "800".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            loss.next_value.as_str(),
            loss.cash_delta.as_str(),
            loss.expense.as_str(),
            loss.realized_gain.as_str()
        ),
        ("0", "0", "0", "-800")
    );
    for action in [
        Action::Receive {
            principal: "801".into(),
            interest: "0".into(),
        },
        Action::WriteOff {
            principal: "801".into(),
        },
    ] {
        assert!(calculate(Kind::Receivable, "800", false, &action).is_err());
    }
}

#[test]
fn valuation_is_non_cash_and_equity_is_informational() {
    let effect = calculate(
        Kind::Vehicle,
        "60000000",
        false,
        &Action::Value {
            value: "55000000".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            effect.cash_delta.as_str(),
            effect.income.as_str(),
            effect.expense.as_str(),
            effect.valuation_gain.as_str()
        ),
        ("0", "0", "0", "-5000000")
    );
    let summary = summarize(&effect.next_value, false, Some(35000000)).unwrap();
    assert_eq!(
        (
            summary.net_worth_asset.as_str(),
            summary.equity.as_deref(),
            summary.spendable_cash.as_str(),
            summary.cash_forecast_balance.as_str()
        ),
        ("55000000", Some("20000000"), "0", "0")
    );
    let archived = summarize(&effect.next_value, true, Some(35000000)).unwrap();
    assert_eq!(archived.net_worth_asset, "0");
    assert_eq!(archived.equity, summary.equity); // History/detail remains visible.
    assert!(calculate(
        Kind::Vehicle,
        "55000000",
        true,
        &Action::Value { value: "1".into() }
    )
    .is_err());
}

#[test]
fn purchase_sale_and_zero_proceeds_disposal() {
    let bought = calculate(
        Kind::Vehicle,
        "0",
        false,
        &Action::Buy {
            cost: "60000000".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (bought.cash_delta.as_str(), bought.expense.as_str()),
        ("-60000000", "0")
    );
    let sold = calculate(
        Kind::Vehicle,
        "55000000",
        false,
        &Action::Sell {
            proceeds: "52000000".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (
            sold.next_value.as_str(),
            sold.cash_delta.as_str(),
            sold.income.as_str(),
            sold.realized_gain.as_str()
        ),
        ("0", "52000000", "0", "-3000000")
    );
    let disposed = calculate(
        Kind::OtherAsset,
        "500",
        false,
        &Action::Sell {
            proceeds: "0".into(),
        },
    )
    .unwrap();
    assert_eq!(disposed.realized_gain, "-500");
    assert!(calculate(
        Kind::Vehicle,
        "0",
        false,
        &Action::Sell {
            proceeds: "1".into()
        }
    )
    .is_err());
}

#[test]
fn insurance_value_is_manual_and_receivables_cannot_be_revalued() {
    assert!(calculate(
        Kind::Receivable,
        "100",
        false,
        &Action::Value {
            value: "200".into()
        }
    )
    .is_err());
    let effect = calculate(
        Kind::InsuranceCashValue,
        "100",
        false,
        &Action::Value {
            value: "150".into(),
        },
    )
    .unwrap();
    assert_eq!(
        (effect.next_value.as_str(), effect.cash_delta.as_str()),
        ("150", "0")
    );
    for action in [
        Action::Buy { cost: "50".into() },
        Action::Lend {
            principal: "50".into(),
        },
        Action::Sell {
            proceeds: "150".into(),
        },
    ] {
        assert!(calculate(Kind::InsuranceCashValue, "100", false, &action).is_err());
    }
}

#[test]
fn strict_string_amounts_and_overflow() {
    for value in ["-1", "+1", " 1", "1.0", "1e3", "9223372036854775808", ""] {
        assert!(calculate(
            Kind::Vehicle,
            "0",
            false,
            &Action::Value {
                value: value.into()
            }
        )
        .is_err());
    }
    assert!(calculate(
        Kind::Receivable,
        &i64::MAX.to_string(),
        false,
        &Action::Lend {
            principal: "1".into()
        }
    )
    .is_err());
    assert!(calculate(
        Kind::Receivable,
        &i64::MAX.to_string(),
        false,
        &Action::Receive {
            principal: i64::MAX.to_string(),
            interest: "1".into()
        }
    )
    .is_err());
    let effect = calculate(
        Kind::Vehicle,
        "0",
        false,
        &Action::Value {
            value: "9007199254740993".into(),
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(effect).unwrap()["next_value"],
        json!("9007199254740993")
    );
    assert!(serde_json::from_value::<Action>(
        json!({"kind":"value", "value": 9007199254740993u64})
    )
    .is_err());
    assert!(serde_json::from_value::<Kind>(json!("unknown")).is_err());
    assert_eq!(
        summarize(&i64::MAX.to_string(), false, Some(i64::MIN))
            .unwrap()
            .equity
            .as_deref(),
        Some("18446744073709551615")
    );
}
