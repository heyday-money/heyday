use rust_xlsxwriter::{Format, Workbook};
use sqlx::{Row, SqlitePool};
use std::{io::Write, path::Path};
use tauri_plugin_dialog::DialogExt;

// Monetary columns are decimal text: Excel numbers only retain 15 significant
// digits, whereas our database accepts the full signed 64-bit minor-unit range.
fn decimal(value: i64, digits: u32) -> String {
    let value = i128::from(value);
    let scale = 10_i128.pow(digits);
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.abs();
    if digits == 0 {
        return format!("{sign}{absolute}");
    }
    format!(
        "{sign}{}.{:0width$}",
        absolute / scale,
        absolute % scale,
        width = digits as usize
    )
}

const HEADERS: &[&str] = &[
    "Account ID",
    "Name",
    "Type",
    "Status",
    "Currency",
    "Opening balance",
    "Current balance",
    "Institution",
    "Last four digits",
    "Loan type",
    "Individual card limit (saved)",
    "Shared limit group",
    "Shared group limit",
    "Revolving loan limit",
    "Statement day",
    "Payment due day",
    "Annual interest (%)",
    "Monthly installment",
    "Initial loan amount",
    "Paid off on",
    "Notes",
    "Created at (UTC)",
    "Updated at (UTC)",
];

async fn snapshot(pool: &SqlitePool) -> Result<(Option<String>, Vec<Vec<String>>), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let digits = match currency.as_deref() {
        Some("JPY" | "KRW" | "VND") => 0,
        Some("KWD" | "BHD") => 3,
        _ => 2,
    };
    let records = sqlx::query("SELECT a.*, p.paid_off_on, g.name AS group_name, g.credit_limit AS group_limit, f.credit_limit AS facility_limit FROM accounts a LEFT JOIN loan_payoffs p ON p.account_id=a.id LEFT JOIN card_limit_members m ON m.account_id=a.id LEFT JOIN card_limit_groups g ON g.id=m.group_id LEFT JOIN loan_facilities f ON f.account_id=a.id ORDER BY a.type, a.name COLLATE NOCASE, a.id")
        .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    let mut rows = Vec::with_capacity(records.len());
    for row in records {
        let text = |key: &str| -> Result<String, String> {
            row.try_get::<Option<String>, _>(key)
                .map(|v| v.unwrap_or_default())
                .map_err(|e| e.to_string())
        };
        let number = |key: &str, precision| -> Result<String, String> {
            row.try_get::<Option<i64>, _>(key)
                .map(|v| v.map(|v| decimal(v, precision)).unwrap_or_default())
                .map_err(|e| e.to_string())
        };
        let paid_off = text("paid_off_on")?;
        let archived: bool = row.try_get("is_archived").map_err(|e| e.to_string())?;
        rows.push(vec![
            text("id")?,
            text("name")?,
            text("type")?,
            if !paid_off.is_empty() {
                "Paid off"
            } else if archived {
                "Archived"
            } else {
                "Active"
            }
            .into(),
            currency.clone().unwrap_or_default(),
            number("opening_balance", digits)?,
            number("current_balance", digits)?,
            text("institution")?,
            text("last_four")?,
            text("loan_type")?,
            number("credit_limit", digits)?,
            text("group_name")?,
            number("group_limit", digits)?,
            number("facility_limit", digits)?,
            number("statement_day", 0)?,
            number("payment_due_day", 0)?,
            number("interest_rate_ten_thousandths", 4)?,
            number("monthly_installment", digits)?,
            number("initial_loan_amount", digits)?,
            paid_off,
            text("notes")?,
            text("created_at")?,
            text("updated_at")?,
        ]);
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((currency, rows))
}

