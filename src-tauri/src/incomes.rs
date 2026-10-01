use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Income {
    id: String,
    name: String,
    destination_account_id: String,
    destination_account_name: String,
    #[serde(rename = "type")]
    income_type: String,
    estimated_amount: String,
    deductions_total: String,
    recurrence_frequency: String,
    recurrence_day_of_month: i64,
    is_auto_create_transaction: bool,
    is_active: bool,
    created_at: String,
    updated_at: String,
}

pub(crate) const SELECT: &str = "SELECT i.id, i.name, i.destination_account_id, a.name AS destination_account_name, i.type AS income_type, CAST(i.estimated_amount AS TEXT) AS estimated_amount, CAST(COALESCE((SELECT SUM(d.amount) FROM income_deductions d WHERE d.income_id=i.id),0) AS TEXT) AS deductions_total, i.recurrence_frequency, i.recurrence_day_of_month, i.is_auto_create_transaction, i.is_active, i.created_at, i.updated_at FROM incomes i JOIN accounts a ON a.id = i.destination_account_id";

#[derive(Deserialize)]
pub struct NewIncome {
    #[serde(default)]
    deductions: Vec<crate::income_deductions::DeductionInput>,
    name: String,
    destination_account_id: String,
    #[serde(rename = "type")]
    income_type: String,
    estimated_amount: String,
    currency: String,
    recurrence_frequency: String,
    recurrence_day_of_month: i64,
    is_auto_create_transaction: bool,
    is_active: bool,
}

fn validate_definition(input: &NewIncome) -> Result<i64, String> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter an income name between 1 and 100 characters.".into());
    }
    if !["salary", "variable", "investment", "other"].contains(&input.income_type.as_str()) {
        return Err("Choose a supported income type.".into());
    }
    let amount = input
        .estimated_amount
        .parse::<i64>()
        .map_err(|_| "Estimated amount is outside the supported range.")?;
    if amount < 0 {
        return Err("Estimated amount cannot be negative.".into());
    }
    crate::income_deductions::validate(&input.deductions, amount, input.income_type == "salary")?;
    if input.recurrence_frequency != "monthly" {
        return Err("Only monthly recurrence is currently supported.".into());
    }
    if !(1..=31).contains(&input.recurrence_day_of_month) {
        return Err("Income day must be between 1 and 31.".into());
    }
    if input.is_auto_create_transaction {
        return Err("Automatic transaction creation is not available yet.".into());
    }
    Ok(amount)
}

pub async fn insert_income(pool: &SqlitePool, input: NewIncome) -> Result<Income, String> {
    let amount = validate_definition(&input)?;
    let name = input.name.trim();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err(
            "Choose the app currency in Settings, then reload income before saving.".into(),
        );
    }
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ? AND is_archived = 0)",
    )
    .bind(&input.destination_account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if !valid {
        return Err("Choose an existing, active destination account.".into());
    }
    let id: String = sqlx::query_scalar("INSERT INTO incomes (id, name, destination_account_id, type, estimated_amount, recurrence_frequency, recurrence_day_of_month, is_auto_create_transaction, is_active) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, 'monthly', ?, 0, ?) RETURNING id")
        .bind(name).bind(input.destination_account_id).bind(input.income_type).bind(amount)
        .bind(input.recurrence_day_of_month).bind(input.is_active)
        .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    crate::income_deductions::replace(&mut tx, &id, &input.deductions).await?;
    let income = sqlx::query_as::<_, Income>(&format!("{SELECT} WHERE i.id = ?"))
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(income)
}

