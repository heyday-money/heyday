use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Account {
    id: String,
    name: String,
    #[serde(rename = "type")]
    account_type: String,
    loan_type: Option<String>,
    // Amounts cross IPC as decimal integer strings, never lossy JS numbers.
    opening_balance: String,
    current_balance: String,
    institution: Option<String>,
    last_four: Option<String>,
    notes: Option<String>,
    credit_limit: Option<String>,
    statement_day: Option<i64>,
    payment_due_day: Option<i64>,
    interest_rate_bps: Option<i64>,
}

pub(crate) const COLUMNS: &str = "id, name, type AS account_type, loan_type, CAST(opening_balance AS TEXT) AS opening_balance, CAST(current_balance AS TEXT) AS current_balance, institution, last_four, notes, CAST(credit_limit AS TEXT) AS credit_limit, statement_day, payment_due_day, interest_rate_bps";

#[derive(Deserialize)]
pub struct NewAccount {
    name: String,
    #[serde(rename = "type")]
    account_type: String,
    loan_type: Option<String>,
    opening_balance: String,
    currency: String,
    institution: Option<String>,
    last_four: Option<String>,
    notes: Option<String>,
    credit_limit: Option<String>,
    statement_day: Option<i64>,
    payment_due_day: Option<i64>,
    interest_rate_bps: Option<i64>,
}

fn optional_text(value: Option<String>, limit: usize) -> Result<Option<String>, String> {
    let value = value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if value.as_ref().is_some_and(|s| s.chars().count() > limit) {
        return Err(format!("Text must be no longer than {limit} characters."));
    }
    Ok(value)
}

fn amount(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| "Amount is outside the supported range.".into())
}

pub async fn insert_account(pool: &SqlitePool, input: NewAccount) -> Result<Account, String> {
    let kind = input.account_type.as_str();
    if !["cash", "bank", "credit_card", "loan", "investment"].contains(&kind) {
        return Err("Choose a supported account type.".into());
    }
    if kind == "loan" {
        if !input.loan_type.as_deref().is_some_and(|value| {
            [
                "mortgage",
                "auto_loan",
                "student_loan",
                "personal_loan",
                "medical_debt",
                "other_debt",
            ]
            .contains(&value)
        }) {
            return Err("Choose a supported loan type.".into());
        }
    } else if input.loan_type.is_some() {
        return Err("Loan type applies only to loan accounts.".into());
    }
    let name = optional_text(Some(input.name), 100)?.ok_or("Enter an account name.")?;
    let institution = optional_text(input.institution, 100)?;
    let notes = optional_text(input.notes, 1000)?;
    let last_four = optional_text(input.last_four, 4)?;
    if let Some(last) = &last_four {
        if !["bank", "credit_card"].contains(&kind)
            || last.len() != 4
            || !last.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Last four digits must contain exactly four digits for a bank or credit card account.".into());
        }
    }
    let balance = amount(&input.opening_balance)?;
    // Signed balances are allowed: negative debt represents a credit/overpayment.
    let limit = input.credit_limit.as_deref().map(amount).transpose()?;
    if limit.is_some_and(|v| v < 0)
        || (kind != "credit_card" && (limit.is_some() || input.statement_day.is_some()))
    {
        return Err(
            "Only credit cards can have a non-negative credit limit and statement day.".into(),
        );
    }
    let debt = ["credit_card", "loan"].contains(&kind);
    if !debt && (input.payment_due_day.is_some() || input.interest_rate_bps.is_some()) {
        return Err(
            "Payment schedules and interest rates apply only to credit cards and loans.".into(),
        );
    }
    if [input.statement_day, input.payment_due_day]
        .into_iter()
        .flatten()
        .any(|d| !(1..=31).contains(&d))
    {
        return Err("Scheduled days must be between 1 and 31.".into());
    }
    if input
        .interest_rate_bps
        .is_some_and(|v| !(0..=10000).contains(&v))
    {
        return Err("Annual interest rate must be between 0 and 100%.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err(
            "Choose the app currency in Settings, then reload accounts before saving.".into(),
        );
    }
    let query = format!("INSERT INTO accounts (id, name, type, opening_balance, current_balance, institution, last_four, notes, credit_limit, statement_day, payment_due_day, interest_rate_bps, loan_type) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING {COLUMNS}");
    let account = sqlx::query_as::<_, Account>(&query)
        .bind(name)
        .bind(kind)
        .bind(balance)
        .bind(balance)
        .bind(institution)
        .bind(last_four)
        .bind(notes)
        .bind(limit)
        .bind(input.statement_day)
        .bind(input.payment_due_day)
        .bind(input.interest_rate_bps)
        .bind(input.loan_type)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
