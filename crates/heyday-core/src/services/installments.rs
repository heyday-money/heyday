use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Installment {
    pub id: String,
    pub name: String,
    pub account_id: String,
    pub account_name: String,
    pub debt_account_id: String,
    pub debt_account_name: String,
    pub debt_account_type: String,
    pub monthly_amount: String,
    pub installment_count: i64,
    pub interest_rate_millis: Option<String>,
    pub first_due_date: String,
    pub purchase_kind: String,
    pub purchase_transaction_id: Option<String>,
}
pub const SELECT: &str = "SELECT i.id, i.name, i.account_id, a.name AS account_name, i.debt_account_id, d.name AS debt_account_name, d.type AS debt_account_type, CAST(i.monthly_amount AS TEXT) AS monthly_amount, i.installment_count, CAST(i.interest_rate_millis AS TEXT) AS interest_rate_millis, i.first_due_date, i.purchase_kind, i.purchase_transaction_id FROM installments i JOIN accounts a ON a.id = i.account_id JOIN accounts d ON d.id = i.debt_account_id ORDER BY i.first_due_date, i.created_at, i.id";

#[derive(Deserialize)]
pub struct Purchase {
    pub amount: String,
    pub date: String,
}

#[derive(Deserialize)]
pub struct SaveInstallment {
    pub id: Option<String>,
    pub name: String,
    pub account_id: String,
    pub debt_account_id: String,
    pub monthly_amount: String,
    pub installment_count: i64,
    pub interest_rate_millis: Option<String>,
    pub first_due_date: String,
    pub currency: String,
    pub purchase: Option<Purchase>,
}
pub async fn save(pool: &SqlitePool, input: SaveInstallment) -> Result<(), String> {
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
        .interest_rate_millis
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
    let source: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type IN ('cash', 'bank', 'wallet'))").bind(&input.account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    let debt: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0 AND type = 'credit_card')").bind(&input.debt_account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if !source || !debt || input.account_id == input.debt_account_id {
        return Err(
            "Choose an active cash, bank, or digital wallet account and a different active credit card.".into(),
        );
    }
    let purchase_id = if let Some(purchase) = input.purchase {
        if purchase.date > input.first_due_date {
            return Err("The first due date cannot be before the purchase date.".into());
        }
        let row = crate::transactions::insert_in_connection(
            &mut tx,
            crate::transactions::NewTransaction {
                cleared_account_ids: vec![],
                kind: "expense".into(),
                account_id: input.debt_account_id.clone(),
                destination_account_id: None,
                amount: purchase.amount,
                date: purchase.date,
                description: name.to_owned(),
                currency: input.currency.clone(),
                payee_id: None,
                category_id: None,
                income_source_id: None,
            },
        )
        .await?;
        Some(row.id)
    } else {
        None
    };
    // Keep exact surviving occurrences, never shift an inclusion to a different payment.
    // The existing review trigger preserves the issuer statement for explicit review.
    if let Some(id) = &input.id {
        let included: Vec<(String, String, String)> = sqlx::query_as("SELECT c.statement_id,c.date,s.account_id FROM card_statement_installments c JOIN card_statements s ON s.id=c.statement_id WHERE c.installment_id=?")
            .bind(id).fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
        for (statement, date, card) in included {
            if card != input.debt_account_id
                || !(0..input.installment_count)
                    .any(|n| crate::card_billing::scheduled_date(&input.first_due_date, n) == date)
            {
                sqlx::query("UPDATE card_statements SET needs_review=1 WHERE id=?")
                    .bind(&statement)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                sqlx::query(
                    "DELETE FROM card_statement_installments WHERE installment_id=? AND date=?",
                )
                .bind(id)
                .bind(date)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            }
        }
    }
    let result = if let Some(id) = input.id {
        sqlx::query("UPDATE installments SET name = ?, account_id = ?, debt_account_id = ?, monthly_amount = ?, installment_count = ?, first_due_date = ?, interest_rate_millis = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(name).bind(input.account_id).bind(input.debt_account_id).bind(amount).bind(input.installment_count).bind(input.first_due_date).bind(interest).bind(id).execute(&mut *tx).await
    } else {
        sqlx::query("INSERT INTO installments (id, name, account_id, debt_account_id, monthly_amount, installment_count, first_due_date, interest_rate_millis, purchase_kind, purchase_transaction_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(name).bind(input.account_id).bind(input.debt_account_id).bind(amount).bind(input.installment_count).bind(input.first_due_date).bind(interest).bind(if purchase_id.is_some() { "new_purchase" } else { "existing_purchase" }).bind(purchase_id).execute(&mut *tx).await
    }.map_err(|e| e.to_string())?;
    if result.rows_affected() != 1 {
        return Err("Installment no longer exists. Reload to continue.".into());
    }
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<(), String> {
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
pub async fn save_installment(pool: &SqlitePool, input: SaveInstallment) -> Result<(), String> {
    save(pool, input).await
}
pub async fn delete_installment(pool: &SqlitePool, id: String) -> Result<(), String> {
    remove(pool, &id).await
}
