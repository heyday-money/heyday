use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Category {
    id: String,
    name: String,
    subtotal: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Item {
    id: String,
    category_id: String,
    name: String,
    description: String,
    card_name: String,
    transaction_category_id: Option<String>,
    schedule_amount: Option<String>,
    schedule_start: Option<String>,
    schedule_end: Option<String>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Amount {
    item_id: String,
    month: String,
    amount: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Month {
    month: String,
    status: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Opening {
    month: String,
    amount: String,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct ExpenseCategory {
    id: String,
    name: String,
    is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerIncome {
    id: String,
    name: String,
    estimated_amount: String,
    recurrence_day_of_month: i64,
    is_active: bool,
    destination_account_name: String,
    account_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerDebtAccount {
    id: String,
    name: String,
    loan_type: Option<String>,
    current_balance: String,
    notes: Option<String>,
    is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerInstallment {
    id: String,
    name: String,
    debt_account_id: String,
    debt_account_name: String,
    debt_account_type: String,
    account_name: String,
    monthly_amount: String,
    installment_count: i64,
    first_due_date: String,
    accounts_available: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerCreditCard {
    id: String,
    name: String,
    is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct PlannerCardTransaction {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    account_id: String,
    account_type: String,
    destination_account_id: Option<String>,
    destination_account_type: Option<String>,
    amount: String,
    date: String,
}
#[derive(Serialize)]
pub struct Planner {
    categories: Vec<Category>,
    incomes: Vec<PlannerIncome>,
    income_deductions: Vec<crate::income_deductions::IncomeDeduction>,
    source_currency: Option<String>,
    debt_accounts: Vec<PlannerDebtAccount>,
    installments: Vec<PlannerInstallment>,
    credit_cards: Vec<PlannerCreditCard>,
    card_transactions: Vec<PlannerCardTransaction>,
    items: Vec<Item>,
    amounts: Vec<Amount>,
    months: Vec<Month>,
    opening: Option<Opening>,
    period_start_day: i64,
    expense_categories: Vec<ExpenseCategory>,
}
async fn snapshot(conn: &mut SqliteConnection) -> Result<Planner, sqlx::Error> {
    Ok(Planner {
        income_deductions: sqlx::query_as(&format!("{} ORDER BY rowid",crate::income_deductions::SELECT)).fetch_all(&mut *conn).await?,
        credit_cards: sqlx::query_as("SELECT id,name,is_archived FROM accounts WHERE type='credit_card' ORDER BY name,id").fetch_all(&mut *conn).await?,
        card_transactions: sqlx::query_as("SELECT t.id,t.type AS kind,t.account_id,a.type AS account_type,t.destination_account_id,d.type AS destination_account_type,CAST(t.amount AS TEXT) AS amount,t.date FROM transactions t JOIN accounts a ON a.id=t.account_id LEFT JOIN accounts d ON d.id=t.destination_account_id WHERE a.type='credit_card' OR d.type='credit_card' ORDER BY t.date,t.id").fetch_all(&mut *conn).await?,
        incomes: sqlx::query_as("SELECT i.id,i.name,CAST(i.estimated_amount AS TEXT) AS estimated_amount,i.recurrence_day_of_month,i.is_active,a.name AS destination_account_name,a.is_archived AS account_archived FROM incomes i JOIN accounts a ON a.id=i.destination_account_id ORDER BY i.created_at,i.id").fetch_all(&mut *conn).await?,
        debt_accounts: sqlx::query_as("SELECT id,name,loan_type,CAST(current_balance AS TEXT) AS current_balance,notes,is_archived FROM accounts WHERE type='loan' ORDER BY name,id").fetch_all(&mut *conn).await?,
        installments: sqlx::query_as("SELECT i.id,i.name,i.debt_account_id,d.name AS debt_account_name,d.type AS debt_account_type,a.name AS account_name,CAST(i.monthly_amount AS TEXT) AS monthly_amount,i.installment_count,i.first_due_date,(a.is_archived=0 AND d.is_archived=0 AND a.type IN ('cash','bank') AND d.type IN ('credit_card','loan')) AS accounts_available FROM installments i JOIN accounts a ON a.id=i.account_id JOIN accounts d ON d.id=i.debt_account_id ORDER BY i.first_due_date,i.id").fetch_all(&mut *conn).await?,
        source_currency: sqlx::query_scalar("SELECT currency FROM settings WHERE id=1").fetch_one(&mut *conn).await?,
        categories: sqlx::query_as("SELECT id,name,subtotal FROM planner_categories ORDER BY position").fetch_all(&mut *conn).await?,
        items: sqlx::query_as("SELECT id,category_id,name,description,card_name,transaction_category_id,CAST(schedule_amount AS TEXT) AS schedule_amount,schedule_start,schedule_end FROM planner_items ORDER BY rowid").fetch_all(&mut *conn).await?,
        amounts: sqlx::query_as("SELECT item_id,month,CAST(amount AS TEXT) AS amount FROM planner_amounts UNION ALL SELECT 'income:' || income_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_income_amounts UNION ALL SELECT 'debt:' || account_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_debt_amounts UNION ALL SELECT 'installment:' || installment_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_installment_amounts UNION ALL SELECT 'deduction:' || deduction_id AS item_id,month,CAST(amount AS TEXT) AS amount FROM planner_deduction_amounts").fetch_all(&mut *conn).await?,
        months: sqlx::query_as("SELECT month,status FROM planner_months").fetch_all(&mut *conn).await?,
        opening: sqlx::query_as("SELECT month,CAST(amount AS TEXT) AS amount FROM planner_opening WHERE id=1").fetch_optional(&mut *conn).await?,
        period_start_day: sqlx::query_scalar("SELECT period_start_day FROM settings WHERE id=1").fetch_one(&mut *conn).await?,
        expense_categories: sqlx::query_as("SELECT id,name,is_archived FROM categories ORDER BY name").fetch_all(conn).await?,
    })
}
#[tauri::command]
pub async fn get_cashflow_planner(pool: tauri::State<'_, SqlitePool>) -> Result<Planner, String> {
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
fn month_index(value: &str) -> Result<i32, String> {
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
fn money(value: &str, signed: bool) -> Result<i64, String> {
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
async fn apply(pool: &SqlitePool, input: Change) -> Result<Planner, String> {
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
#[tauri::command]
pub async fn save_cashflow_planner(
    pool: tauri::State<'_, SqlitePool>,
    input: Change,
) -> Result<Planner, String> {
    apply(pool.inner(), input).await
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
    fn item(id: Option<String>, amount: Option<&str>) -> Change {
        Change::Item {
            id,
            category_id: "installments".into(),
            name: "Laptop".into(),
            description: "Work computer".into(),
            card_name: "Card A".into(),
            transaction_category_id: None,
            schedule_amount: amount.map(String::from),
            schedule_start: amount.map(|_| "2026-12".into()),
            schedule_end: amount.map(|_| "2027-02".into()),
        }
    }
    #[tokio::test]
    async fn planner_crud_validates_and_preserves_explicit_zero_and_overrides() {
        let pool = database().await;
        let initial = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(initial.items.len(), 10);
        assert!(initial.amounts.is_empty() && initial.opening.is_none());
        assert_eq!(initial.expense_categories.len(), 10);
        let saved = apply(&pool, item(None, Some("300000"))).await.unwrap();
        let id = saved.items.last().unwrap().id.clone();
        apply(
            &pool,
            Change::Amount {
                item_id: id.clone(),
                month: "2027-01".into(),
                amount: Some("0".into()),
            },
        )
        .await
        .unwrap();
        let saved = apply(&pool, item(Some(id.clone()), Some("400000")))
            .await
            .unwrap();
        assert_eq!(saved.amounts[0].amount, "0");
        assert_eq!(
            saved.items.last().unwrap().schedule_amount.as_deref(),
            Some("400000")
        );
        for bad in ["2027-13", "0000-01", "27-01", "2027-1", "éé-01"] {
            assert!(apply(
                &pool,
                Change::Status {
                    month: bad.into(),
                    status: "tracking".into()
                }
            )
            .await
            .is_err());
        }
        for bad in ["-1", "1.00", "9223372036854775808", "+1", ""] {
            assert!(apply(
                &pool,
                Change::Amount {
                    item_id: id.clone(),
                    month: "2027-01".into(),
                    amount: Some(bad.into())
                }
            )
            .await
            .is_err());
        }
        assert!(apply(&pool, item(None, Some("9223372036854775807")))
            .await
            .is_err());
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "missing".into(),
                month: "2027-01".into(),
                amount: Some("1".into())
            }
        )
        .await
        .is_err());
        let saved = apply(
            &pool,
            Change::Opening {
                month: "2026-12".into(),
                amount: Some("0".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(saved.opening.unwrap().amount, "0");
        let saved = apply(&pool, Change::Delete { id }).await.unwrap();
        assert_eq!(saved.items.len(), 10);
        assert!(saved.amounts.is_empty());
        let saved = apply(
            &pool,
            Change::Opening {
                month: "2026-12".into(),
                amount: None,
            },
        )
        .await
        .unwrap();
        assert!(saved.opening.is_none());
        let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        let transactions: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((accounts, transactions), (0, 0));
    }
    #[tokio::test]
    async fn upgrade_preserves_existing_currency_balances_and_category_references() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 12)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE settings SET currency='USD',period_start_day=31")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,9000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO categories(id,name,name_key,is_archived) VALUES('existing-water','WATER','water',1)").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0012_cashflow_planner.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0013_planner_income.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let data = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(data.period_start_day, 31);
        assert_eq!(
            data.items
                .iter()
                .find(|i| i.id == "expenses-0")
                .unwrap()
                .transaction_category_id
                .as_deref(),
            Some("existing-water")
        );
        assert_eq!(data.expense_categories.len(), 10);
        assert!(
            data.expense_categories
                .iter()
                .find(|c| c.id == "existing-water")
                .unwrap()
                .is_archived
        );
        let row: (String,i64,i64) = sqlx::query_as("SELECT currency,opening_balance,current_balance FROM settings,accounts WHERE accounts.id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(row, ("USD".into(), 10000, 9000));
        assert!(data.amounts.is_empty());
    }

    #[tokio::test]
    async fn income_snapshot_and_overrides_preserve_sources_and_balances() {
        let pool = database().await;
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Employer salary','bank','salary',5000000,25)").execute(&pool).await.unwrap();
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "income:salary".into(),
                month: "2026-12".into(),
                amount: Some("0".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.incomes.len(), 1);
        assert_eq!(result.incomes[0].estimated_amount, "5000000");
        assert_eq!(result.source_currency.as_deref(), Some("THB"));
        assert_eq!(result.amounts[0].item_id, "income:salary");
        assert_eq!(result.amounts[0].amount, "0");
        let balance: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 10000);
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "income:missing".into(),
                month: "2026-12".into(),
                amount: Some("100".into())
            }
        )
        .await
        .is_err());
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "income:salary".into(),
                month: "2026-12".into(),
                amount: None,
            },
        )
        .await
        .unwrap();
        assert!(result.amounts.is_empty());
        sqlx::query("UPDATE settings SET currency='USD'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "income:salary".into(),
                month: "2026-12".into(),
                amount: Some("100".into())
            }
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn income_migration_preserves_edited_placeholders_and_saved_amounts() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 13)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE planner_items SET name='My bonus' WHERE id='income-2'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO planner_amounts VALUES('income-0','2026-12',0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0013_planner_income.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result.items.iter().any(|i| i.id == "income-0"));
        assert!(result
            .items
            .iter()
            .any(|i| i.id == "income-2" && i.name == "My bonus"));
        assert!(!result.items.iter().any(|i| i.id == "income-1"));
        assert_eq!(result.amounts[0].amount, "0");
    }

    #[tokio::test]
    async fn linked_debt_and_installments_query_and_save_without_ledger_effects() {
        let pool = database().await;
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000),('loan','Home loan','loan',9007199254740993,9007199254740993),('card','Visa','credit_card',20000,20000)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO installments(id,name,account_id,debt_account_id,monthly_amount,installment_count,first_due_date) VALUES('card-plan','Laptop','bank','card',300000,3,'2026-12-25'),('legacy','Legacy loan','bank','loan',100000,2,'2026-12-25')").execute(&pool).await.unwrap();
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "debt:loan".into(),
                month: "2026-12".into(),
                amount: Some("1200000".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.debt_accounts.len(), 1);
        assert_eq!(result.debt_accounts[0].current_balance, "9007199254740993");
        assert_eq!(result.installments.len(), 2);
        assert!(result.installments.iter().all(|i| i.accounts_available));
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "installment:card-plan".into(),
                month: "2026-12".into(),
                amount: Some("0".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            result
                .amounts
                .iter()
                .find(|a| a.item_id == "installment:card-plan")
                .unwrap()
                .amount,
            "0"
        );
        assert_eq!(
            result
                .amounts
                .iter()
                .find(|a| a.item_id == "debt:loan")
                .unwrap()
                .amount,
            "1200000"
        );
        for id in [
            "debt:bank",
            "debt:card",
            "debt:missing",
            "installment:legacy",
            "installment:missing",
        ] {
            assert!(apply(
                &pool,
                Change::Amount {
                    item_id: id.into(),
                    month: "2026-12".into(),
                    amount: Some("1".into())
                }
            )
            .await
            .is_err());
        }
        for amount in ["-1", "1.01", "9223372036854775808"] {
            assert!(apply(
                &pool,
                Change::Amount {
                    item_id: "installment:card-plan".into(),
                    month: "2026-12".into(),
                    amount: Some(amount.into())
                }
            )
            .await
            .is_err());
        }
        let transactions: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(transactions, 0);
        let balance: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id='card'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 20000);
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='bank'")
            .execute(&pool)
            .await
            .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result.installments.iter().all(|i| !i.accounts_available));
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "installment:card-plan".into(),
                month: "2026-12".into(),
                amount: None,
            },
        )
        .await
        .unwrap();
        assert!(!result
            .amounts
            .iter()
            .any(|a| a.item_id == "installment:card-plan"));
        sqlx::query("UPDATE settings SET currency='USD'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "debt:loan".into(),
                month: "2026-12".into(),
                amount: Some("1".into())
            }
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn debt_migration_only_removes_untouched_placeholders() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 14)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE planner_items SET name='My mortgage' WHERE id='debt-0'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO planner_amounts VALUES('installments-0','2026-12',0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result
            .items
            .iter()
            .any(|i| i.id == "debt-0" && i.name == "My mortgage"));
        assert!(result.items.iter().any(|i| i.id == "installments-0"));
        assert!(!result
            .items
            .iter()
            .any(|i| i.id == "debt-1" || i.id == "installments-1"));
        assert_eq!(result.amounts[0].amount, "0");
    }

    #[tokio::test]
    async fn snapshot_includes_exact_card_transactions_and_archived_card_history() {
        let pool = database().await;
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(id,name,type,opening_balance,current_balance,is_archived) VALUES('bank','Bank','bank',0,0,1),('card','Old Visa','credit_card',0,0,1)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('bill','repayment','bank','card',9007199254740993,'2026-12-25',''),('purchase','expense','card',NULL,10000,'2026-12-26','')").execute(&pool).await.unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(result.credit_cards.len(), 1);
        assert!(result.credit_cards[0].is_archived);
        assert_eq!(result.card_transactions.len(), 2);
        let bill = result
            .card_transactions
            .iter()
            .find(|t| t.id == "bill")
            .unwrap();
        assert_eq!(bill.amount, "9007199254740993");
        assert_eq!(bill.account_type, "bank");
        assert_eq!(
            bill.destination_account_type.as_deref(),
            Some("credit_card")
        );
        sqlx::query("DELETE FROM transactions WHERE id='bill'")
            .execute(&pool)
            .await
            .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(result.card_transactions.len(), 1);
        assert_eq!(result.card_transactions[0].kind, "expense");
    }

    #[tokio::test]
    async fn card_transaction_migration_preserves_custom_rows_and_zero_entries() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 15)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE planner_items SET name='Extra payment' WHERE id='cards-0'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO planner_amounts VALUES('cards-1','2026-12',0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0015_planner_card_transactions.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0016_planner_titles.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result
            .items
            .iter()
            .any(|i| i.id == "cards-0" && i.name == "Extra payment"));
        assert!(result.items.iter().any(|i| i.id == "cards-1"));
        assert!(!result
            .items
            .iter()
            .any(|i| i.id == "cards-2" || i.id == "cards-3"));
        assert_eq!(
            result
                .categories
                .iter()
                .find(|c| c.id == "cards")
                .unwrap()
                .name,
            "Credit Cards"
        );
    }

    #[tokio::test]
    async fn salary_deduction_snapshot_and_cycle_override_keep_gross_intact() {
        let pool = database().await;
        sqlx::query("UPDATE settings SET currency='THB'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO accounts(id,name,type,opening_balance) VALUES('bank','Bank','bank',0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary','bank','salary',5000000,25)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,description,amount) VALUES('tax','salary','Tax','',200000)")
            .execute(&pool)
            .await
            .unwrap();
        let result = apply(
            &pool,
            Change::Amount {
                item_id: "deduction:tax".into(),
                month: "2026-12".into(),
                amount: Some("0".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.incomes[0].estimated_amount, "5000000");
        assert_eq!(result.income_deductions[0].amount, "200000");
        assert_eq!(result.amounts[0].item_id, "deduction:tax");
        assert_eq!(result.amounts[0].amount, "0");
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "deduction:missing".into(),
                month: "2026-12".into(),
                amount: Some("1".into())
            }
        )
        .await
        .is_err());
        sqlx::query("UPDATE settings SET currency='USD'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(apply(
            &pool,
            Change::Amount {
                item_id: "deduction:tax".into(),
                month: "2026-12".into(),
                amount: Some("1".into())
            }
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn salary_migration_preserves_existing_manual_deductions() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version < 17)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO planner_amounts VALUES('deductions-0','2026-12',0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE planner_items SET name='My social security' WHERE id='deductions-1'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result.items.iter().any(|i| i.id == "deductions-0"));
        assert!(result.items.iter().any(|i| i.name == "My social security"));
        assert!(!result.items.iter().any(|i| i.id == "deductions-2"));
        assert_eq!(result.amounts[0].amount, "0");
        assert!(result.income_deductions.is_empty());
    }

    #[tokio::test]
    async fn planner_survives_reopening_database_and_migration_rerun() {
        let path = std::env::temp_dir().join(format!(
            "heyday-planner-{}-{}.db",
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
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        apply(
            &pool,
            Change::Amount {
                item_id: "expenses-0".into(),
                month: "2026-12".into(),
                amount: Some("9007199254740993".into()),
            },
        )
        .await
        .unwrap();
        apply(
            &pool,
            Change::Status {
                month: "2026-12".into(),
                status: "complete".into(),
            },
        )
        .await
        .unwrap();
        apply(
            &pool,
            Change::Opening {
                month: "2026-11".into(),
                amount: Some("-10000".into()),
            },
        )
        .await
        .unwrap();
        pool.close().await;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let data = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(data.amounts[0].amount, "9007199254740993");
        assert_eq!(data.months[0].status, "complete");
        assert_eq!(data.opening.unwrap().amount, "-10000");
        assert_eq!(data.items.len(), 10);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