pub async fn create_account(
    pool: tauri::State<'_, SqlitePool>,
    input: NewAccount,
) -> Result<Account, String> {
    insert_account(pool.inner(), input).await
}

#[tauri::command]
pub async fn list_accounts(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Account>, String> {
    sqlx::query_as::<_, Account>(&format!(
        "SELECT {COLUMNS} FROM accounts WHERE is_archived = 0 ORDER BY created_at, id"
    ))
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn database() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }
    fn input(kind: &str) -> NewAccount {
        NewAccount {
            name: " Test account ".into(),
            account_type: kind.into(),
            loan_type: (kind == "loan").then(|| "other_debt".into()),
            currency: "THB".into(),
            opening_balance: "9007199254740993".into(),
            institution: None,
            last_four: None,
            notes: None,
            credit_limit: None,
            statement_day: None,
            payment_due_day: None,
            interest_rate_bps: None,
        }
    }
    #[tokio::test]
    async fn loan_classifications_are_persisted_and_validated() {
        let pool = database().await;
        crate::save_currency(&pool, "THB").await.unwrap();
        for kind in [
            "mortgage",
            "auto_loan",
            "student_loan",
            "personal_loan",
            "medical_debt",
            "other_debt",
        ] {
            let mut new = input("loan");
            new.loan_type = Some(kind.into());
            let created = insert_account(&pool, new).await.unwrap();
            let stored: Account =
                sqlx::query_as(&format!("SELECT {COLUMNS} FROM accounts WHERE id = ?"))
                    .bind(created.id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(stored.loan_type.as_deref(), Some(kind));
            assert_eq!(stored.current_balance, "9007199254740993");
        }
        for classification in [None, Some("invalid"), Some("")] {
            let mut new = input("loan");
            new.loan_type = classification.map(str::to_owned);
            assert!(insert_account(&pool, new).await.is_err());
        }
        for kind in ["cash", "bank", "credit_card", "investment"] {
            let mut new = input(kind);
            new.loan_type = Some("mortgage".into());
            assert!(insert_account(&pool, new).await.is_err());
        }
        assert!(sqlx::query("UPDATE accounts SET loan_type = 'invalid'")
            .execute(&pool)
            .await
            .is_err());
        assert!(sqlx::query("UPDATE accounts SET type = 'bank'")
            .execute(&pool)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn loan_migration_preserves_existing_records() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in [
            include_str!("../migrations/0001_initial.sql"),
            include_str!("../migrations/0002_account_details.sql"),
            include_str!("../migrations/0003_transactions.sql"),
            include_str!("../migrations/0004_optional_transaction_description.sql"),
            include_str!("../migrations/0005_payees_and_categories.sql"),
            include_str!("../migrations/0006_payment_plans.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance, is_archived) VALUES ('old', 'Old loan', 'loan', 1234, 1000, 1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO incomes (id, name, destination_account_id, type, estimated_amount, recurrence_day_of_month) VALUES ('income', 'Income', 'old', 'other', 100, 1)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0007_loan_types.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let stored: Account =
            sqlx::query_as(&format!("SELECT {COLUMNS} FROM accounts WHERE id = 'old'"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored.loan_type, None);
        assert_eq!(stored.opening_balance, "1234");
        assert_eq!(stored.current_balance, "1000");
        let row: (i64, String) = sqlx::query_as("SELECT a.is_archived, i.destination_account_id FROM accounts a JOIN incomes i ON a.id = i.destination_account_id").fetch_one(&pool).await.unwrap();
        assert_eq!(row, (1, "old".into()));
    }

    #[tokio::test]
    async fn create_all_types_and_lock_currency() {
        let pool = database().await;
        assert!(insert_account(&pool, input("cash")).await.is_err());
        assert!(crate::save_currency(&pool, "INVALID").await.is_err());
        crate::save_currency(&pool, "THB").await.unwrap();
        for kind in ["cash", "bank", "credit_card", "loan", "investment"] {
            let mut new = input(kind);
            if kind == "credit_card" {
                new.last_four = Some("0123".into());
                new.credit_limit = Some("10000000".into());
                new.statement_day = Some(31);
                new.payment_due_day = Some(15);
                new.interest_rate_bps = Some(1250);
            }
            let result = insert_account(&pool, new).await.unwrap();
            assert_eq!(result.name, "Test account");
            assert_eq!(result.opening_balance, "9007199254740993");
            if kind == "credit_card" {
                assert_eq!(result.last_four.as_deref(), Some("0123"));
            }
        }
        assert!(crate::save_currency(&pool, "USD").await.is_err());
        crate::save_currency(&pool, "THB").await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 5);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM incomes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    #[tokio::test]
    async fn rejects_invalid_fields_without_partial_records() {
        let pool = database().await;
        crate::save_currency(&pool, "THB").await.unwrap();
        let mut cases = vec![];
        let mut value = input("cash");
        value.name = "  ".into();
        cases.push(value);
        let mut value = input("cash");
        value.credit_limit = Some("100".into());
        cases.push(value);
        let mut value = input("investment");
        value.payment_due_day = Some(5);
        cases.push(value);
        let mut value = input("credit_card");
        value.statement_day = Some(32);
        cases.push(value);
        let mut value = input("credit_card");
        value.last_four = Some("abcd".into());
        cases.push(value);
        let mut value = input("credit_card");
        value.credit_limit = Some("-1".into());
        cases.push(value);
        let mut value = input("loan");
        value.interest_rate_bps = Some(10001);
        cases.push(value);
        let mut value = input("cash");
        value.opening_balance = "9223372036854775808".into();
        cases.push(value);
        let mut value = input("cash");
        value.currency = "JPY".into();
        cases.push(value);
        cases.push(input("unknown"));
        for value in cases {
            assert!(insert_account(&pool, value).await.is_err());
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let mut value = input("loan");
        value.opening_balance = "-100".into();
        assert_eq!(
            insert_account(&pool, value).await.unwrap().opening_balance,
            "-100"
        );
    }
    #[tokio::test]
    async fn account_survives_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "heyday-accounts-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        crate::save_currency(&pool, "THB").await.unwrap();
        let created = insert_account(&pool, input("investment")).await.unwrap();
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        sqlx::migrate!("./migrations").run(&reopened).await.unwrap();
        let stored: Account =
            sqlx::query_as(&format!("SELECT {COLUMNS} FROM accounts WHERE id = ?"))
                .bind(&created.id)
                .fetch_one(&reopened)
                .await
                .unwrap();
        assert_eq!(stored.opening_balance, created.opening_balance);
        assert_eq!(stored.account_type, "investment");
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn upgrade_preserves_balances_and_income_references() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('old', 'Old account', 'cash', 1234)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO incomes (id, name, destination_account_id, type, estimated_amount, recurrence_day_of_month) VALUES ('salary', 'Salary', 'old', 'salary', 100, 1)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0002_account_details.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let row: (i64, String) = sqlx::query_as("SELECT a.opening_balance, i.destination_account_id FROM accounts a JOIN incomes i ON a.id = i.destination_account_id").fetch_one(&pool).await.unwrap();
        assert_eq!(row, (1234, "old".into()));
        assert!(
            sqlx::query("UPDATE accounts SET credit_limit = 100 WHERE id = 'old'")
                .execute(&pool)
                .await
                .is_err()
        );
    }
}
