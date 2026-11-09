pub fn nonnegative_amount(s: &str) -> Result<i64, String> {
    s.parse::<i64>()
        .ok()
        .filter(|v| *v >= 0)
        .ok_or_else(|| "Enter a non-negative integer amount within the supported range.".into())
}

pub fn is_debt(kind: &str) -> bool {
    ["credit_card", "loan"].contains(&kind)
}
/// Economic value uses the opposite sign for amounts owed. i128 preserves i64::MIN.
pub fn account_value(kind: &str, balance: i64) -> i128 {
    if is_debt(kind) {
        -i128::from(balance)
    } else {
        i128::from(balance)
    }
}
/// An economic inflow increases assets and reduces debt. Wide deltas support edits.
pub fn adjust_balance(kind: &str, balance: i64, inflow: i128) -> Result<i64, String> {
    let balance = i128::from(balance);
    let next = if is_debt(kind) {
        balance.checked_sub(inflow)
    } else {
        balance.checked_add(inflow)
    };
    next.and_then(|v| i64::try_from(v).ok()).ok_or_else(|| {
        "This transaction would put an account balance outside the supported range.".into()
    })
}

/// Precision for currencies currently offered by the application.
/// Unknown currencies must be handled explicitly by the caller.
pub fn fraction_digits(currency: &str) -> Option<u32> {
    match currency {
        "JPY" | "KRW" | "VND" => Some(0),
        "KWD" | "BHD" => Some(3),
        "THB" | "USD" | "EUR" | "GBP" | "CNY" | "SGD" | "AUD" | "CAD" | "CHF" | "INR" => Some(2),
        _ => None,
    }
}

/// Convert decimal input exactly, rejecting excess precision rather than rounding.
/// Matches the frontend decimalToInteger helper for supported precision (0..=4).
pub fn decimal_to_integer(value: &str, digits: u32) -> Result<String, String> {
    let invalid = || format!("Enter a valid amount with up to {digits} decimal places.");
    if digits > 4 {
        return Err(invalid());
    }
    // ECMAScript String.trim whitespace, matching the frontend input parser.
    let value = value.trim_matches(|c: char| {
        matches!(c,
        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
    });

    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || parts.next().is_some()
        || fraction.is_some_and(|s| {
            s.is_empty() || s.len() > digits as usize || !s.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err(invalid());
    }
    let fraction = fraction.unwrap_or("");
    let combined = format!(
        "{whole}{fraction}{}",
        "0".repeat(digits as usize - fraction.len())
    );
    let significant = combined.trim_start_matches('0');
    if significant.is_empty() {
        return Ok("0".into());
    }
    let signed = format!("{}{significant}", if negative { "-" } else { "" });
    signed
        .parse::<i64>()
        .map(|n| n.to_string())
        .map_err(|_| "Amount is outside the supported range.".into())
}
