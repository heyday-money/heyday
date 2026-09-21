use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Installment {
    id: String,
    name: String,
    account_id: String,
    account_name: String,
    debt_account_id: String,
    debt_account_name: String,
    debt_account_type: String,
    monthly_amount: String,
    installment_count: i64,
    interest_rate_bps: Option<String>,
    first_due_date: String,
    purchase_kind: String,
    purchase_transaction_id: Option<String>,
}
pub(crate) const SELECT: &str = "SELECT i.id, i.name, i.account_id, a.name AS account_name, i.debt_account_id, d.name AS debt_account_name, d.type AS debt_account_type, CAST(i.monthly_amount AS TEXT) AS monthly_amount, i.installment_count, CAST(i.interest_rate_bps AS TEXT) AS interest_rate_bps, i.first_due_date, i.purchase_kind, i.purchase_transaction_id FROM installments i JOIN accounts a ON a.id = i.account_id JOIN accounts d ON d.id = i.debt_account_id ORDER BY i.first_due_date, i.created_at, i.id";

#[derive(Deserialize)]
pub struct Purchase {
    amount: String,
    date: String,
}

#[derive(Deserialize)]
pub struct SaveInstallment {
    id: Option<String>,
    name: String,
    account_id: String,
    debt_account_id: String,
    monthly_amount: String,
    installment_count: i64,
    interest_rate_bps: Option<String>,
    first_due_date: String,
    currency: String,
    purchase: Option<Purchase>,
}
async fn save(pool: &SqlitePool, input: SaveInstallment) -> Result<(), String> {
    if input.id.is_some() && input.purchase.is_some() {
        return Err("Editing a schedule cannot record another purchase.".into());
    }
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter an installment name between 1 and 100 characters.".into());
    }
    if !(1..=600).contains(&input.installment_count) {
        return Err("Choose between 1 and 600 monthly installments.".into());
    }
    let interest = input
        .interest_rate_bps
        .as_deref()
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "Enter a valid interest rate within the supported range.")
        })
        .transpose()?;
    if interest.is_some_and(|value| value < 0) {
        return Err("Interest rate must be zero or greater.".into());
    }
    let amount = input
        .monthly_amount
        .parse::<i64>()
        .map_err(|_| "Enter an integer amount within the supported range.")?;
    if amount <= 0 || amount.checked_mul(input.installment_count).is_none() {
        return Err(
            "Monthly payment must be positive and the total must fit the supported range.".into(),
        );
    }
    if !crate::transactions::valid_date(&input.first_due_date) {
        return Err("Enter a valid first due date.".into());
    }
    let year: i64 = input.first_due_date[..4].parse().unwrap();
    let month: i64 = input.first_due_date[5..7].parse().unwrap();
    if year * 12 + month - 1 + input.installment_count - 1 >= 10000 * 12 {
        return Err("The last installment must be no later than December 9999.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency changed. Reload before saving the installment.".into());
    }
    if let Some(id) = &input.id {
        let legacy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM installments i JOIN accounts a ON a.id = i.debt_account_id WHERE i.id = ? AND a.type = 'loan')")
            .bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if legacy {
            return Err("Existing loan schedules are preserved for reference and forecasting. New installment plans are for credit cards only; manage loans in Accounts.".into());
        }
    }
    if let Some(id) = &input.id {
        let recorded_card: Option<String> = sqlx::query_scalar("SELECT debt_account_id FROM installments WHERE id = ? AND purchase_kind = 'new_purchase'")
            .bind(id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
        if recorded_card.is_some_and(|card| card != input.debt_account_id) {
            return Err("The credit card cannot change after recording the purchase. Correct the purchase in Transactions.".into());
        }
    }
    let source: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    let debt: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type = 'credit_card')").bind(&input.debt_account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if !source || !debt || input.account_id == input.debt_account_id {
        return Err(
            "Choose an active cash or bank account and a different active credit card.".into(),
        );
    }
    let purchase_id = if let Some(purchase) = input.purchase {
        if purchase.date > input.first_due_date {
            return Err("The first due date cannot be before the purchase date.".into());
        }
        let row = crate::transactions::insert_in_connection(
            &mut tx,
            crate::transactions::NewTransaction {
                kind: "expense".into(),
                account_id: input.debt_account_id.clone(),
                destination_account_id: None,
                amount: purchase.amount,
                date: purchase.date,
                description: name.to_owned(),
                currency: input.currency.clone(),
                payee_id: None,
                category_id: None,
            },
        )
        .await?;
        Some(row.id)
    } else {
        None
    };
    let result = if let Some(id) = input.id {
        sqlx::query("UPDATE installments SET name = ?, account_id = ?, debt_account_id = ?, monthly_amount = ?, installment_count = ?, first_due_date = ?, interest_rate_bps = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(name).bind(input.account_id).bind(input.debt_account_id).bind(amount).bind(input.installment_count).bind(input.first_due_date).bind(interest).bind(id).execute(&mut *tx).await
    } else {
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date, interest_rate_bps, purchase_kind, purchase_transaction_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(name).bind(input.account_id).bind(input.debt_account_id).bind(amount).bind(input.installment_count).bind(input.first_due_date).bind(interest).bind(if purchase_id.is_some() { "new_purchase" } else { "existing_purchase" }).bind(purchase_id).execute(&mut *tx).await
    }.map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Installment no longer exists. Reload to continue.".into());
    }
    tx.commit().await.map_err(|e| e.to_string())
}
async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM installments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Installment no longer exists. Reload to continue.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn save_installment(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveInstallment,
) -> Result<(), String> {
    save(pool.inner(), input).await
}
#[tauri::command]
pub async fn delete_installment(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
) -> Result<(), String> {
    remove(pool.inner(), &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn seed(pool: &SqlitePool) {
        sqlx::migrate!("./migrations").run(pool).await.unwrap();
        sqlx::query("UPDATE settings SET currency = 'THB'")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance, current_balance, is_archived) VALUES ('bank', 'Bank', 'bank', 10000, 9000, 0), ('card', 'Card', 'credit_card', 5000, 4000, 0), ('loan', 'Loan', 'loan', 8000, 7000, 0), ('old', 'Old', 'loan', 6000, 6000, 1)").execute(pool).await.unwrap();
    }
    fn input() -> SaveInstallment {
        SaveInstallment {
            id: None,
            name: " Laptop ".into(),
            account_id: "bank".into(),
            debt_account_id: "card".into(),
            monthly_amount: "1000".into(),
            installment_count: 12,
            interest_rate_bps: Some("0".into()),
            first_due_date: "2024-01-31".into(),
            currency: "THB".into(),
            purchase: None,
        }
    }
    async fn records(pool: &SqlitePool) -> Vec<Installment> {
        sqlx::query_as(SELECT).fetch_all(pool).await.unwrap()
    }
    async fn balances(pool: &SqlitePool) -> Vec<(String, i64, i64)> {
        sqlx::query_as("SELECT id, opening_balance, current_balance FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn schedules_persist_without_ledger_effects() {
        let path = std::env::temp_dir().join(format!(
            "heyday-installments-{}-{}.db",
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
        seed(&pool).await;
        let before = balances(&pool).await;
        save(&pool, input()).await.unwrap();
        let card = records(&pool).await.remove(0);
        assert_eq!(card.name, "Laptop");
        assert_eq!(card.interest_rate_bps.as_deref(), Some("0"));
        assert_eq!(card.debt_account_name, "Card");
        let mut second = input();
        second.interest_rate_bps = Some("750".into());
        second.monthly_amount = "9007199254740993".into();
        save(&pool, second).await.unwrap();
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        sqlx::migrate!("./migrations").run(&reopened).await.unwrap();
        assert_eq!(records(&reopened).await.len(), 2);
        assert!(records(&reopened)
            .await
            .iter()
            .any(|row| row.monthly_amount == "9007199254740993"
                && row.debt_account_name == "Card"
                && row.interest_rate_bps.as_deref() == Some("750")));
        let mut edit = input();
        edit.id = Some(card.id.clone());
        edit.monthly_amount = "2500".into();
        edit.installment_count = 6;
        edit.interest_rate_bps = Some("1234".into());
        save(&reopened, edit).await.unwrap();
        assert!(records(&reopened).await.iter().any(|row| row.id == card.id
            && row.monthly_amount == "2500"
            && row.installment_count == 6
            && row.interest_rate_bps.as_deref() == Some("1234")));
        remove(&reopened, &card.id).await.unwrap();
        assert!(remove(&reopened, &card.id).await.is_err());
        assert_eq!(balances(&reopened).await, before);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&reopened)
            .await
            .unwrap();
        assert_eq!(count, 0);
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn new_purchase_increases_debt_once_and_schedule_edits_never_rebook_it() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        sqlx::query("UPDATE accounts SET current_balance = 1000000, credit_limit = 10000000 WHERE id = 'card'").execute(&pool).await.unwrap();
        let mut purchase = input();
        purchase.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        save(&pool, purchase).await.unwrap();
        let row = records(&pool).await.remove(0);
        assert_eq!(row.purchase_kind, "new_purchase");
        let transaction_id = row.purchase_transaction_id.clone().unwrap();
        let card: (i64, i64) = sqlx::query_as("SELECT current_balance, credit_limit - current_balance FROM accounts WHERE id = 'card'").fetch_one(&pool).await.unwrap();
        assert_eq!(card, (3000000, 7000000));
        let expense: (String, String, i64) =
            sqlx::query_as("SELECT type, account_id, amount FROM transactions WHERE id = ?")
                .bind(&transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(expense, ("expense".into(), "card".into(), 2000000));
        let mut edit = input();
        edit.id = Some(row.id.clone());
        edit.monthly_amount = "5000".into();
        save(&pool, edit).await.unwrap();
        let mut rebook = input();
        rebook.id = Some(row.id.clone());
        rebook.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        assert!(save(&pool, rebook).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let updated: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(updated, 3000000);
        crate::transactions::remove(&pool, &transaction_id)
            .await
            .unwrap();
        let retained = records(&pool).await.remove(0);
        assert_eq!(retained.purchase_kind, "new_purchase");
        assert_eq!(retained.purchase_transaction_id, None);
        let reversed: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reversed, 1000000);
        remove(&pool, &row.id).await.unwrap();
        let mut another = input();
        another.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-15".into(),
        });
        save(&pool, another).await.unwrap();
        remove(&pool, &records(&pool).await[0].id).await.unwrap();
        let preserved: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id = 'card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(preserved, 3000000);
    }

    #[tokio::test]
    async fn purchase_and_schedule_fail_atomically() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        for (amount, date) in [
            ("0", "2024-01-01"),
            ("100", "2999-01-01"),
            ("100", "2024-02-30"),
            ("9223372036854775807", "2024-01-01"),
        ] {
            let mut value = input();
            value.purchase = Some(Purchase {
                amount: amount.into(),
                date: date.into(),
            });
            assert!(save(&pool, value).await.is_err());
        }
        sqlx::raw_sql("CREATE TRIGGER fail_schedule BEFORE INSERT ON installments BEGIN SELECT RAISE(ABORT, 'Schedule failed'); END;").execute(&pool).await.unwrap();
        let mut value = input();
        value.purchase = Some(Purchase {
            amount: "2000000".into(),
            date: "2024-01-01".into(),
        });
        assert!(save(&pool, value)
            .await
            .unwrap_err()
            .contains("Schedule failed"));
        assert_eq!(balances(&pool).await, before);
        assert!(records(&pool).await.is_empty());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn legacy_loan_schedules_remain_readable_but_cannot_be_rewritten() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date) VALUES ('legacy', 'Car payments', 'bank', 'loan', 1000, 12, '2026-01-31')").execute(&pool).await.unwrap();
        let before = balances(&pool).await;
        let legacy = records(&pool).await.remove(0);
        assert_eq!(legacy.debt_account_type, "loan");
        assert_eq!(legacy.monthly_amount, "1000");
        let mut edit = input();
        edit.id = Some(legacy.id.clone());
        assert!(save(&pool, edit)
            .await
            .unwrap_err()
            .contains("Existing loan schedules"));
        assert_eq!(records(&pool).await[0].debt_account_id, "loan");
        remove(&pool, &legacy.id).await.unwrap();
        assert_eq!(balances(&pool).await, before);
    }

    #[tokio::test]
    async fn interest_migration_preserves_existing_schedule_and_unknown_rate() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in [
            include_str!("../migrations/0001_initial.sql"),
            include_str!("../migrations/0008_installments.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Bank', 'bank', 10000), ('card', 'Card', 'credit_card', 5000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date) VALUES ('existing', 'Laptop', 'bank', 'card', 1000, 12, '2024-01-31')").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0009_installment_interest.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0010_installment_purchases.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let row = records(&pool).await.remove(0);
        assert_eq!(row.interest_rate_bps, None);
        assert_eq!(row.monthly_amount, "1000");
        assert_eq!(row.installment_count, 12);
        assert_eq!(row.first_due_date, "2024-01-31");
        assert_eq!(row.debt_account_id, "card");
        assert!(
            sqlx::query("UPDATE installments SET interest_rate_bps = -1")
                .execute(&pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("UPDATE installments SET interest_rate_bps = 1.5")
                .execute(&pool)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn invalid_and_stale_schedules_do_not_write() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        seed(&pool).await;
        let before = balances(&pool).await;
        let mut cases = vec![];
        for rate in ["-1", "1.5", "bad", "9223372036854775808"] {
            let mut value = input();
            value.interest_rate_bps = Some(rate.into());
            cases.push(value);
        }
        for amount in [
            "0",
            "-1",
            "1.2",
            "9223372036854775808",
            "9223372036854775807",
        ] {
            let mut value = input();
            value.monthly_amount = amount.into();
            cases.push(value);
        }
        for count in [0, -1, 601] {
            let mut value = input();
            value.installment_count = count;
            cases.push(value);
        }
        for date in [
            "2023-02-29",
            "2024-02-30",
            "2024-13-01",
            "0000-01-01",
            "9999-12-31",
            "bad",
        ] {
            let mut value = input();
            value.first_due_date = date.into();
            cases.push(value);
        }
        for id in ["bank", "loan", "missing", "old"] {
            let mut value = input();
            value.debt_account_id = id.into();
            cases.push(value);
        }
        for id in ["card", "missing", "old"] {
            let mut value = input();
            value.account_id = id.into();
            cases.push(value);
        }
        for name in ["".to_owned(), " ".into(), "x".repeat(101)] {
            let mut value = input();
            value.name = name;
            cases.push(value);
        }
        let mut value = input();
        value.currency = "USD".into();
        cases.push(value);
        let mut value = input();
        value.id = Some("missing".into());
        cases.push(value);
        for value in cases {
            assert!(save(&pool, value).await.is_err());
        }
        assert!(records(&pool).await.is_empty());
        assert_eq!(balances(&pool).await, before);
        let mut value = input();
        value.first_due_date = "2024-02-29".into();
        value.interest_rate_bps = None;
        save(&pool, value).await.unwrap();
        let mut value = input();
        value.first_due_date = "9999-12-31".into();
        value.installment_count = 1;
        value.monthly_amount = "9223372036854775807".into();
        save(&pool, value).await.unwrap();
        assert_eq!(records(&pool).await.len(), 2);
    }
}
