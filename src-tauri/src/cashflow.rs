//! Desktop IPC adapters; financial operations live in heyday-core.
use heyday_core::services::cashflow as service;
pub use heyday_core::services::cashflow::*;
use sqlx::SqlitePool;
#[cfg(test)]
use sqlx::SqliteConnection;

#[tauri::command]
pub async fn get_cashflow_planner(pool: tauri::State<'_, SqlitePool>) -> Result<Planner, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::get_cashflow_planner(pool.inner()).await
}

#[tauri::command]
pub async fn save_cashflow_planner(
    pool: tauri::State<'_, SqlitePool>,
    input: Change,
) -> Result<Planner, String> {
    let _operation = heyday_core::database::operations::operation()?;
    service::save_cashflow_planner(pool.inner(), input).await
}
#[cfg(test)]
pub async fn snapshot(conn: &mut SqliteConnection) -> Result<Planner, sqlx::Error> {
    service::snapshot(conn).await
}
#[cfg(test)]
pub async fn apply(pool: &SqlitePool, input: Change) -> Result<Planner, String> {
    service::apply(pool, input).await
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
        assert_eq!(initial.expense_categories.len(), 11);
        assert!(initial.expense_categories.iter().any(|category| category.name == "Fee/Interest"));
        let category_id = &initial.expense_categories[0].id;
        sqlx::query("UPDATE categories SET icon='coffee' WHERE id=?")
            .bind(category_id).execute(&pool).await.unwrap();
        let updated = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(updated.expense_categories.iter().find(|c| &c.id == category_id).unwrap().icon, "coffee");
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
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
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0012_cashflow_planner.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0013_planner_income.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0024_loan_monthly_installment.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0027_card_billing.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0028_card_billing_review.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0036_loan_payoffs.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0039_selective_defaults.sql")).execute(&pool).await.unwrap();
        // Adapt the historical fixture to the current snapshot fields.
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0010_irregular_income.sql").split("DROP TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
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
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0013_planner_income.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0024_loan_monthly_installment.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0027_card_billing.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0028_card_billing_review.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0036_loan_payoffs.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0039_selective_defaults.sql")).execute(&pool).await.unwrap();
        // Adapt the historical fixture to the current snapshot fields.
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0010_irregular_income.sql").split("DROP TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
        sqlx::query("UPDATE accounts SET monthly_installment=9007199254740993 WHERE id='loan'")
            .execute(&pool)
            .await
            .unwrap();
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
        assert_eq!(
            result.debt_accounts[0].monthly_installment.as_deref(),
            Some("9007199254740993")
        );
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
        crate::accounts::change_archive(&pool, "bank", true).await.unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(result.installments.iter().all(|i| !i.accounts_available));
        assert!(result.incomes.iter().all(|i| i.account_archived));
        crate::accounts::change_archive(&pool, "bank", false).await.unwrap();
        let restored = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert!(restored.installments.iter().all(|i| i.accounts_available));
        assert!(restored.incomes.iter().all(|i| !i.account_archived));
        crate::accounts::change_archive(&pool, "bank", true).await.unwrap();
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
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
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0014_planner_debt_sources.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0024_loan_monthly_installment.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0027_card_billing.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0028_card_billing_review.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0036_loan_payoffs.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0039_selective_defaults.sql")).execute(&pool).await.unwrap();
        // Adapt the historical fixture to the current snapshot fields.
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0010_irregular_income.sql").split("DROP TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
    async fn snapshot_identifies_linked_loan_charges_independently_of_category() {
        let pool = database().await;
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,loan_type) VALUES('bank','Bank','bank',NULL),('loan','Loan','loan','personal_loan'); INSERT INTO loan_facilities VALUES('loan',2000000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','loan','Contract',2000000,2000000,'2024-01-01','bank','bank',0,173367,12,'2024-02-01','existing'); INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('charge','expense','bank',77754,'2024-02-01',''),('other','expense','bank',100,'2024-02-01',''); INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount) VALUES('charge','payment','contract','interest',77754);")
            .execute(&pool).await.unwrap();
        let result = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        let charge = result.ledger_transactions.iter().find(|t|t.id=="charge").unwrap();
        assert_eq!(charge.loan_account_id.as_deref(), Some("loan"));
        assert_eq!(charge.loan_component.as_deref(), Some("interest"));
        let other = result.ledger_transactions.iter().find(|t|t.id=="other").unwrap();
        assert!(other.loan_account_id.is_none() && other.loan_component.is_none());
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
        assert_eq!(result.ledger_transactions.len(), 2);
        assert_eq!(result.ledger_transactions.iter().find(|t|t.id=="bill").unwrap().account_name,"Bank");
        // Non-card expenses must be part of the same snapshot, including archived accounts.
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('food','expense','bank',500,'2026-12-26','Lunch')").execute(&pool).await.unwrap();
        let refreshed = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(refreshed.ledger_transactions.len(),3);
        assert_eq!(refreshed.card_transactions.len(),2);
        assert_eq!(refreshed.ledger_transactions.iter().find(|t|t.id=="food").unwrap().amount,"500");
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
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
            "../tests/fixtures/alpha-migrations/0015_planner_card_transactions.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0016_planner_titles.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0024_loan_monthly_installment.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0027_card_billing.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0028_card_billing_review.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0036_loan_payoffs.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0039_selective_defaults.sql")).execute(&pool).await.unwrap();
        // Adapt the historical fixture to the current snapshot fields.
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0010_irregular_income.sql").split("DROP TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
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
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0017_salary_deductions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0018_deduction_debt_account.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../tests/fixtures/alpha-migrations/0024_loan_monthly_installment.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0025_loan_contracts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0027_card_billing.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0028_card_billing_review.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0029_transaction_income_source.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0030_category_icons.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0036_loan_payoffs.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0038_account_loan_repayments.sql")).execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0039_selective_defaults.sql")).execute(&pool).await.unwrap();
        // Adapt the historical fixture to the current snapshot fields.
        sqlx::raw_sql(include_str!("../../crates/heyday-core/migrations/0010_irregular_income.sql").split("DROP TRIGGER").next().unwrap()).execute(&pool).await.unwrap();
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
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
        heyday_core::database::MIGRATOR.run(&pool).await.unwrap();
        let data = snapshot(&mut *pool.acquire().await.unwrap()).await.unwrap();
        assert_eq!(data.amounts[0].amount, "9007199254740993");
        assert_eq!(data.months[0].status, "complete");
        assert_eq!(data.opening.unwrap().amount, "-10000");
        assert_eq!(data.items.len(), 10);
        pool.close().await;
        crate::test_support::remove_file(&path).await.unwrap();
    }
}
