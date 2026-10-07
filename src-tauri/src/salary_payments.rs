use crate::transactions::{insert_in_connection, NewTransaction};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
fn err(e: sqlx::Error) -> String {
    e.to_string()
}
#[derive(Deserialize, Serialize, Clone)]
pub struct Deduction {
    pub id: String,
    pub amount: String,
    pub debt_account_id: Option<String>,
    pub interest: String,
    pub fee: String,
    pub contract_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Breakdown {
    name: String,
    debt_account_name: Option<String>,
    principal: String,
    #[serde(flatten)]
    values: Deduction,
}
#[derive(Deserialize)]
pub struct PaymentInput {
    pub income_id: String,
    pub destination_account_id: String,
    pub occurrence: String,
    pub date: String,
    pub gross: String,
    pub currency: String,
    pub deductions: Vec<Deduction>,
    pub confirmed: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Payment {
    id: String,
    income_id: String,
    occurrence: String,
    date: String,
    gross: String,
    net: String,
    breakdown: String,
}
fn amount(s: &str) -> Result<i64, String> {
    s.parse::<i64>()
        .ok()
        .filter(|v| *v >= 0)
        .ok_or_else(|| "Enter a non-negative integer amount within the supported range.".into())
}
pub(crate) async fn record(pool: &SqlitePool, input: PaymentInput) -> Result<(), String> {
    if !input.confirmed {
        return Err("Confirm the salary payment before saving.".into());
    }
    if !crate::transactions::valid_date(&format!("{}-01", input.occurrence))
        || !crate::transactions::valid_date(&input.date)
    {
        return Err("Choose a valid salary month and payment date.".into());
    }
    let gross = amount(&input.gross)?;
    let mut tx = pool.begin().await.map_err(err)?;
    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if input.date > today || input.occurrence.as_str() > &today[..7] {
        return Err("Transactions cannot be dated in the future.".into());
    }
    let currency: Option<String> = sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
        .fetch_one(&mut *tx)
        .await
        .map_err(err)?;
    if currency.as_deref() != Some(&input.currency) {
        return Err("Currency changed. Reload transactions before saving.".into());
    }
    let (name,bank):(String,String)=sqlx::query_as("SELECT i.name,i.destination_account_id FROM incomes i JOIN accounts a ON a.id=i.destination_account_id WHERE i.id=? AND i.type='salary' AND i.is_active=1 AND a.is_archived=0 AND a.type IN ('cash','bank','wallet')")
        .bind(&input.income_id).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Choose an active salary with a cash, bank or wallet destination.")?;
    if bank != input.destination_account_id {
        return Err("Salary destination changed. Reload before recording.".into());
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM salary_payments WHERE income_id=? AND occurrence=?) OR EXISTS(SELECT 1 FROM transactions WHERE income_source_id=? AND (substr(date,1,7)=? OR substr(date,1,7)=?) AND id NOT IN (SELECT transaction_id FROM salary_payment_transactions))")
        .bind(&input.income_id).bind(&input.occurrence).bind(&input.income_id).bind(&input.date[..7]).bind(&input.occurrence).fetch_one(&mut *tx).await.map_err(err)?;
    if duplicate {
        return Err("A salary payment or linked income receipt already exists for this month. Review and reverse the existing entry before recording again.".into());
    }
    let definitions: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id,name,debt_account_id FROM income_deductions WHERE income_id=? ORDER BY id",
    )
    .bind(&input.income_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(err)?;
    if definitions.len() != input.deductions.len() || input.deductions.len() > 100 {
        return Err("Salary deductions changed. Reload before recording.".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut total = 0i64;
    let mut breakdown = Vec::new();
    for d in &input.deductions {
        let (_, label, debt) = definitions
            .iter()
            .find(|v| v.0 == d.id)
            .ok_or("Salary deductions changed. Reload before recording.")?;
        if &d.debt_account_id != debt {
            return Err("Salary deductions changed. Reload before recording.".into());
        }
        if !seen.insert(&d.id) {
            return Err("Duplicate salary deduction.".into());
        }
        let value = amount(&d.amount)?;
        let interest = amount(&d.interest)?;
        let fee = amount(&d.fee)?;
        let principal = value
            .checked_sub(interest)
            .and_then(|v| v.checked_sub(fee))
            .filter(|v| *v >= 0)
            .ok_or("Interest and fees cannot exceed the deduction.")?;
        total = total
            .checked_add(value)
            .ok_or("Deductions exceed the supported range.")?;
        let mut debt_name = None;
        if let Some(id) = debt {
            let (n,kind,balance):(String,String,i64)=sqlx::query_as("SELECT name,type,current_balance FROM accounts WHERE id=? AND is_archived=0 AND type IN ('loan','credit_card')").bind(id).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("Choose an active debt account for every linked deduction.")?;
            debt_name = Some(n);
            if kind == "credit_card" && (interest != 0 || fee != 0 || d.contract_id.is_some()) {
                return Err(
                    "Credit card payroll payments use the full deduction without a loan split."
                        .into(),
                );
            }
            if let Some(contract) = &d.contract_id {
                let updated=sqlx::query("UPDATE loan_contracts SET remaining_principal=remaining_principal-? WHERE id=? AND account_id=? AND needs_review=0 AND borrowing_date<=? AND remaining_principal>=?")
                    .bind(principal).bind(contract).bind(id).bind(&input.date).bind(principal).execute(&mut *tx).await.map_err(err)?;
                if updated.rows_affected() != 1 {
                    return Err(
                        "Choose a valid loan contract with sufficient remaining principal.".into(),
                    );
                }
            } else if kind == "loan" {
                let contracts: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM loan_contracts WHERE account_id=?)",
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?;
                if contracts {
                    return Err("Choose a contract for this loan repayment.".into());
                }
                if principal > balance.max(0) {
                    return Err("Principal payment exceeds this loan's balance.".into());
                }
            }
        } else if interest != 0 || fee != 0 || d.contract_id.is_some() {
            return Err("Only linked loans support principal, interest and fees.".into());
        }
        breakdown.push(Breakdown {
            name: label.clone(),
            debt_account_name: debt_name,
            principal: if debt.is_some() {
                principal.to_string()
            } else {
                "0".into()
            },
            values: d.clone(),
        });
    }
    let net = gross
        .checked_sub(total)
        .filter(|v| *v >= 0)
        .ok_or("Total deductions cannot exceed gross salary.")?;
    let id:String=sqlx::query_scalar("INSERT INTO salary_payments(id,income_id,occurrence,date,gross,net,breakdown) VALUES(lower(hex(randomblob(16))),?,?,?,?,?,?) RETURNING id")
        .bind(&input.income_id).bind(&input.occurrence).bind(&input.date).bind(gross).bind(net).bind(serde_json::to_string(&breakdown).map_err(|e|e.to_string())?).fetch_one(&mut *tx).await.map_err(err)?;
    for (account, value, role, part) in
        std::iter::once((bank, net, "net", None)).chain(breakdown.iter().filter_map(|b| {
            b.values.debt_account_id.as_ref().map(|a| {
                (
                    a.clone(),
                    b.principal.parse::<i64>().unwrap(),
                    "principal",
                    Some(b),
                )
            })
        }))
    {
        if value == 0 {
            continue;
        }
        let row = insert_in_connection(
            &mut tx,
            NewTransaction {
                kind: "income".into(),
                account_id: account,
                destination_account_id: None,
                amount: value.to_string(),
                date: input.date.clone(),
                description: format!(
                    "{} · {} · {}",
                    name,
                    input.occurrence,
                    if role == "net" {
                        "Net salary"
                    } else {
                        "Payroll principal"
                    }
                )
                .chars()
                .take(200)
                .collect(),
                currency: input.currency.clone(),
                payee_id: None,
                category_id: None,
                income_source_id: if role == "net" {
                    Some(input.income_id.clone())
                } else {
                    None
                },
                cleared_account_ids: vec![],
            },
        )
        .await?;
        sqlx::query("INSERT INTO salary_payment_transactions VALUES(?,?,?)")
            .bind(&row.id)
            .bind(&id)
            .bind(role)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        if let Some(b) = part {
            let kind: String = sqlx::query_scalar("SELECT type FROM accounts WHERE id=?")
                .bind(&b.values.debt_account_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(err)?;
            if kind == "loan" {
                sqlx::query("INSERT INTO loan_payment_parts(transaction_id,payment_id,contract_id,component,amount,loan_account_id) VALUES(?,?,?,'principal',?,?)")
                    .bind(&row.id).bind(format!("{}:{}",id,b.values.id)).bind(&b.values.contract_id).bind(value).bind(if b.values.contract_id.is_none(){b.values.debt_account_id.as_deref()}else{None}).execute(&mut *tx).await.map_err(err)?;
            }
        }
    }
    for b in &breakdown {
        if let Some(account) = &b.values.debt_account_id {
            let negative: bool = sqlx::query_scalar(
                "SELECT type='loan' AND current_balance<0 FROM accounts WHERE id=?",
            )
            .bind(account)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
            if negative {
                return Err("Principal payment exceeds this loan's balance.".into());
            }
        }
    }
    crate::loans::validate_allocations(&mut tx).await?;
    tx.commit().await.map_err(err)
}
pub(crate) async fn group(conn: &mut SqliteConnection, id: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT payment_id FROM salary_payment_transactions WHERE transaction_id=?")
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(err)
}
#[tauri::command]
pub async fn record_salary_payment(
    pool: tauri::State<'_, SqlitePool>,
    input: PaymentInput,
) -> Result<(), String> {
    let _operation = crate::backups::operation()?;
    record(pool.inner(), input).await
}
#[tauri::command]
pub async fn list_salary_payments(
    pool: tauri::State<'_, SqlitePool>,
    income_id: Option<String>,
    payment_id: Option<String>,
    account_id: Option<String>,
) -> Result<Vec<Payment>, String> {
    let _operation = crate::backups::operation()?;
    history(pool.inner(), income_id, payment_id, account_id).await
}
async fn history(
    pool: &SqlitePool,
    income_id: Option<String>,
    payment_id: Option<String>,
    account_id: Option<String>,
) -> Result<Vec<Payment>, String> {
    sqlx::query_as("SELECT id,income_id,occurrence,date,CAST(gross AS TEXT) gross,CAST(net AS TEXT) net,breakdown FROM salary_payments WHERE (? IS NULL OR income_id=?) AND (? IS NULL OR id=?) AND (? IS NULL OR EXISTS(SELECT 1 FROM json_each(breakdown) WHERE json_extract(value,'$.debt_account_id')=?)) ORDER BY date DESC,id")
        .bind(&income_id).bind(&income_id).bind(&payment_id).bind(&payment_id).bind(&account_id).bind(&account_id).fetch_all(pool).await.map_err(err)
}

#[tauri::command]
pub async fn delete_salary_payment(
    pool: tauri::State<'_, SqlitePool>,
    id: String,
    confirmed: bool,
) -> Result<(), String> {
    let _operation = crate::backups::operation()?;
    reverse(pool.inner(), &id, confirmed).await
}
async fn reverse(pool: &SqlitePool, id: &str, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("Confirm reversal of the entire salary payment.".into());
    }
    let transaction: Option<String> = sqlx::query_scalar(
        "SELECT transaction_id FROM salary_payment_transactions WHERE payment_id=? LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    if let Some(transaction) = transaction {
        crate::transactions::remove_confirmed(pool, &transaction, true).await
    } else {
        sqlx::query("DELETE FROM salary_payments WHERE id=? AND NOT EXISTS(SELECT 1 FROM salary_payment_transactions WHERE payment_id=?)").bind(&id).bind(&id).execute(pool).await.map_err(err)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    pub(crate) async fn database() -> SqlitePool {
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
        sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,opening_balance,current_balance) VALUES('bank','Bank','bank',10000,10000),('home','Home loan','loan',2000000,2000000),('student','Education loan','loan',1000000,1000000); INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('salary','Salary','bank','salary',5000000,25); INSERT INTO income_deductions(id,income_id,name,amount,debt_account_id) VALUES('home','salary','Home',920000,'home'),('student','salary','Education',126000,'student'),('tax','salary','Tax',100000,NULL);").execute(&pool).await.unwrap();
        pool
    }
    pub(crate) fn input() -> PaymentInput {
        serde_json::from_value(serde_json::json!({"income_id":"salary","destination_account_id":"bank","occurrence":"2024-01","date":"2024-01-25","gross":"5000000","currency":"THB","confirmed":true,"deductions":[
            {"id":"home","debt_account_id":"home","amount":"920000","interest":"200000","fee":"20000","contract_id":null},
            {"id":"student","debt_account_id":"student","amount":"126000","interest":"0","fee":"0","contract_id":null},
            {"id":"tax","debt_account_id":null,"amount":"100000","interest":"0","fee":"0","contract_id":null}
        ]})).unwrap()
    }
    async fn balances(p: &SqlitePool) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(p)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn payroll_records_net_once_and_principal_only_then_reverses_whole_group() {
        let p = database().await;
        let before = balances(&p).await;
        record(&p, input()).await.unwrap();
        assert_eq!(
            balances(&p).await,
            vec![
                ("bank".into(), 3864000),
                ("home".into(), 1300000),
                ("student".into(), 874000)
            ]
        );
        let transactions: Vec<crate::transactions::Transaction> =
            sqlx::query_as(crate::transactions::SELECT)
                .fetch_all(&p)
                .await
                .unwrap();
        assert_eq!(transactions.len(), 3);
        let saved: String = sqlx::query_scalar("SELECT breakdown FROM salary_payments")
            .fetch_one(&p)
            .await
            .unwrap();
        let parts: Vec<Breakdown> = serde_json::from_str(&saved).unwrap();
        assert_eq!(parts[0].principal, "700000");
        assert_eq!(parts[0].values.interest, "200000");
        sqlx::query("UPDATE income_deductions SET amount=1,name='Changed'")
            .execute(&p)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT breakdown FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            saved
        );
        assert!(record(&p, input())
            .await
            .unwrap_err()
            .contains("already exists"));
        let principal: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='principal' LIMIT 1",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        crate::transactions::remove(&p, &principal).await.unwrap();
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        record(&p, input()).await.unwrap();
    }
    #[tokio::test]
    async fn invalid_or_stale_payroll_rolls_back_all_balances_and_history() {
        let p = database().await;
        let before = balances(&p).await;
        for case in 0..10 {
            let mut value = input();
            match case {
                0 => value.confirmed = false,
                1 => value.gross = "1".into(),
                2 => value.deductions[0].interest = "920001".into(),
                3 => value.deductions[1].amount = "999999999".into(),
                4 => value.deductions[0].debt_account_id = Some("student".into()),
                5 => value.deductions[0].amount = "9223372036854775808".into(),
                6 => value.date = "2024-02-30".into(),
                7 => value.currency = "USD".into(),
                8 => value.deductions[1] = value.deductions[0].clone(),
                _ => value.deductions[0].contract_id = Some("missing".into()),
            }
            assert!(record(&p, value).await.is_err(), "case {case}");
            assert_eq!(balances(&p).await, before);
        }
        sqlx::query("CREATE TRIGGER fail_payroll BEFORE INSERT ON salary_payment_transactions WHEN NEW.role='principal' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&p).await.unwrap();
        assert!(record(&p, input()).await.is_err());
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn existing_receipt_blocks_payroll_and_zero_net_does_not_create_bank_income() {
        let p = database().await;
        sqlx::query("INSERT INTO transactions(id,type,account_id,amount,date,description,income_source_id) VALUES('old','income','bank',1,'2024-01-24','','salary')").execute(&p).await.unwrap();
        assert!(record(&p, input())
            .await
            .unwrap_err()
            .contains("already exists"));
        sqlx::query("DELETE FROM transactions WHERE id='old'")
            .execute(&p)
            .await
            .unwrap();
        let mut value = input();
        value.gross = "1146000".into();
        record(&p, value).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&p)
                .await
                .unwrap(),
            10000
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT net FROM salary_payments")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn contract_principal_and_exact_large_salary_survive_reversal() {
        let p = database().await;
        sqlx::raw_sql("UPDATE accounts SET loan_type='personal_loan' WHERE id='home'; INSERT INTO loan_facilities VALUES('home',3000000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','home','Contract',2000000,2000000,'2023-01-01','bank','bank',0,920000,12,'2024-01-25','existing');").execute(&p).await.unwrap();
        let mut value = input();
        value.gross = "9007199254740993".into();
        value.deductions[0].contract_id = Some("contract".into());
        record(&p, value).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT current_balance FROM accounts WHERE id='bank'")
                .fetch_one(&p)
                .await
                .unwrap(),
            9007199253604993
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
                .fetch_one(&p)
                .await
                .unwrap(),
            1300000
        );
        let id: String = sqlx::query_scalar(
            "SELECT transaction_id FROM salary_payment_transactions WHERE role='net'",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        crate::transactions::remove(&p, &id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
                .fetch_one(&p)
                .await
                .unwrap(),
            2000000
        );
    }

    #[tokio::test]
    async fn interest_only_zero_net_payment_remains_in_loan_history_and_can_be_reversed() {
        let p = database().await;
        let before = balances(&p).await;
        let mut value = input();
        value.gross = "1146000".into();
        value.deductions[0].interest = "920000".into();
        value.deductions[0].fee = "0".into();
        value.deductions[1].interest = "126000".into();
        record(&p, value).await.unwrap();
        assert_eq!(balances(&p).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
                .fetch_one(&p)
                .await
                .unwrap(),
            0
        );
        let saved = history(&p, None, None, Some("home".into())).await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].net, "0");
        assert_eq!(
            history(&p, None, None, Some("unrelated".into()))
                .await
                .unwrap()
                .len(),
            0
        );
        assert!(reverse(&p, &saved[0].id, false).await.is_err());
        reverse(&p, &saved[0].id, true).await.unwrap();
        assert!(history(&p, None, None, None).await.unwrap().is_empty());
    }
}