#[derive(Deserialize)]
pub struct UpdateIncome {
    id: String,
    #[serde(flatten)]
    changes: NewIncome,
}
async fn update(pool: &SqlitePool, input: UpdateIncome) -> Result<Income, String> {
    let amount = validate_definition(&input.changes)?;
    if !input.changes.deductions.is_empty() {
        return Err("Manage existing salary deductions separately in Income.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1").fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.changes.currency.as_str()) {
        return Err("Currency changed. Reload Income before saving.".into());
    }
    let existing: Option<(String, String)> = sqlx::query_as("SELECT type,destination_account_id FROM incomes WHERE id=?").bind(&input.id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
    let (kind, destination) = existing.ok_or("Income source no longer exists. Reload to continue.")?;
    if kind != input.changes.income_type {
        return Err("Income type cannot be changed. Add a separate source for a different type.".into());
    }
    let deductions: Vec<i64> = sqlx::query_scalar("SELECT amount FROM income_deductions WHERE income_id=?").bind(&input.id).fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
    let total = deductions.into_iter().try_fold(0i64, |sum, value| sum.checked_add(value)).ok_or("Deductions exceed the supported range.")?;
    if total > amount { return Err("Estimated gross salary cannot be lower than its existing deductions. Adjust deductions in Income first.".into()); }
    let available: Option<bool> = sqlx::query_scalar("SELECT is_archived=0 FROM accounts WHERE id=?").bind(&input.changes.destination_account_id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
    if available != Some(true) && !(available == Some(false) && destination == input.changes.destination_account_id && !input.changes.is_active) {
        return Err("Choose an active destination account. An inactive source may keep its existing archived destination.".into());
    }
    sqlx::query("UPDATE incomes SET name=?,destination_account_id=?,estimated_amount=?,recurrence_day_of_month=?,is_active=?,is_auto_create_transaction=0,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(input.changes.name.trim()).bind(input.changes.destination_account_id).bind(amount).bind(input.changes.recurrence_day_of_month).bind(input.changes.is_active).bind(&input.id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let result = sqlx::query_as::<_, Income>(&format!("{SELECT} WHERE i.id=?")).bind(&input.id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(result)
}
#[tauri::command]
pub async fn update_income(pool: tauri::State<'_, SqlitePool>, input: UpdateIncome) -> Result<Income, String> {
    let _database_operation = crate::backups::operation()?;
    update(pool.inner(), input).await
}

#[tauri::command]
pub async fn create_income(
    pool: tauri::State<'_, SqlitePool>,
    input: NewIncome,
) -> Result<Income, String> {
    let _database_operation = crate::backups::operation()?;
    insert_income(pool.inner(), input).await
}

#[tauri::command]
pub async fn list_incomes(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Income>, String> {
    let _database_operation = crate::backups::operation()?;
    sqlx::query_as::<_, Income>(&format!("{SELECT} ORDER BY i.created_at, i.id"))
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    fn input() -> NewIncome {
        NewIncome {
            deductions: vec![],
            name: " Salary ".into(),
            destination_account_id: "bank".into(),
            income_type: "salary".into(),
            estimated_amount: "9007199254740993".into(),
            currency: "THB".into(),
            recurrence_frequency: "monthly".into(),
            recurrence_day_of_month: 31,
            is_auto_create_transaction: false,
            is_active: true,
        }
    }
    async fn seed(pool: &SqlitePool) {
        sqlx::migrate!("./migrations").run(pool).await.unwrap();
        crate::save_currency(pool, "THB").await.unwrap();
        sqlx::query("INSERT INTO accounts (id, name, type, opening_balance) VALUES ('bank', 'Everyday bank', 'bank', 150000)").execute(pool).await.unwrap();
    }
    #[tokio::test]
    async fn source_edits_preserve_receipts_deductions_overrides_and_balances() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
        seed(&pool).await;
        sqlx::query("INSERT INTO accounts(id,name,type,current_balance) VALUES('other','Other bank','bank',99)").execute(&pool).await.unwrap();
        let source = insert_income(&pool,input()).await.unwrap();
        sqlx::query("UPDATE incomes SET created_at='2000-01-01 00:00:00' WHERE id=?").bind(&source.id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,amount) VALUES('tax',?,'Tax',100)").bind(&source.id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO planner_income_amounts VALUES(?,'2026-01',0)").bind(&source.id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO planner_deduction_amounts VALUES('tax','2026-01',0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description,income_source_id) VALUES('receipt','income','bank',200,'2026-01-01','Salary receipt',?)").bind(&source.id).execute(&pool).await.unwrap();
        let before:Vec<(String,i64)>=sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id").fetch_all(&pool).await.unwrap();
        let mut changes=input();changes.name=" Updated salary ".into();changes.destination_account_id="other".into();changes.recurrence_day_of_month=28;changes.estimated_amount="9223372036854775807".into();changes.is_active=false;
        let updated=update(&pool,UpdateIncome{id:source.id.clone(),changes}).await.unwrap();
        assert_eq!(updated.id,source.id);assert_eq!(updated.name,"Updated salary");assert_eq!(updated.created_at,"2000-01-01 00:00:00");
        assert_eq!(updated.destination_account_id,"other");assert_eq!(updated.recurrence_day_of_month,28);assert_eq!(updated.estimated_amount,"9223372036854775807");assert_eq!(updated.deductions_total,"100");assert!(!updated.is_active);assert!(!updated.is_auto_create_transaction);
        let receipt:(String,String,i64,String)=sqlx::query_as("SELECT income_source_id,account_id,amount,description FROM transactions WHERE id='receipt'").fetch_one(&pool).await.unwrap();
        assert_eq!(receipt,(source.id.clone(),"bank".into(),200,"Salary receipt".into()));
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT amount FROM planner_income_amounts WHERE income_id=?").bind(&source.id).fetch_one(&pool).await.unwrap(),0);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT amount FROM planner_deduction_amounts WHERE deduction_id='tax'").fetch_one(&pool).await.unwrap(),0);
        assert_eq!(sqlx::query_scalar::<_,String>("SELECT income_id FROM income_deductions WHERE id='tax'").fetch_one(&pool).await.unwrap(),source.id);
        let after:Vec<(String,i64)>=sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id").fetch_all(&pool).await.unwrap();assert_eq!(before,after);
        // Reactivation restores generated estimates while retaining the same source.
        let mut changes=input();changes.is_active=true;
        assert!(update(&pool,UpdateIncome{id:updated.id,changes}).await.unwrap().is_active);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM incomes").fetch_one(&pool).await.unwrap(),1);
    }
    #[tokio::test]
    async fn invalid_edits_roll_back_and_archived_destination_can_be_retained_only_when_inactive() {
        let pool=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();seed(&pool).await;
        let source=insert_income(&pool,input()).await.unwrap();
        sqlx::query("INSERT INTO income_deductions(id,income_id,name,amount) VALUES('tax',?,'Tax',100)").bind(&source.id).execute(&pool).await.unwrap();
        let mut cases=Vec::new();
        let mut value=input();value.estimated_amount="99".into();cases.push(value);
        let mut value=input();value.estimated_amount="-1".into();cases.push(value);
        let mut value=input();value.estimated_amount="9223372036854775808".into();cases.push(value);
        let mut value=input();value.name=" ".into();cases.push(value);
        let mut value=input();value.name="x".repeat(101);cases.push(value);
        let mut value=input();value.income_type="other".into();cases.push(value);
        let mut value=input();value.currency="USD".into();cases.push(value);
        let mut value=input();value.destination_account_id="missing".into();cases.push(value);
        let mut value=input();value.recurrence_frequency="weekly".into();cases.push(value);
        for day in [0,32] {let mut value=input();value.recurrence_day_of_month=day;cases.push(value);}
        let mut value=input();value.is_auto_create_transaction=true;cases.push(value);
        let mut value=input();value.deductions=vec![crate::income_deductions::DeductionInput {id:Some("tax".into()),debt_account_id:None,name:"Tax".into(),description:"".into(),amount:"0".into()}];cases.push(value);
        for changes in cases {
            assert!(update(&pool,UpdateIncome{id:source.id.clone(),changes}).await.is_err());
            let current:Income=sqlx::query_as(&format!("{SELECT} WHERE i.id=?")).bind(&source.id).fetch_one(&pool).await.unwrap();
            assert_eq!(current.name,source.name);assert_eq!(current.estimated_amount,source.estimated_amount);assert!(current.is_active);
        }
        assert!(update(&pool,UpdateIncome{id:"missing".into(),changes:input()}).await.is_err());
        sqlx::query("UPDATE accounts SET is_archived=1 WHERE id='bank'").execute(&pool).await.unwrap();
        assert!(update(&pool,UpdateIncome{id:source.id.clone(),changes:input()}).await.is_err());
        let mut changes=input();changes.is_active=false;
        let disabled=update(&pool,UpdateIncome{id:source.id.clone(),changes}).await.unwrap();assert!(!disabled.is_active);assert_eq!(disabled.destination_account_id,"bank");
        sqlx::query("INSERT INTO accounts(id,name,type,is_archived) VALUES('archived','Archived bank','bank',1)").execute(&pool).await.unwrap();
        let mut changes=input();changes.is_active=false;changes.destination_account_id="archived".into();
        assert!(update(&pool,UpdateIncome{id:source.id,changes}).await.is_err());
    }

    #[tokio::test]
    async fn create_salary_and_deductions_atomically_and_reject_other_types() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        seed(&pool).await;
        let mut salary = input();
        salary.estimated_amount = "5000000".into();
        salary.deductions = vec![crate::income_deductions::DeductionInput {
            debt_account_id: None,
            id: None,
            name: "Tax".into(),
            description: "".into(),
            amount: "275000".into(),
        }];
        let result = insert_income(&pool, salary).await.unwrap();
        assert_eq!(result.estimated_amount, "5000000");
        assert_eq!(result.deductions_total, "275000");
        let mut invalid = input();
        invalid.income_type = "other".into();
        invalid.deductions = vec![crate::income_deductions::DeductionInput {
            debt_account_id: None,
            id: None,
            name: "Tax".into(),
            description: "".into(),
            amount: "1".into(),
        }];
        assert!(insert_income(&pool, invalid).await.is_err());
        let mut stale = input();
        stale.deductions = vec![crate::income_deductions::DeductionInput {
            debt_account_id: None,
            id: Some("unowned".into()),
            name: "Tax".into(),
            description: "".into(),
            amount: "1".into(),
        }];
        assert!(insert_income(&pool, stale).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM incomes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let balance: i64 =
            sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 0);
    }

    #[tokio::test]
    async fn rejects_invalid_sources_without_writes() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        seed(&pool).await;
        let mut cases = vec![];
        let mut value = input();
        value.name = " ".into();
        cases.push(value);
        let mut value = input();
        value.name = "a".repeat(101);
        cases.push(value);
        let mut value = input();
        value.income_type = "unknown".into();
        cases.push(value);
        let mut value = input();
        value.estimated_amount = "-1".into();
        cases.push(value);
        let mut value = input();
        value.estimated_amount = "9223372036854775808".into();
        cases.push(value);
        let mut value = input();
        value.destination_account_id = "missing".into();
        cases.push(value);
        let mut value = input();
        value.currency = "USD".into();
        cases.push(value);
        let mut value = input();
        value.recurrence_frequency = "weekly".into();
        cases.push(value);
        let mut value = input();
        value.recurrence_day_of_month = 0;
        cases.push(value);
        let mut value = input();
        value.recurrence_day_of_month = 32;
        cases.push(value);
        let mut value = input();
        value.is_auto_create_transaction = true;
        cases.push(value);
        for value in cases {
            assert!(insert_income(&pool, value).await.is_err());
        }
        sqlx::query("UPDATE accounts SET is_archived = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(insert_income(&pool, input()).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM incomes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    #[tokio::test]
    async fn sources_persist_without_changing_account_balances() {
        let path = std::env::temp_dir().join(format!(
            "heyday-income-{}-{}.db",
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
        for (index, kind) in ["salary", "variable", "investment", "other"]
            .iter()
            .enumerate()
        {
            let mut value = input();
            value.income_type = kind.to_string();
            value.is_active = index != 3;
            let result = insert_income(&pool, value).await.unwrap();
            assert_eq!(result.name, "Salary");
            assert_eq!(result.destination_account_name, "Everyday bank");
            assert_eq!(result.estimated_amount, "9007199254740993");
            assert_eq!(result.is_active, index != 3);
            assert!(!result.is_auto_create_transaction);
        }
        pool.close().await;
        let reopened = SqlitePool::connect_with(options).await.unwrap();
        sqlx::migrate!("./migrations").run(&reopened).await.unwrap();
        let records = sqlx::query_as::<_, Income>(SELECT)
            .fetch_all(&reopened)
            .await
            .unwrap();
        assert_eq!(records.len(), 4);
        let balance: i64 =
            sqlx::query_scalar("SELECT opening_balance FROM accounts WHERE id = 'bank'")
                .fetch_one(&reopened)
                .await
                .unwrap();
        assert_eq!(balance, 150000);
        let transactions: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
            .fetch_one(&reopened)
            .await
            .unwrap();
        assert_eq!(transactions, 0);
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
