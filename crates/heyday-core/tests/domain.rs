use heyday_core::domain::{dates, money, payroll, transactions};
use serde_json::Value;

#[test]
fn shared_frontend_vectors() {
    let vectors: Value = serde_json::from_str(include_str!("fixtures/domain.json")).unwrap();
    for case in vectors["decimal"].as_array().unwrap() {
        assert_eq!(
            money::decimal_to_integer(
                case["value"].as_str().unwrap(),
                case["digits"].as_u64().unwrap() as u32
            )
            .ok()
            .as_deref(),
            case["result"].as_str(),
            "{case}"
        );
    }
    for case in vectors["precision"].as_array().unwrap() {
        assert_eq!(
            money::fraction_digits(case["currency"].as_str().unwrap()),
            Some(case["digits"].as_u64().unwrap() as u32)
        );
    }
    for case in vectors["balances"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let balance = case["balance"].as_str().unwrap().parse().unwrap();
        let inflow = case["inflow"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            money::account_value(kind, balance).to_string(),
            case["value"].as_str().unwrap()
        );
        assert_eq!(
            money::adjust_balance(kind, balance, inflow)
                .ok()
                .map(|v| v.to_string())
                .as_deref(),
            case["next"].as_str(),
            "{case}"
        );
    }
    for case in vectors["periods"].as_array().unwrap() {
        let period = dates::current_period(
            case["day"].as_u64().unwrap() as u32,
            case["today"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(period.start, case["start"].as_str().unwrap());
        assert_eq!(period.end, case["end"].as_str().unwrap());
    }
    for case in vectors["payroll"].as_array().unwrap() {
        let n = |key: &str| case[key].as_str().unwrap().parse::<i64>().unwrap();
        assert_eq!(
            payroll::principal(n("total"), n("interest"), n("fee"))
                .ok()
                .map(|v| v.to_string())
                .as_deref(),
            case["principal"].as_str()
        );
        assert_eq!(
            payroll::net(n("gross"), n("total"))
                .ok()
                .map(|v| v.to_string())
                .as_deref(),
            case["net"].as_str()
        );
    }
}

#[test]
fn wide_edit_deltas_and_public_boundaries() {
    assert_eq!(
        money::adjust_balance(
            "bank",
            i64::MIN,
            i128::from(i64::MAX) - i128::from(i64::MIN)
        )
        .unwrap(),
        i64::MAX
    );
    assert!(money::adjust_balance("bank", 1, i128::MAX).is_err());
    assert!(money::adjust_balance("loan", 0, i128::MIN).is_err());
    assert!(payroll::add_deduction(i64::MAX, 1).is_err());
    assert_eq!(payroll::net(0, 0).unwrap(), 0);
    for value in ["-1", "9223372036854775808", "1.0", ""] {
        assert!(money::nonnegative_amount(value).is_err());
    }
    // Preserve Rust's existing accepted nonnegative integer syntax.
    assert_eq!(money::nonnegative_amount("+01").unwrap(), 1);
    assert!(dates::current_period(0, "2024-01-01").is_err());
    assert!(dates::current_period(31, "0001-01-01").is_err());
    assert!(dates::current_period(1, "9999-12-31").is_err());
    assert!(dates::current_period(1, "2024-02-30").is_err());
}

#[test]
fn transaction_wire_shape_and_validation_errors() {
    let mut input: transactions::NewTransaction = serde_json::from_value(serde_json::json!({
        "type": "expense", "account_id": "cash", "amount": "9007199254740993",
        "date": "2024-02-29", "description": "", "currency": "THB"
    }))
    .unwrap();
    assert!(input.cleared_account_ids.is_empty());
    assert_eq!(transactions::validate(&input).unwrap(), 9007199254740993);
    input.amount = "0".into();
    assert_eq!(
        transactions::validate(&input).unwrap_err(),
        "Amount must be greater than zero."
    );
    input.kind = "bad".into();
    assert_eq!(
        transactions::validate(&input).unwrap_err(),
        "Choose a transaction type."
    );
    input.kind = "transfer".into();
    input.amount = "1".into();
    assert_eq!(
        transactions::validate(&input).unwrap_err(),
        "Transfers and repayments need two different accounts."
    );
    input.destination_account_id = Some("cash".into());
    assert!(transactions::validate(&input).is_err());
    input.destination_account_id = Some("bank".into());
    input.description = "ก".repeat(200);
    assert!(transactions::validate(&input).is_ok());
    input.description.push('ก');
    assert_eq!(
        transactions::validate(&input).unwrap_err(),
        "Description must be no longer than 200 characters."
    );
}

#[test]
fn payroll_deduction_serialization_preserves_decimal_strings() {
    let value = serde_json::json!({"id":"d", "amount":"9007199254740993", "interest":"0", "fee":"1", "debt_account_id":"loan", "contract_id":null});
    let deduction: payroll::Deduction = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(deduction).unwrap(), value);
}
