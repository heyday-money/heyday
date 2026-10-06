use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Serialize, sqlx::FromRow)]
pub struct Institution {
    pub id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub bank_code: Option<String>,
    pub swift_code: Option<String>,
    pub logo: Option<String>,
    #[sqlx(default)]
    pub logo_asset_id: Option<String>,
    #[sqlx(default)]
    pub logo_mode: String,
    pub is_archived: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct AccountIdentity {
    id: String,
    name: String,
    institution_id: Option<String>,
}
#[derive(Serialize)]
pub struct InstitutionDirectory {
    institutions: Vec<Institution>,
    accounts: Vec<AccountIdentity>,
}
#[derive(Deserialize)]
pub struct SaveInstitution {
    pub logo_change: Option<crate::logos::LogoChange>,
    pub id: Option<String>,
    pub name: String,
    pub is_archived: bool,
}
fn normalize(name: &str) -> Result<(String, String), String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter an institution name between 1 and 100 characters.".into());
    }
    let key = name.to_lowercase();
    Ok((name, key))
}
fn save_error(error: sqlx::Error) -> String {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        "That institution already exists, including archived institutions. Choose or restore it."
            .into()
    } else {
        error.to_string()
    }
}
pub async fn save(pool: &SqlitePool, input: SaveInstitution) -> Result<Institution, String> {
    let (name, key) = normalize(&input.name)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let mut saved = if let Some(id) = input.id {
        sqlx::query_as::<_, Institution>("UPDATE institutions SET name=?,name_key=?,is_archived=? WHERE id=? RETURNING *")
            .bind(name).bind(key).bind(input.is_archived).bind(id).fetch_optional(&mut *tx).await
    } else {
        sqlx::query_as::<_, Institution>("INSERT INTO institutions(id,name,name_key,is_archived) VALUES(lower(hex(randomblob(16))),?,?,?) RETURNING *")
            .bind(name).bind(key).bind(input.is_archived).fetch_optional(&mut *tx).await
    }.map_err(save_error)?.ok_or("This institution no longer exists. Reload Settings.")?;
    if let Some(change) = input.logo_change {
        let id = crate::logos::asset(&mut tx, &change).await?;
        let mode = match change {
            crate::logos::LogoChange::Custom { .. } => "custom",
            crate::logos::LogoChange::Default => "default",
            crate::logos::LogoChange::None => "none",
        };
        sqlx::query("UPDATE institutions SET logo_asset_id=?,logo_mode=? WHERE id=?")
            .bind(&id)
            .bind(mode)
            .bind(&saved.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        saved.logo_asset_id = id;
        saved.logo_mode = mode.into();
        crate::logos::prune(&mut tx).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(saved)
}

// ID is authoritative. Free text is accepted for older clients and imported records.
// Resolution happens inside the account transaction, so failed saves create no institutions.
pub async fn resolve(
    conn: &mut SqliteConnection,
    kind: &str,
    id: Option<&str>,
    name: Option<&str>,
    existing_id: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
    if kind == "cash" {
        if id.is_some() || name.is_some_and(|n| !n.trim().is_empty()) {
            return Err("Cash accounts do not have an institution.".into());
        }
        return Ok((None, None));
    }
    let institution = if let Some(id) = id {
        sqlx::query_as::<_, Institution>("SELECT * FROM institutions WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Choose an available institution.")?
    } else if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
        let (name, key) = normalize(name)?;
        if let Some(found) =
            sqlx::query_as::<_, Institution>("SELECT * FROM institutions WHERE name_key=?")
                .bind(&key)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?
        {
            found
        } else {
            sqlx::query_as::<_, Institution>("INSERT INTO institutions(id,name,name_key) VALUES(lower(hex(randomblob(16))),?,?) RETURNING *")
                .bind(name).bind(key).fetch_one(&mut *conn).await.map_err(save_error)?
        }
    } else {
        return Ok((None, None));
    };
    if institution.is_archived && existing_id != Some(institution.id.as_str()) {
        return Err("Choose an active institution or restore it in Settings.".into());
    }
    Ok((Some(institution.id), Some(institution.name)))
}

#[tauri::command]
pub async fn list_institutions(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<InstitutionDirectory, String> {
    let _database_operation = crate::backups::operation()?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let institutions =
        sqlx::query_as("SELECT * FROM institutions ORDER BY is_archived,name COLLATE NOCASE,id")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let accounts = sqlx::query_as("SELECT id,name,institution_id FROM accounts")
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(InstitutionDirectory {
        institutions,
        accounts,
    })
}
#[tauri::command]
pub async fn delete_institution(pool: tauri::State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let _database_operation = crate::backups::operation()?;
    delete(pool.inner(), &id).await
}

async fn delete(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Transactions reference source/destination accounts, not institutions directly.
    // Blocking every account (including archived ones) protects both history sides.
    let removed = sqlx::query("DELETE FROM institutions WHERE id=? AND NOT EXISTS(SELECT 1 FROM accounts WHERE institution_id=institutions.id)")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    if removed.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM institutions WHERE id=?)")
            .bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        return Err(if exists {
            "This institution has linked accounts, including possibly archived accounts or transaction history. Archive it instead."
        } else { "This institution no longer exists. Reload Settings." }.into());
    }
    crate::logos::prune(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_institution(
    pool: tauri::State<'_, SqlitePool>,
    input: SaveInstitution,
) -> Result<Institution, String> {
    let _database_operation = crate::backups::operation()?;
    save(pool.inner(), input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    async fn database() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await
            .unwrap()
    }
    fn account(kind: &str, id: Option<&str>, name: Option<&str>) -> crate::accounts::NewAccount {
        serde_json::from_value(serde_json::json!({"name":"My account", "type":kind,"loan_type":if kind=="loan" {Some("personal_loan")} else {None},"opening_balance":"9007199254740993","currency":"THB","institution_id":id,"institution":name})).unwrap()
    }
    #[tokio::test]
    async fn deletion_protects_accounts_history_and_is_atomic() {
        let pool = database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,institution_id,is_archived) VALUES('a','Archived','bank','bank-002',1),('b','Active','bank','bank-004',0);
        INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description) VALUES('t','transfer','a','b',125,'2026-01-01','');").execute(&pool).await.unwrap();
        for id in ["bank-002", "bank-004"] { assert!(delete(&pool, id).await.unwrap_err().contains("linked accounts")); }
        let history: i64 = sqlx::query_scalar("SELECT amount FROM transactions WHERE id='t'").fetch_one(&pool).await.unwrap();
        assert_eq!(history, 125);
        assert!(delete(&pool, "missing").await.unwrap_err().contains("no longer exists"));
        sqlx::raw_sql("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES('unused','one','image/png',1,1,X'01'),('shared','two','image/png',1,1,X'02');
        UPDATE institutions SET logo_asset_id='unused',logo_mode='custom',is_archived=1 WHERE id='bank-006';
        UPDATE institutions SET logo_asset_id='shared',logo_mode='custom' WHERE id='bank-014';
        INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('p','Shop','shop','shared');
        CREATE TRIGGER fail_prune BEFORE DELETE ON logo_assets BEGIN SELECT RAISE(ABORT,'test failure'); END;").execute(&pool).await.unwrap();
        assert!(delete(&pool, "bank-006").await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM institutions WHERE id='bank-006'").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        sqlx::query("DROP TRIGGER fail_prune").execute(&pool).await.unwrap();
        delete(&pool, "bank-006").await.unwrap();
        delete(&pool, "bank-014").await.unwrap();
        let assets: Vec<String> = sqlx::query_scalar("SELECT id FROM logo_assets").fetch_all(&pool).await.unwrap();
        assert_eq!(assets, vec!["shared"]);
        let broken: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(&pool).await.unwrap();
        assert_eq!(broken, 0);
    }
    #[tokio::test]
    async fn migrate_free_text_links_and_preserve_history() {
        let pool = database().await;
        for migration in sqlx::migrate!("./tests/fixtures/alpha-migrations")
            .iter()
            .filter(|m| m.version < 31)
        {
            sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
        }
        sqlx::raw_sql("INSERT INTO accounts(id,name,type,institution,opening_balance,current_balance,is_archived) VALUES ('bank','Savings','bank',' KBANK ',9007199254740993,9007199254740990,1),('custom','Other','wallet',' My  Provider ',100,99,0),('same','Second','investment','my provider',25,25,0); INSERT INTO transactions(id,type,account_id,amount,date,description) VALUES('t','expense','bank',3,'2026-01-01','');").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../tests/fixtures/alpha-migrations/0031_institutions.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let row: (String,String,i64,i64,i64) = sqlx::query_as("SELECT institution_id,institution,opening_balance,current_balance,is_archived FROM accounts WHERE id='bank'").fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "bank-004".into(),
                "Kasikornbank Public Company Limited".into(),
                9007199254740993,
                9007199254740990,
                1
            )
        );
        let custom: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT institution_id) FROM accounts WHERE id IN ('custom','same')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(custom, 1);
        save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: Some("bank-004".into()),
                name: "Renamed bank".into(),
                is_archived: false,
            },
        )
        .await
        .unwrap();
        let history: (String,i64,String) = sqlx::query_as("SELECT a.institution,t.amount,a.name FROM transactions t JOIN accounts a ON a.id=t.account_id").fetch_one(&pool).await.unwrap();
        assert_eq!(history, ("Renamed bank".into(), 3, "Savings".into()));
        let violations: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(violations, 0);
    }
    #[tokio::test]
    async fn stable_ids_validation_archive_and_atomic_creation() {
        let pool = database().await;
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        crate::save_currency(&pool, "THB").await.unwrap();
        for kind in ["bank", "wallet", "credit_card", "loan", "investment"] {
            crate::accounts::insert_account(
                &pool,
                account(kind, Some("bank-002"), Some("stale display name")),
            )
            .await
            .unwrap();
        }
        assert!(
            crate::accounts::insert_account(&pool, account("cash", Some("bank-002"), None))
                .await
                .is_err()
        );
        assert!(
            crate::accounts::insert_account(&pool, account("bank", Some("missing"), None))
                .await
                .is_err()
        );
        let renamed = save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: Some("bank-002".into()),
                name: "  My   BANK ".into(),
                is_archived: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(renamed.name, "My BANK");
        assert_eq!(renamed.logo.as_deref(), Some("bbl.svg"));
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: "my bank".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: " ".into(),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(save(
            &pool,
            SaveInstitution {
                logo_change: None,
                id: None,
                name: "a".repeat(101),
                is_archived: false
            }
        )
        .await
        .is_err());
        assert!(
            crate::accounts::insert_account(&pool, account("bank", Some("bank-002"), None))
                .await
                .is_err()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM accounts WHERE institution='My BANK'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 5);
        // Existing links to archived institutions remain valid when editing other details.
        let mut tx = pool.begin().await.unwrap();
        assert!(
            resolve(&mut tx, "bank", Some("bank-002"), None, Some("bank-002"))
                .await
                .is_ok()
        );
        tx.rollback().await.unwrap();
        // Failure after institution resolution rolls back both new records.
        sqlx::query("CREATE TRIGGER fail_account BEFORE INSERT ON accounts BEGIN SELECT RAISE(ABORT,'test'); END").execute(&pool).await.unwrap();
        assert!(crate::accounts::insert_account(
            &pool,
            account("bank", None, Some("New custom provider"))
        )
        .await
        .is_err());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM institutions WHERE name='New custom provider'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        sqlx::query("DROP TRIGGER fail_account")
            .execute(&pool)
            .await
            .unwrap();
        crate::accounts::insert_account(
            &pool,
            account("wallet", None, Some("New custom provider")),
        )
        .await
        .unwrap();
        let logo: Option<String> =
            sqlx::query_scalar("SELECT logo FROM institutions WHERE name='New custom provider'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(logo, None);
    }
}
