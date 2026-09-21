use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Transaction {
    pub(crate) id: String,
    #[serde(rename = "type")]
    kind: String,
    account_id: String,
    account_name: String,
    destination_account_id: Option<String>,
    destination_account_name: Option<String>,
    amount: String,
    date: String,
    description: String,
    payee_id: Option<String>,
    payee_name: Option<String>,
    category_id: Option<String>,
    category_name: Option<String>,
}
pub(crate) const SELECT: &str = "SELECT t.id, t.type AS kind, t.account_id, a.name AS account_name, t.destination_account_id, d.name AS destination_account_name, CAST(t.amount AS TEXT) AS amount, t.date, t.description, t.payee_id, p.name AS payee_name, t.category_id, c.name AS category_name FROM transactions t JOIN accounts a ON a.id = t.account_id LEFT JOIN accounts d ON d.id = t.destination_account_id LEFT JOIN payees p ON p.id = t.payee_id LEFT JOIN categories c ON c.id = t.category_id";

#[derive(Deserialize)]
pub struct NewTransaction {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) account_id: String,
    pub(crate) destination_account_id: Option<String>,
    pub(crate) amount: String,
    pub(crate) date: String,
    pub(crate) description: String,
    pub(crate) currency: String,
    pub(crate) payee_id: Option<String>,
    pub(crate) category_id: Option<String>,
}

