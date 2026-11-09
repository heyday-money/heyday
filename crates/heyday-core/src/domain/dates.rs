pub fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap();
    let month: usize = date[5..7].parse().unwrap();
    let day: u32 = date[8..].parse().unwrap();
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day >= 1 && day <= days[month - 1]
}

/// Monthly occurrence clamped independently from the original date.
/// Caller must supply a valid YYYY-MM-DD date and an offset within years 1..=9999.
pub fn scheduled_date(first: &str, offset: i64) -> String {
    let y: i64 = first[..4].parse().unwrap();
    let m: i64 = first[5..7].parse().unwrap();
    let day: i64 = first[8..].parse().unwrap();
    let index = y * 12 + m - 1 + offset;
    let y = index / 12;
    let m = index % 12 + 1;
    let max = match m {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 31,
    };
    format!("{y:04}-{m:02}-{:02}", day.min(max))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_validation() {
        for date in ["2024-02-29", "2000-02-29", "2025-01-31", "2025-04-30"] {
            assert!(valid_date(date));
        }
        for date in [
            "1900-02-29",
            "2025-02-29",
            "2024-04-31",
            "0000-01-01",
            "2025-13-01",
            "2025-01-00",
            "2025-1-01",
            "abcd-ef-gh",
            "😀2024-01",
        ] {
            assert!(!valid_date(date));
        }
    }
    #[test]
    fn independently_clamped_dates() {
        assert_eq!(scheduled_date("2024-01-31", 1), "2024-02-29");
        assert_eq!(scheduled_date("2024-01-31", 2), "2024-03-31");
        assert_eq!(scheduled_date("2023-01-31", 1), "2023-02-28");
    }
}

/// Inclusive start and exclusive next-start, as local calendar dates (no clock or timezone).
#[derive(Debug, PartialEq, Eq)]
pub struct Period {
    pub start: String,
    pub end: String,
}

/// Payday cycle containing an explicit local date; clamp each month independently.
pub fn current_period(day: u32, today: &str) -> Result<Period, String> {
    if !(1..=31).contains(&day) {
        return Err("Invalid period start day".into());
    }
    if !valid_date(today) {
        return Err("Enter a valid date.".into());
    }
    let year: i64 = today[..4].parse().unwrap();
    let month: i64 = today[5..7].parse().unwrap();
    let boundary = |index: i64| -> Result<String, String> {
        let year = index / 12;
        let month = index % 12 + 1;
        if !(1..=9999).contains(&year) {
            return Err("Period is outside the supported date range.".into());
        }
        // January always contains the configured day. Offset from it, not from a clamped month.
        Ok(scheduled_date(&format!("{year:04}-01-{day:02}"), month - 1))
    };
    let mut index = year * 12 + month - 1;
    if today < boundary(index)?.as_str() {
        index -= 1;
    }
    Ok(Period {
        start: boundary(index)?,
        end: boundary(index + 1)?,
    })
}
