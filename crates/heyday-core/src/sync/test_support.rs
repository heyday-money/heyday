use crate::salary_payments::PaymentInput;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
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
    crate::database::MIGRATOR.run(&pool).await.unwrap();
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