pub(crate) fn valid_date(date: &str) -> bool {
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

// Economic inflows increase assets and reduce amounts owed; outflows do the reverse.
async fn adjust(
    conn: &mut SqliteConnection,
    id: &str,
    inflow: i64,
    active: bool,
) -> Result<String, String> {
    let (kind, balance, archived): (String, i64, bool) =
        sqlx::query_as("SELECT type, current_balance, is_archived FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Choose an existing account.")?;
    if active && archived {
        return Err("Choose an active account.".into());
    }
    let debt = ["credit_card", "loan"].contains(&kind.as_str());
    let next = if debt {
        balance.checked_sub(inflow)
    } else {
        balance.checked_add(inflow)
    }
    .ok_or("This transaction would put an account balance outside the supported range.")?;
    sqlx::query(
        "UPDATE accounts SET current_balance = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(next)
    .bind(id)
    .execute(conn)
    .await
    .map_err(|e| e.to_string())?;
    Ok(kind)
}

pub(crate) async fn insert_in_connection(
    conn: &mut SqliteConnection,
    input: NewTransaction,
) -> Result<Transaction, String> {
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
    let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if input.date > today {
        return Err("Transactions cannot be dated in the future.".into());
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload transactions before saving.".into());
    }
    if input.kind != "expense" && (input.payee_id.is_some() || input.category_id.is_some()) {
        return Err("Payee and spending category apply only to expenses.".into());
    }
    for (table, id, label) in [
        ("payees", &input.payee_id, "payee"),
        ("categories", &input.category_id, "category"),
    ] {
        if let Some(id) = id {
            let active: bool = sqlx::query_scalar(&format!(
                "SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ? AND is_archived = 0)"
            ))
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            if !active {
                return Err(format!("Choose an existing, active {label}."));
            }
        }
    }
    adjust(
        &mut *conn,
        &input.account_id,
        if input.kind == "income" {
            amount
        } else {
            -amount
        },
        true,
    )
    .await?;
    if let Some(id) = &input.destination_account_id {
        let kind = adjust(&mut *conn, id, amount, true).await?;
        if input.kind == "repayment" && !["credit_card", "loan"].contains(&kind.as_str()) {
            return Err("Repayments must go to a credit card or loan.".into());
        }
    }
    let id: String = sqlx::query_scalar("INSERT INTO transactions (id, type, account_id, destination_account_id, amount, date, description, payee_id, category_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(input.kind).bind(input.account_id).bind(input.destination_account_id).bind(amount).bind(input.date).bind(description).bind(input.payee_id).bind(input.category_id)
        .fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
    let row = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
        .bind(id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row)
}

async fn insert(pool: &SqlitePool, input: NewTransaction) -> Result<Transaction, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let row = insert_in_connection(&mut tx, input).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(row)
}

pub(crate) async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let row: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Transaction no longer exists. Reload the page.")?;
    let amount = row.amount.parse::<i64>().map_err(|e| e.to_string())?;
    adjust(
        &mut tx,
        &row.account_id,
        if row.kind == "income" {
            -amount
        } else {
            amount
        },
        false,
    )
    .await?;
    if let Some(destination) = row.destination_account_id {
        adjust(&mut tx, &destination, -amount, false).await?;
    }
    sqlx::query("DELETE FROM transactions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_transaction(
    pool: tauri::State<'_, SqlitePool>,
    input: NewTransaction,
) -> Result<Transaction, String> {
    insert(pool.inner(), input).await
}
#[tauri::command]
pub async fn delete_transaction(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    remove(pool.inner(), &id).await
}
#[tauri::command]
pub async fn list_transactions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Transaction>, String> {
    sqlx::query_as(&format!(
        "{SELECT} ORDER BY t.date DESC, t.created_at DESC, t.id DESC"
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
        sqlx::raw_sql(include_str!("../migrations/0001_initial.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0002_account_details.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Bank', 'bank', 10000), ('card', 'Card', 'credit_card', 2000), ('loan', 'Loan', 'loan', -100), ('cash', 'Cash', 'cash', 0)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0003_transactions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0004_optional_transaction_description.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0005_payees_and_categories.sql"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }
    fn input(kind: &str, account: &str, destination: Option<&str>, amount: &str) -> NewTransaction {
        NewTransaction {
            kind: kind.into(),
            account_id: account.into(),
            destination_account_id: destination.map(str::to_owned),
            amount: amount.into(),
            date: "2024-02-29".into(),
            description: " Test ".into(),
            currency: "THB".into(),
            payee_id: None,
            category_id: None,
        }
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id, opening_balance, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
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
    #[tokio::test]
    async fn balances_signs_and_reversal() {
        let pool = database().await;
        let initial = balances(&pool).await;
        assert!(initial
            .iter()
            .all(|(_, opening, current)| opening == current));
        let income = insert(&pool, input("income", "bank", None, "1000"))
            .await
            .unwrap();
        let expense = insert(&pool, input("expense", "card", None, "500"))
            .await
            .unwrap();
        let repayment = insert(&pool, input("repayment", "bank", Some("card"), "3000"))
            .await
            .unwrap();
        let transfer = insert(&pool, input("transfer", "bank", Some("cash"), "200"))
            .await
            .unwrap();
        let loan = insert(&pool, input("expense", "loan", None, "500"))
            .await
            .unwrap();
        assert_eq!(
            balances(&pool).await,
            vec![
                ("bank".into(), 10000, 7800),
                ("card".into(), 2000, -500),
                ("cash".into(), 0, 200),
                ("loan".into(), -100, 400)
            ]
        );
        for row in [income, expense, repayment, transfer, loan] {
            remove(&pool, &row.id).await.unwrap();
            assert!(remove(&pool, &row.id).await.is_err());
        }
        assert_eq!(balances(&pool).await, initial);
    }
    #[tokio::test]
    async fn invalid_inputs_and_overflow_roll_back_everything() {
        let pool = database().await;
        sqlx::query("UPDATE accounts SET current_balance = ? WHERE id = 'card'")
            .bind(i64::MIN)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET is_archived = 1 WHERE id = 'cash'")
            .execute(&pool)
            .await
            .unwrap();
        let initial = balances(&pool).await;
        let mut cases = vec![
            input("transfer", "bank", Some("card"), "1"),
            input("income", "missing", None, "1"),
            input("income", "cash", None, "1"),
            input("transfer", "bank", Some("bank"), "1"),
            input("transfer", "bank", None, "1"),
            input("income", "bank", Some("card"), "1"),
            input("repayment", "card", Some("bank"), "1"),
            input("unknown", "bank", None, "1"),
        ];
        for amount in ["0", "-1", "1.5", "9223372036854775808"] {
            cases.push(input("income", "bank", None, amount));
        }
        let mut stale = input("income", "bank", None, "1");
        stale.currency = "USD".into();
        cases.push(stale);
        let mut date = input("income", "bank", None, "1");
        date.date = "2025-02-29".into();
        cases.push(date);
        let mut future = input("income", "bank", None, "1");
        future.date = "9999-01-01".into();
        cases.push(future);
        let mut empty = input("income", "bank", None, "1");
        empty.description = "x".repeat(201);
        cases.push(empty);
        for case in cases {
            assert!(insert(&pool, case).await.is_err());
            assert_eq!(balances(&pool).await, initial);
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    #[tokio::test]
    async fn precise_amounts_and_failed_delete_are_atomic() {
        let pool = database().await;
        let row = insert(&pool, input("income", "bank", None, "9007199254740993"))
            .await
            .unwrap();
        assert_eq!(row.amount, "9007199254740993");
        let stored: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 9007199254750993);
        remove(&pool, &row.id).await.unwrap();
        let row = insert(&pool, input("transfer", "bank", Some("card"), "10"))
            .await
            .unwrap();
        sqlx::query("UPDATE accounts SET current_balance = ? WHERE id = 'card'")
            .bind(i64::MAX)
            .execute(&pool)
            .await
            .unwrap();
        let initial = balances(&pool).await;
        assert!(remove(&pool, &row.id).await.is_err());
        assert_eq!(balances(&pool).await, initial);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn transactions_survive_reopen() {
        let path = std::env::temp_dir().join(format!(
            "heyday-transactions-{}-{}.db",
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
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance) VALUES ('bank', 'Bank', 'bank', 100, 100)").execute(&pool).await.unwrap();
        let payee = option(
            &pool,
            crate::transaction_options::OptionKind::Payee,
            None,
            "Market",
            false,
        )
        .await;
        let category = option(
            &pool,
            crate::transaction_options::OptionKind::Category,
            None,
            "Groceries",
            false,
        )
        .await;
        let mut value = input("expense", "bank", None, "125");
        value.payee_id = Some(payee.id.clone());
        value.category_id = Some(category.id.clone());
        let row = insert(&pool, value).await.unwrap();
        pool.close().await;
        let pool = SqlitePool::connect_with(options).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind(&row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.amount, "125");
        assert_eq!(stored.payee_name.as_deref(), Some("Market"));
        assert_eq!(stored.category_name.as_deref(), Some("Groceries"));
        assert_eq!(stored.payee_id.as_deref(), Some(payee.id.as_str()));
        assert_eq!(stored.category_id.as_deref(), Some(category.id.as_str()));
        assert_eq!(balances(&pool).await, vec![("bank".into(), 100, -25)]);
        remove(&pool, &row.id).await.unwrap();
        assert_eq!(balances(&pool).await, vec![("bank".into(), 100, 100)]);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn optional_description_migration_preserves_history_and_balances() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in [
            include_str!("../migrations/0001_initial.sql"),
            include_str!("../migrations/0002_account_details.sql"),
            include_str!("../migrations/0003_transactions.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance) VALUES ('bank', 'Bank', 'bank', 1000, 1000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transactions (id, type, account_id, amount, date, description) VALUES ('old', 'expense', 'bank', 125, '2024-02-29', 'Test')").execute(&pool).await.unwrap();
        sqlx::query("UPDATE accounts SET current_balance = 875 WHERE id = 'bank'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0004_optional_transaction_description.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0005_payees_and_categories.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind("old")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.description, "Test");
        assert_eq!(stored.amount, "125");
        assert_eq!(balances(&pool).await, vec![("bank".into(), 1000, 875)]);
        for description in ["", "   "] {
            let mut value = input("expense", "bank", None, "25");
            value.description = description.into();
            let row = insert(&pool, value).await.unwrap();
            assert_eq!(row.description, "");
            remove(&pool, &row.id).await.unwrap();
        }
        remove(&pool, "old").await.unwrap();
        assert_eq!(balances(&pool).await, vec![("bank".into(), 1000, 1000)]);
    }
    async fn option(
        pool: &SqlitePool,
        kind: crate::transaction_options::OptionKind,
        id: Option<String>,
        name: &str,
        archived: bool,
    ) -> crate::transaction_options::TransactionOption {
        crate::transaction_options::save(
            pool,
            crate::transaction_options::SaveOption {
                kind,
                id,
                name: name.into(),
                is_archived: archived,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn payees_and_categories_validate_and_preserve_references() {
        use crate::transaction_options::{save, OptionKind, SaveOption};
        let pool = database().await;
        let payee = option(&pool, OptionKind::Payee, None, "  Corner   Shop  ", false).await;
        let category = option(&pool, OptionKind::Category, None, "Groceries", false).await;
        assert_eq!(payee.name, "Corner Shop");
        for kind in [OptionKind::Payee, OptionKind::Category] {
            for name in [" ".to_string(), "x".repeat(101)] {
                assert!(save(
                    &pool,
                    SaveOption {
                        kind,
                        id: None,
                        name,
                        is_archived: false
                    }
                )
                .await
                .is_err());
            }
        }
        assert!(save(
            &pool,
            SaveOption {
                kind: OptionKind::Payee,
                id: None,
                name: "corner shop".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveOption {
                kind: OptionKind::Category,
                id: Some("missing".into()),
                name: "New name".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        let initial = balances(&pool).await;
        for (kind, payee_id, category_id) in [
            ("expense", Some("missing".to_string()), None),
            ("expense", None, Some("missing".to_string())),
            ("income", Some(payee.id.clone()), None),
            ("transfer", None, Some(category.id.clone())),
        ] {
            let mut value = input(
                kind,
                "bank",
                if kind == "transfer" {
                    Some("card")
                } else {
                    None
                },
                "100",
            );
            value.payee_id = payee_id;
            value.category_id = category_id;
            assert!(insert(&pool, value).await.is_err());
            assert_eq!(balances(&pool).await, initial);
        }
        let mut value = input("expense", "bank", None, "100");
        value.payee_id = Some(payee.id.clone());
        value.category_id = Some(category.id.clone());
        let row = insert(&pool, value).await.unwrap();
        assert_eq!(row.payee_name.as_deref(), Some("Corner Shop"));
        assert_eq!(row.category_name.as_deref(), Some("Groceries"));
        option(
            &pool,
            OptionKind::Payee,
            Some(payee.id.clone()),
            "Local Market",
            true,
        )
        .await;
        option(
            &pool,
            OptionKind::Category,
            Some(category.id.clone()),
            "Food",
            true,
        )
        .await;
        let stored: Transaction = sqlx::query_as(&format!("{SELECT} WHERE t.id = ?"))
            .bind(&row.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored.payee_name.as_deref(), Some("Local Market"));
        assert_eq!(stored.category_name.as_deref(), Some("Food"));
        let archived_balances = balances(&pool).await;
        for (payee_id, category_id) in [
            (Some(payee.id.clone()), None),
            (None, Some(category.id.clone())),
        ] {
            let mut value = input("expense", "bank", None, "100");
            value.payee_id = payee_id;
            value.category_id = category_id;
            assert!(insert(&pool, value).await.is_err());
            assert_eq!(balances(&pool).await, archived_balances);
        }
        assert!(sqlx::query("DELETE FROM payees WHERE id = ?")
            .bind(&payee.id)
            .execute(&pool)
            .await
            .is_err());
        assert!(sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(&category.id)
            .execute(&pool)
            .await
            .is_err());
        option(
            &pool,
            OptionKind::Payee,
            Some(payee.id.clone()),
            "Local Market",
            false,
        )
        .await;
        option(
            &pool,
            OptionKind::Category,
            Some(category.id.clone()),
            "Food",
            false,
        )
        .await;
        let mut restored = input("expense", "bank", None, "100");
        restored.payee_id = Some(payee.id);
        restored.category_id = Some(category.id);
        let second = insert(&pool, restored).await.unwrap();
        remove(&pool, &row.id).await.unwrap();
        remove(&pool, &second.id).await.unwrap();
        assert_eq!(balances(&pool).await, initial);
    }
}
