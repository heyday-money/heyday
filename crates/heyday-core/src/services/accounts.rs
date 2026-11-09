use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Account {
    #[sqlx(default)]
    pub payroll_linked: bool,
    #[sqlx(default)]
    pub paid_off_on: Option<String>,
    pub is_archived: bool,
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub loan_type: Option<String>,
    // Amounts cross IPC as decimal integer strings, never lossy JS numbers.
    pub opening_balance: String,
    pub current_balance: String,
    pub institution: Option<String>,
    pub last_four: Option<String>,
    pub notes: Option<String>,
    pub credit_limit: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub interest_rate_ten_thousandths: Option<i64>,
    pub monthly_installment: Option<String>,
    pub initial_loan_amount: Option<String>,
}

pub const COLUMNS: &str = "is_archived, id, name, type AS account_type, loan_type, CAST(opening_balance AS TEXT) AS opening_balance, CAST(current_balance AS TEXT) AS current_balance, institution, last_four, notes, CAST(credit_limit AS TEXT) AS credit_limit, statement_day, payment_due_day, interest_rate_ten_thousandths, CAST(monthly_installment AS TEXT) AS monthly_installment, CAST(initial_loan_amount AS TEXT) AS initial_loan_amount";

pub const PAYROLL_COLUMN: &str = "EXISTS(SELECT 1 FROM income_deductions d JOIN incomes i ON i.id=d.income_id WHERE d.debt_account_id=accounts.id AND i.type='salary') AS payroll_linked";

#[derive(Deserialize)]
pub struct NewAccount {
    pub revolving_credit_limit: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub loan_type: Option<String>,
    pub opening_balance: String,
    pub currency: String,
    pub institution: Option<String>,
    pub last_four: Option<String>,
    pub notes: Option<String>,
    pub credit_limit: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub interest_rate_ten_thousandths: Option<i64>,
    pub monthly_installment: Option<String>,
    pub initial_loan_amount: Option<String>,
    pub institution_id: Option<String>,
}

pub fn optional_text(value: Option<String>, limit: usize) -> Result<Option<String>, String> {
    let value = value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if value.as_ref().is_some_and(|s| s.chars().count() > limit) {
        return Err(format!("Text must be no longer than {limit} characters."));
    }
    Ok(value)
}

pub fn amount(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| "Amount is outside the supported range.".into())
}

pub fn validate_account(
    mut input: NewAccount,
    allow_unclassified: bool,
) -> Result<NewAccount, String> {
    let kind = input.account_type.as_str();
    if ![
        "cash",
        "bank",
        "wallet",
        "credit_card",
        "loan",
        "investment",
    ]
    .contains(&kind)
    {
        return Err("Choose a supported account type.".into());
    }
    if kind == "loan" && !(allow_unclassified && input.loan_type.is_none()) {
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
    } else if kind != "loan" && input.loan_type.is_some() {
        return Err("Loan type applies only to loan accounts.".into());
    }
    let name = optional_text(Some(input.name), 100)?.ok_or("Enter an account name.")?;
    let institution = optional_text(input.institution, 100)?;
    let notes = optional_text(input.notes, 1000)?;
    let last_four = optional_text(input.last_four, 4)?;
    if let Some(last) = &last_four {
        if last.len() != 4 || !last.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Last four digits must contain exactly four digits.".into());
        }
    }
    amount(&input.opening_balance)?;
    // Signed balances are allowed: negative debt represents a credit/overpayment.
    let limit = input.credit_limit.as_deref().map(amount).transpose()?;
    if limit.is_some_and(|v| v < 0)
        || (kind != "credit_card" && (limit.is_some() || input.statement_day.is_some()))
    {
        return Err(
            "Only credit cards can have a non-negative credit limit and statement day.".into(),
        );
    }
    if let Some(value) = input.monthly_installment.as_deref() {
        if kind != "loan" || amount(value)? < 0 {
            return Err(
                "Monthly installment must be a non-negative amount on a loan account.".into(),
            );
        }
    }
    if let Some(value) = input.initial_loan_amount.as_deref() {
        if kind != "loan" || amount(value)? < 0 {
            return Err(
                "Initial loan amount must be a non-negative amount on a loan account.".into(),
            );
        }
    }
    let debt = ["credit_card", "loan"].contains(&kind);
    if !debt && (input.payment_due_day.is_some() || input.interest_rate_ten_thousandths.is_some()) {
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
        .interest_rate_ten_thousandths
        .is_some_and(|v| !(0..=1000000).contains(&v))
    {
        return Err("Annual interest rate must be between 0 and 100%.".into());
    }
    input.name = name;
    input.institution = institution;
    input.notes = notes;
    input.last_four = last_four;
    Ok(input)
}