fn workbook(
    currency: Option<&str>,
    rows: &[Vec<String>],
) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color("#EDE9FE")
        .set_text_wrap();
    let sheet = workbook.add_worksheet().set_name("Accounts")?;
    sheet.set_freeze_panes(1, 2)?;
    sheet.set_row_height(0, 32)?;
    for (column, title) in HEADERS.iter().enumerate() {
        sheet.set_column_width(column as u16, if column == 20 { 45 } else { 25 })?;
        sheet.write_string_with_format(0, column as u16, *title, &header)?;
    }
    for (index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            // Explicit strings prevent formula injection and preserve leading zeros.
            sheet.write_string((index + 1) as u32, column as u16, value)?;
        }
    }
    sheet.autofilter(0, 0, rows.len() as u32, (HEADERS.len() - 1) as u16)?;
    let notes = workbook.add_worksheet().set_name("Read me")?;
    notes.set_column_width(0, 110)?;
    let copy = [
        "Heyday Money — Account export".to_string(),
        format!("Currency: {}. Includes every active, archived and paid-off account from one database snapshot.", currency.unwrap_or("Not selected")),
        "All amounts are exact decimal text in currency units, not minor units. This avoids Excel's 15-digit numeric precision limit.".into(),
        "Positive cash, bank, wallet and investment balances are assets. Negative balances represent overdrafts or negative valuations.".into(),
        "Positive credit card and loan balances are amounts owed. Negative debt balances represent credits or overpayments.".into(),
        "Individual card limits remain saved but are inactive for grouped cards. Repeated shared group limits must not be summed.".into(),
        "Blank means not configured; zero is explicit. Annual interest is a percentage. Last four digits and IDs are text.".into(),
        "This export contains account details only, not transactions, schedules, logos or a restorable database. Use Backup & restore for a full backup.".into(),
    ];
    let wrapped = Format::new().set_text_wrap();
    for (row, text) in copy.iter().enumerate() {
        notes.write_string_with_format(
            row as u32,
            0,
            text,
            if row == 0 { &header } else { &wrapped },
        )?;
        notes.set_row_height(row as u32, 36)?;
    }
    workbook.save_to_buffer()
}

fn publish(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = tempfile::NamedTempFile::new_in(
        target
            .parent()
            .ok_or("Choose an export destination folder.")?,
    )
    .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(target).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn export_accounts_excel(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Option<String>, String> {
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Export accounts to Excel")
            .add_filter("Excel workbook", &["xlsx"])
            .set_file_name("heyday-money-accounts.xlsx")
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = selected else {
        return Ok(None);
    };
    let mut target = path.into_path().map_err(|e| e.to_string())?;
    if target.extension().is_none() {
        target.set_extension("xlsx");
    }
    if !target
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("xlsx"))
    {
        return Err("Choose an Excel filename ending in .xlsx.".into());
    }
    let _guard = heyday_core::database::operations::operation()?;
    crate::backups::protect_live_path(pool.inner(), &target)?;
    let (currency, rows) = snapshot(pool.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = workbook(currency.as_deref(), &rows).map_err(|e| e.to_string())?;
        publish(&target, &bytes)?;
        Ok(Some(target.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    fn xml(bytes: &[u8], name: &str) -> String {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    #[test]
    fn preserves_currency_precision_and_i64_boundaries() {
        assert_eq!(decimal(i64::MIN, 2), "-92233720368547758.08");
        assert_eq!(decimal(i64::MAX, 3), "9223372036854775.807");
        assert_eq!(decimal(9007199254740993, 2), "90071992547409.93");
        assert_eq!(decimal(12345, 0), "12345");
        assert_eq!(decimal(0, 3), "0.000");
    }

    #[tokio::test]
    async fn exports_all_statuses_and_shared_limits_without_mutation() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        sqlx::raw_sql("UPDATE settings SET currency='KWD'; INSERT INTO accounts(id,name,type,current_balance,is_archived,last_four) VALUES('a','=1+1','credit_card',-9007199254740993,1,'0001'),('b','Bank','bank',0,0,NULL),('c','Loan','loan',0,1,NULL); INSERT INTO loan_payoffs VALUES('c','2026-01-01'); INSERT INTO card_limit_groups VALUES('g','Shared','shared',12345); INSERT INTO card_limit_members VALUES('a','g');").execute(&pool).await.unwrap();
        let (currency, rows) = snapshot(&pool).await.unwrap();
        assert_eq!(rows.len(), 3);
        let card = rows.iter().find(|r| r[0] == "a").unwrap();
        assert_eq!(card[1], "=1+1");
        assert_eq!(card[3], "Archived");
        assert_eq!(card[6], "-9007199254740.993");
        assert_eq!(card[8], "0001");
        assert_eq!(card[12], "12.345");
        assert_eq!(rows.iter().find(|r| r[0] == "c").unwrap()[3], "Paid off");
        let bytes = workbook(currency.as_deref(), &rows).unwrap();
        let strings = xml(&bytes, "xl/sharedStrings.xml");
        assert!(strings.contains("=1+1"));
        assert!(strings.contains("0001"));
        assert!(strings.contains("-9007199254740.993"));
        let sheet = xml(&bytes, "xl/worksheets/sheet1.xml");
        assert!(!sheet.contains("<f>"));
        assert!(sheet.contains("state=\"frozen\""));
        assert!(sheet.contains("<autoFilter"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.xlsx");
        publish(&path, &bytes).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(snapshot(&pool).await.unwrap().1, rows);
    }

    #[tokio::test]
    async fn empty_database_exports_with_headers() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        let (currency, rows) = snapshot(&pool).await.unwrap();
        assert!(currency.is_none());
        assert!(rows.is_empty());
        assert!(workbook(None, &rows).is_ok());
    }
}
