use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub subtotal: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Item {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub description: String,
    pub card_name: String,
    pub transaction_category_id: Option<String>,
    pub schedule_amount: Option<String>,
    pub schedule_start: Option<String>,
    pub schedule_end: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Amount {
    pub item_id: String,
    pub month: String,
    pub amount: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Month {
    pub month: String,
    pub status: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Opening {
    pub month: String,
    pub amount: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct ExpenseCategory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerIncome {
    pub id: String,
    pub name: String,
    pub estimated_amount: String,
    pub recurrence_day_of_month: i64,
    pub recurrence_frequency: String,
    pub is_active: bool,
    pub destination_account_name: String,
    pub account_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerDebtAccount {
    pub paid_off_on: Option<String>,
    pub monthly_installment: Option<String>,
    pub id: String,
    pub name: String,
    pub loan_type: Option<String>,
    pub current_balance: String,
    pub notes: Option<String>,
    pub is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerInstallment {
    pub id: String,
    pub name: String,
    pub debt_account_id: String,
    pub debt_account_name: String,
    pub debt_account_type: String,
    pub account_name: String,
    pub monthly_amount: String,
    pub installment_count: i64,
    pub first_due_date: String,
    pub accounts_available: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerCreditCard {
    pub id: String,
    pub name: String,
    pub is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerCardTransaction {
    pub loan_account_id: Option<String>,
    pub loan_component: Option<String>,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: String,
    pub account_type: String,
    pub category_name: Option<String>,
    pub income_source_id: Option<String>,
    pub income_source_name: Option<String>,
    pub category_id: Option<String>,
    pub account_name: String,
    pub destination_account_id: Option<String>,
    pub destination_account_type: Option<String>,
    pub amount: String,
    pub date: String,
}
#[derive(Serialize)]
pub struct Planner {
    pub selective_defaults: Vec<crate::selective_defaults::Exclusion>,
    pub ledger_transactions: Vec<PlannerCardTransaction>,
    pub card_billing: crate::card_billing::BillingData,
    pub loan_contracts: Vec<crate::loans::Contract>,
    pub loan_facilities: Vec<crate::loans::Facility>,
    pub categories: Vec<Category>,
    pub incomes: Vec<PlannerIncome>,
    pub income_deductions: Vec<crate::income_deductions::IncomeDeduction>,
    pub source_currency: Option<String>,
    pub debt_accounts: Vec<PlannerDebtAccount>,
    pub installments: Vec<PlannerInstallment>,
    pub credit_cards: Vec<PlannerCreditCard>,
    pub card_transactions: Vec<PlannerCardTransaction>,
    pub items: Vec<Item>,
    pub amounts: Vec<Amount>,
    pub months: Vec<Month>,
    pub opening: Option<Opening>,
    pub period_start_day: i64,
    pub expense_categories: Vec<ExpenseCategory>,
}
pub async fn snapshot(conn: &mut SqliteConnection) -> Result<Planner, sqlx::Error> {
    let mut result=Planner {
        selective_defaults: crate::selective_defaults::periods(conn).await?,
        ledger_transactions: sqlx::query_as("SELECT (SELECT COALESCE(lp.loan_account_id,c.account_id) FROM loan_payment_parts lp LEFT JOIN loan_contracts c ON c.id=lp.contract_id WHERE lp.transaction_id=t.id) AS loan_account_id,(SELECT component FROM loan_payment_parts WHERE transaction_id=t.id) AS loan_component,t.id,t.type AS kind,t.account_id,a.name AS account_name,a.type AS account_type,t.income_source_id,(SELECT name FROM incomes WHERE id=t.income_source_id) income_source_name,t.category_id,(SELECT name FROM categories WHERE id=t.category_id) category_name,t.destination_account_id,d.type AS destination_account_type,CAST(t.amount AS TEXT) AS amount,t.date FROM transactions t JOIN accounts a ON a.id=t.account_id LEFT JOIN accounts d ON d.id=t.destination_account_id ORDER BY t.date,t.id").fetch_all(&mut *conn).await?,
        card_billing: crate::card_billing::snapshot(conn).await?,
        loan_contracts: crate::loans::contracts(conn).await?,
        loan_facilities: crate::loans::facilities(conn).await?,
        income_deductions: sqlx::query_as(&format!("{} ORDER BY rowid",crate::income_deductions::SELECT)).fetch_all(&mut *conn).await?,
        credit_cards: sqlx::query_as("SELECT id,name,is_archived FROM accounts WHERE type='credit_card' ORDER BY name,id").fetch_all(&mut *conn).await?,
        card_transactions: sqlx::query_as("SELECT (SELECT COALESCE(lp.loan_account_id,c.account_id) FROM loan_payment_parts lp LEFT JOIN loan_contracts c ON c.id=lp.contract_id WHERE lp.transaction_id=t.id) AS loan_account_id,(SELECT component FROM loan_payment_parts WHERE transaction_id=t.id) AS loan_component,t.id,t.type AS kind,t.account_id,a.name AS account_name,a.type AS account_type,t.income_source_id,(SELECT name FROM incomes WHERE id=t.income_source_id) income_source_name,t.category_id,(SELECT name FROM categories WHERE id=t.category_id) category_name,t.destination_account_id,d.type AS destination_account_type,CAST(t.amount AS TEXT) AS amount,t.date FROM transactions t JOIN accounts a ON a.id=t.account_id LEFT JOIN accounts d ON d.id=t.destination_account_id WHERE a.type='credit_card' OR d.type='credit_card' ORDER BY t.date,t.id").fetch_all(&mut *conn).await?,
        incomes: sqlx::query_as("SELECT i.id,i.name,CAST(i.estimated_amount AS TEXT) AS estimated_amount,i.recurrence_day_of_month,CASE WHEN i.is_irregular=1 THEN 'irregular' ELSE i.recurrence_frequency END AS recurrence_frequency,i.is_active,a.name AS destination_account_name,a.is_archived AS account_archived FROM incomes i JOIN accounts a ON a.id=i.destination_account_id ORDER BY i.created_at,i.id").fetch_all(&mut *conn).await?,
        debt_accounts: sqlx::query_as("SELECT id,name,loan_type,CAST(current_balance AS TEXT) AS current_balance,CAST(monthly_installment AS TEXT) AS monthly_installment,notes,is_archived,(SELECT paid_off_on FROM loan_payoffs WHERE account_id=accounts.id) AS paid_off_on FROM accounts WHERE type='loan' ORDER BY name,id").fetch_all(&mut *conn).await?,
        installments: sqlx::query_as("SELECT i.id,i.name,i.debt_account_id,d.name AS debt_account_name,d.type AS debt_account_type,a.name AS account_name,CAST(i.monthly_amount AS TEXT) AS monthly_amount,i.installment_count,i.first_due_date,(a.is_archived=0 AND d.is_archived=0 AND a.type IN ('cash','bank','wallet') AND d.type IN ('credit_card','loan')) AS accounts_available FROM installments i JOIN accounts a ON a.id=i.account_id JOIN accounts d ON d.id=i.debt_account_id ORDER BY i.first_due_date,i.id").fetch_all(&mut *conn).await?,
        source_currency: sqlx::query_scalar("SELECT currency FROM settings WHERE id=1").fetch_one(&mut *conn).await?,
        categories: sqlx::query_as("SELECT id,name,subtotal FROM planner_categories ORDER BY position").fetch_all(&mut *conn).await?,
        items: sqlx::query_as("SELECT id,category_id,name,description,card_name,transaction_category_id,CAST(schedule_amount AS TEXT) AS schedule_amount,schedule_start,schedule_end FROM planner_items ORDER BY rowid").fetch_all(&mut *conn).await?,
        amounts: sqlx::query_as("SELECT item_id,month,CAST(amount AS TEXT) AS amount FROM planner_amounts UNION ALL SELECT 'income:' || income_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_income_amounts UNION ALL SELECT 'debt:' || account_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_debt_amounts UNION ALL SELECT 'installment:' || installment_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_installment_amounts UNION ALL SELECT 'deduction:' || deduction_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_deduction_amounts").fetch_all(&mut *conn).await?,
        months: sqlx::query_as("SELECT month,status FROM planner_months").fetch_all(&mut *conn).await?,
        opening: sqlx::query_as("SELECT month,CAST(amount AS TEXT) AS amount FROM planner_opening WHERE id=1").fetch_optional(&mut *conn).await?,
        period_start_day: sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id=1").fetch_one(&mut *conn).await?,
        expense_categories: sqlx::query_as("SELECT id,name,icon,is_archived FROM categories ORDER BY name").fetch_all(&mut *conn).await?,
    };
    let imported = crate::cloud_sync_ledger::imported_entries(conn)
        .await
        .map_err(sqlx::Error::Protocol)?;
    for row in result
        .ledger_transactions
        .iter_mut()
        .chain(result.card_transactions.iter_mut())
    {
        if let Some((_, entry)) = imported.get(&row.id) {
            if row.loan_account_id.is_none() {
                row.loan_account_id = entry.loan_account_id.clone();
                row.loan_component = entry.loan_component.clone();
            }
            if row.income_source_name.is_none() {
                if let Some(source) = &entry.income_source {
                    row.income_source_id = Some(source.id.clone());
                    row.income_source_name = Some(source.name.clone());
                }
            }
        }
    }
    Ok(result)
}
pub async fn get_cashflow_planner(pool: &SqlitePool) -> Result<Planner, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = snapshot(&mut tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    Item {
        id: Option<String>,
        category_id: String,
        name: String,
        description: String,
        card_name: String,
        transaction_category_id: Option<String>,
        schedule_amount: Option<String>,
        schedule_start: Option<String>,
        schedule_end: Option<String>,
    },
    Delete {
        id: String,
    },
    Amount {
        item_id: String,
        month: String,
        amount: Option<String>,
    },
    Status {
        month: String,
        status: String,
    },
    Opening {
        month: String,
        amount: Option<String>,
    },
}
pub fn month_index(value: &str) -> Result<i32, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || bytes[4] != b'-'
        || !bytes[..4]
            .iter()
            .chain(bytes[5..].iter())
            .all(u8::is_ascii_digit)
    {
        return Err("Choose a valid cycle month.".into());
    }
    let year: i32 = value[..4].parse().map_err(|_| "Invalid year")?;
    let month: i32 = value[5..].parse().map_err(|_| "Invalid month")?;
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err("Choose a valid cycle month.".into());
    }
    Ok(year * 12 + month - 1)
}
pub fn money(value: &str, signed: bool) -> Result<i64, String> {
    if value.is_empty()
        || !value
            .strip_prefix('-')
            .unwrap_or(value)
            .bytes()
            .all(|b| b.is_ascii_digit())
    {
        return Err("Enter an integer satang amount.".into());
    }
    let amount: i64 = value
        .parse()
        .map_err(|_| "Amount is outside the supported range.")?;
    if !signed && amount < 0 {
        return Err("Amount cannot be negative.".into());
    }
    Ok(amount)
}
pub async fn apply(pool: &SqlitePool, input: Change) -> Result<Planner, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    match input {
        Change::Item {
            id,
            category_id,
            name,
            description,
            card_name,
            transaction_category_id,
            schedule_amount,
            schedule_start,
            schedule_end,
        } => {
            let name = name.trim();
            if name.is_empty()
                || name.chars().count() > 100
                || description.chars().count() > 2000
                || card_name.chars().count() > 100
            {
                return Err(
                    "Enter a name up to 100 characters and description up to 2000 characters."
                        .into(),
                );
            }
            if ![
                "income",
                "deductions",
                "debt",
                "installments",
                "cards",
                "expenses",
            ]
            .contains(&category_id.as_str())
            {
                return Err("Invalid category.".into());
            }
            let amount = match (&schedule_amount, &schedule_start, &schedule_end) {
                (None, None, None) => None,
                (Some(a), Some(s), Some(e)) => {
                    let count = month_index(e)? - month_index(s)? + 1;
                    if count < 1 || (category_id == "installments" && count > 600) {
                        return Err("Invalid schedule range (installments: 1–600 cycles).".into());
                    }
                    let value = money(a, false)?;
                    value
                        .checked_mul(i64::from(count))
                        .ok_or("Scheduled total exceeds the supported range.")?;
                    Some(value)
                }
                _ => return Err("Provide an amount, first cycle and final cycle together.".into()),
            };
            if let Some(ref category) = transaction_category_id {
                if category_id != "expenses" {
                    return Err("Only general expenses can link transaction categories.".into());
                }
                let allowed: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE id=? AND (is_archived=0 OR EXISTS(SELECT 1 FROM planner_items WHERE id=? AND transaction_category_id=categories.id)))").bind(category).bind(&id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
                if !allowed {
                    return Err("Choose an active transaction category.".into());
                }
            }
            if let Some(ref id) = id {
                let existing: Option<String> =
                    sqlx::query_scalar("SELECT category_id FROM planner_items WHERE id=?")
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                if existing.as_deref() != Some(&category_id) {
                    return Err("Item is unavailable or category changed.".into());
                }
            }
            sqlx::query("INSERT INTO planner_items(id,category_id,name,description,card_name,transaction_category_id,schedule_amount,schedule_start,schedule_end) VALUES(COALESCE(?,lower(hex(randomblob(16)))),?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,card_name=excluded.card_name,transaction_category_id=excluded.transaction_category_id,schedule_amount=excluded.schedule_amount,schedule_start=excluded.schedule_start,schedule_end=excluded.schedule_end")
                .bind(id).bind(category_id).bind(name).bind(description.trim()).bind(card_name.trim()).bind(transaction_category_id).bind(amount).bind(schedule_start).bind(schedule_end).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
        Change::Delete { id } => {
            sqlx::query("DELETE FROM planner_items WHERE id=?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        Change::Amount {
            item_id,
            month,
            amount,
        } => {
            month_index(&month)?;
            if let Some(income_id) = item_id.strip_prefix("income:") {
                let currency: Option<String> =
                    sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                if currency.as_deref() != Some("THB") {
                    return Err("Income overrides require THB income sources.".into());
                }
                if let Some(amount) = amount {
                    sqlx::query("INSERT INTO planner_income_amounts VALUES(?,?,?) ON CONFLICT(income_id,month) DO UPDATE SET amount=excluded.amount").bind(income_id).bind(month).bind(money(&amount,false)?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                } else {
                    sqlx::query("DELETE FROM planner_income_amounts WHERE income_id=? AND month=?")
                        .bind(income_id)
                        .bind(month)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            } else if let Some(deduction_id) = item_id.strip_prefix("deduction:") {
                let currency: Option<String> =
                    sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                if currency.as_deref() != Some("THB") {
                    return Err("Deduction overrides require THB salary sources.".into());
                }
                if let Some(amount) = amount {
                    sqlx::query("INSERT INTO planner_deduction_amounts VALUES(?,?,?) ON CONFLICT(deduction_id,month) DO UPDATE SET amount=excluded.amount").bind(deduction_id).bind(month).bind(money(&amount,false)?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                } else {
                    sqlx::query(
                        "DELETE FROM planner_deduction_amounts WHERE deduction_id=? AND month=?",
                    )
                    .bind(deduction_id)
                    .bind(month)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
            } else if item_id.starts_with("debt:") || item_id.starts_with("installment:") {
                let currency: Option<String> =
                    sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                if currency.as_deref() != Some("THB") {
                    return Err("Linked payment overrides require THB sources.".into());
                }
                let (is_debt, source_id) = if let Some(id) = item_id.strip_prefix("debt:") {
                    (true, id)
                } else {
                    (false, item_id.strip_prefix("installment:").unwrap())
                };
                let exists: bool = sqlx::query_scalar(if is_debt {
                    "SELECT EXISTS(SELECT 1 FROM accounts WHERE id=? AND type='loan')"
                } else {
                    "SELECT EXISTS(SELECT 1 FROM installments i JOIN accounts a ON a.id=i.debt_account_id WHERE i.id=? AND a.type='credit_card')"
                }).bind(source_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
                if !exists {
                    return Err(
                        "This linked payment source is unavailable. Reload the planner.".into(),
                    );
                }
                if let Some(amount) = amount {
                    sqlx::query(if is_debt {
                        "INSERT INTO planner_debt_amounts VALUES(?,?,?) ON CONFLICT(account_id,month) DO UPDATE SET amount=excluded.amount"
                    } else {
                        "INSERT INTO planner_installment_amounts VALUES(?,?,?) ON CONFLICT(installment_id,month) DO UPDATE SET amount=excluded.amount"
                    }).bind(source_id).bind(month).bind(money(&amount,false)?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                } else {
                    sqlx::query(if is_debt {
                        "DELETE FROM planner_debt_amounts WHERE account_id=? AND month=?"
                    } else {
                        "DELETE FROM planner_installment_amounts WHERE installment_id=? AND month=?"
                    })
                    .bind(source_id)
                    .bind(month)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
            } else if let Some(amount) = amount {
                sqlx::query("INSERT INTO planner_amounts VALUES(?,?,?) ON CONFLICT(item_id,month) DO UPDATE SET amount=excluded.amount").bind(item_id).bind(month).bind(money(&amount,false)?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
            } else {
                sqlx::query("DELETE FROM planner_amounts WHERE item_id=? AND month=?")
                    .bind(item_id)
                    .bind(month)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Change::Status { month, status } => {
            month_index(&month)?;
            if !["tracking", "forecast", "complete"].contains(&status.as_str()) {
                return Err("Invalid cycle status.".into());
            }
            sqlx::query("INSERT INTO planner_months VALUES(?,?) ON CONFLICT(month) DO UPDATE SET status=excluded.status").bind(month).bind(status).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
        Change::Opening { month, amount } => {
            month_index(&month)?;
            if let Some(amount) = amount {
                sqlx::query("INSERT INTO planner_opening VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET month=excluded.month,amount=excluded.amount").bind(month).bind(money(&amount,true)?).execute(&mut *tx).await.map_err(|e| e.to_string())?;
            } else {
                sqlx::query("DELETE FROM planner_opening")
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    let result = snapshot(&mut tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
pub async fn save_cashflow_planner(pool: &SqlitePool, input: Change) -> Result<Planner, String> {
    apply(pool, input).await
}