pub async fn insert_account(pool: &SqlitePool, input: NewAccount) -> Result<Account, String> {
    insert_with_logo(pool, input, None).await
}
pub async fn insert_with_logo(
    pool: &SqlitePool,
    input: NewAccount,
    logo_change: Option<crate::logos::LogoChange>,
) -> Result<Account, String> {
    let input = validate_account(input, false)?;
    let revolving_limit = input
        .revolving_credit_limit
        .as_deref()
        .map(amount)
        .transpose()?;
    if revolving_limit.is_some_and(|v| v < 0)
        || (revolving_limit.is_some()
            && (input.account_type != "loan"
                || input.loan_type.as_deref() != Some("personal_loan")))
    {
        return Err(
            "Revolving credit requires a Personal Loan and non-negative credit limit.".into(),
        );
    }
    let balance = amount(&input.opening_balance)?;
    let limit = input.credit_limit.as_deref().map(amount).transpose()?;
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
    let (institution_id, institution) = crate::institutions::resolve(
        &mut tx,
        &input.account_type,
        input.institution_id.as_deref(),
        input.institution.as_deref(),
        None,
    )
    .await?;
    let query = format!("INSERT INTO accounts (id, name, type, opening_balance, current_balance, institution, last_four, notes, credit_limit, statement_day, payment_due_day, interest_rate_ten_thousandths, loan_type, monthly_installment, initial_loan_amount, institution_id) VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING {COLUMNS}");
    let account = sqlx::query_as::<_, Account>(&query)
        .bind(input.name)
        .bind(input.account_type)
        .bind(balance)
        .bind(balance)
        .bind(institution)
        .bind(input.last_four)
        .bind(input.notes)
        .bind(limit)
        .bind(input.statement_day)
        .bind(input.payment_due_day)
        .bind(input.interest_rate_ten_thousandths)
        .bind(input.loan_type)
        .bind(
            input
                .monthly_installment
                .as_deref()
                .map(amount)
                .transpose()?,
        )
        .bind(
            input
                .initial_loan_amount
                .as_deref()
                .map(amount)
                .transpose()?,
        )
        .bind(institution_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(limit) = revolving_limit {
        sqlx::query("INSERT INTO loan_facilities(account_id,credit_limit) VALUES (?,?)")
            .bind(&account.id)
            .bind(limit)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    crate::logos::save_account(&mut tx, &account.id, logo_change.as_ref()).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(account)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountUpdate {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub loan_type: Option<String>,
    pub currency: String,
    pub institution: Option<String>,
    pub last_four: Option<String>,
    pub notes: Option<String>,
    pub credit_limit: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub interest_rate_ten_thousandths: Option<i64>,
    pub monthly_installment: Option<String>,
    pub initial_loan_amount: Option<String>,
    pub institution_id: Option<String>,
}

pub async fn update(pool: &SqlitePool, input: AccountUpdate) -> Result<Account, String> {
    update_with_logo(pool, input, None).await
}
pub async fn update_with_logo(
    pool: &SqlitePool,
    input: AccountUpdate,
    logo_change: Option<crate::logos::LogoChange>,
) -> Result<Account, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let saved: Account = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM accounts WHERE id = ? AND is_archived = 0"
    ))
    .bind(&input.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("This account is no longer available. Reload accounts and try again.")?;
    if input.account_type != saved.account_type {
        return Err("Account type cannot be changed after creation.".into());
    }
    let payroll_linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM income_deductions d JOIN incomes i ON i.id=d.income_id WHERE d.debt_account_id=? AND i.type='salary')")
        .bind(&input.id).fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
    if payroll_linked
        && input
            .monthly_installment
            .as_deref()
            .map(amount)
            .transpose()?
            != saved
                .monthly_installment
                .as_deref()
                .map(amount)
                .transpose()?
    {
        return Err("Monthly installment is locked while linked to salary deductions. Manage the link in Income first.".into());
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if currency.as_deref() != Some(input.currency.as_str()) {
        return Err("Currency has changed. Reload accounts before saving.".into());
    }
    let details = validate_account(
        NewAccount {
            institution_id: input.institution_id,
            revolving_credit_limit: None,
            name: input.name,
            account_type: input.account_type,
            loan_type: input.loan_type,
            opening_balance: saved.opening_balance,
            currency: input.currency,
            institution: input.institution,
            last_four: input.last_four,
            notes: input.notes,
            credit_limit: input.credit_limit,
            statement_day: input.statement_day,
            payment_due_day: input.payment_due_day,
            interest_rate_ten_thousandths: input.interest_rate_ten_thousandths,
            monthly_installment: input.monthly_installment,
            initial_loan_amount: input.initial_loan_amount,
        },
        saved.loan_type.is_none(),
    )?;
    let existing_id: Option<String> =
        sqlx::query_scalar("SELECT institution_id FROM accounts WHERE id=?")
            .bind(&input.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (institution_id, institution) = crate::institutions::resolve(
        &mut tx,
        &details.account_type,
        details.institution_id.as_deref(),
        details.institution.as_deref(),
        existing_id.as_deref(),
    )
    .await?;
    // Neither opening nor current balance is accepted in the update payload or written here.
    let query = format!("UPDATE accounts SET name = ?, loan_type = ?, institution = ?, last_four = ?, notes = ?, credit_limit = ?, statement_day = ?, payment_due_day = ?, interest_rate_ten_thousandths = ?, monthly_installment = ?, initial_loan_amount = ?, institution_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING {COLUMNS}");
    let mut account: Account = sqlx::query_as(&query)
        .bind(details.name)
        .bind(details.loan_type)
        .bind(institution)
        .bind(details.last_four)
        .bind(details.notes)
        .bind(details.credit_limit.as_deref().map(amount).transpose()?)
        .bind(details.statement_day)
        .bind(details.payment_due_day)
        .bind(details.interest_rate_ten_thousandths)
        .bind(
            details
                .monthly_installment
                .as_deref()
                .map(amount)
                .transpose()?,
        )
        .bind(
            details
                .initial_loan_amount
                .as_deref()
                .map(amount)
                .transpose()?,
        )
        .bind(institution_id)
        .bind(input.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    crate::logos::save_account(&mut tx, &account.id, logo_change.as_ref()).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    account.payroll_linked = payroll_linked;
    Ok(account)
}

pub async fn update_account(
    pool: &SqlitePool,
    input: AccountUpdate,
    logo_change: Option<crate::logos::LogoChange>,
) -> Result<Account, String> {
    update_with_logo(pool, input, logo_change).await
}

pub async fn create_account(
    pool: &SqlitePool,
    input: NewAccount,
    logo_change: Option<crate::logos::LogoChange>,
) -> Result<Account, String> {
    insert_with_logo(pool, input, logo_change).await
}

pub async fn list_accounts(
    pool: &SqlitePool,
    include_paid_off: Option<bool>,
    include_archived: Option<bool>,
) -> Result<Vec<Account>, String> {
    let query = if include_archived.unwrap_or(false) {
        format!("SELECT {COLUMNS}, {PAYROLL_COLUMN}, (SELECT paid_off_on FROM loan_payoffs WHERE account_id=accounts.id) AS paid_off_on FROM accounts ORDER BY created_at,id")
    } else if include_paid_off.unwrap_or(false) {
        format!("SELECT {COLUMNS}, {PAYROLL_COLUMN}, (SELECT paid_off_on FROM loan_payoffs WHERE account_id=accounts.id) AS paid_off_on FROM accounts WHERE is_archived=0 OR id IN (SELECT account_id FROM loan_payoffs) ORDER BY created_at,id")
    } else {
        format!("SELECT {COLUMNS}, {PAYROLL_COLUMN} FROM accounts WHERE is_archived=0 ORDER BY created_at,id")
    };
    sqlx::query_as::<_, Account>(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

pub async fn change_archive(
    pool: &SqlitePool,
    account_id: &str,
    archived: bool,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let state: Option<(bool, bool)> = sqlx::query_as("SELECT is_archived, EXISTS(SELECT 1 FROM loan_payoffs WHERE account_id=accounts.id) FROM accounts WHERE id=?")
        .bind(account_id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
    let (current, paid_off) = state.ok_or("Account not found.")?;
    if paid_off {
        return Err(
            "Paid-off loans use the separate payoff flow and cannot be restored here.".into(),
        );
    }
    if current == archived {
        return Err("Account archive status has changed. Reload and try again.".into());
    }
    sqlx::query("UPDATE accounts SET is_archived=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(archived)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn set_account_archived(
    pool: &SqlitePool,
    account_id: String,
    archived: bool,
) -> Result<(), String> {
    change_archive(pool, &account_id, archived).await
}

pub async fn remove_archived(pool: &SqlitePool, account_id: &str) -> Result<(), String> {
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| e.to_string())?;
    let archived: Option<bool> = sqlx::query_scalar("SELECT is_archived FROM accounts WHERE id=?")
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if !archived.ok_or("Account not found.")? {
        return Err("Archive this account before deleting it.".into());
    }
    let transactions: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id=? OR destination_account_id=?)",
    )
    .bind(account_id)
    .bind(account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if transactions {
        return Err("Accounts with transaction records cannot be deleted.".into());
    }
    // Preserve children whose foreign keys would otherwise cascade on deletion.
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM planner_debt_amounts WHERE account_id=?) OR EXISTS(SELECT 1 FROM loan_payoffs WHERE account_id=?)")
        .bind(account_id).bind(account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if linked {
        return Err("This account has linked records. Remove its links before deleting it.".into());
    }
    sqlx::query("DELETE FROM accounts WHERE id=?")
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            if error.as_database_error().is_some_and(|error| {
                error.is_foreign_key_violation()
                    || error.message() == "FOREIGN KEY constraint failed"
            }) {
                "This account has linked records. Remove its links before deleting it.".to_owned()
            } else {
                error.to_string()
            }
        })?;
    crate::logos::prune(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn delete_account(pool: &SqlitePool, account_id: String) -> Result<(), String> {
    remove_archived(pool, &account_id).await
}

pub async fn mark_paid_off(pool: &SqlitePool, account_id: &str) -> Result<(), String> {
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| e.to_string())?;
    let account: Option<(String, i64, bool)> =
        sqlx::query_as("SELECT type,current_balance,is_archived FROM accounts WHERE id=?")
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (kind, balance, archived) = account.ok_or("Loan account not found.")?;
    if kind != "loan" || archived {
        return Err("Choose an active loan account.".into());
    }
    if balance != 0 {
        return Err("The loan balance must be zero. Record the final repayment or resolve any credit first.".into());
    }
    let unresolved: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=? AND (remaining_principal != 0 OR needs_review != 0))")
        .bind(account_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    if unresolved {
        return Err("Resolve remaining contract principal and contracts needing review before marking this loan paid off.".into());
    }
    let payroll: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM income_deductions WHERE debt_account_id=?)",
    )
    .bind(account_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if payroll {
        return Err(
            "Remove this loan's linked payroll deductions in Income before marking it paid off."
                .into(),
        );
    }
    let today: String = sqlx::query_scalar("SELECT date('now', 'localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO loan_payoffs(account_id,paid_off_on) VALUES (?,?)")
        .bind(account_id)
        .bind(today)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("UPDATE accounts SET is_archived=1,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn mark_loan_paid_off(pool: &SqlitePool, account_id: String) -> Result<(), String> {
    mark_paid_off(pool, &account_id).await
}
