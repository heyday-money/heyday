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
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub institution_id: Option<String>,
    pub logo_asset_id: Option<String>,
}
#[derive(Serialize)]
pub struct InstitutionDirectory {
    pub institutions: Vec<Institution>,
    pub accounts: Vec<AccountIdentity>,
}
#[derive(Deserialize)]
pub struct SaveInstitution {
    pub logo_change: Option<crate::logos::LogoChange>,
    pub id: Option<String>,
    pub name: String,
    pub is_archived: bool,
}
pub fn normalize(name: &str) -> Result<(String, String), String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Enter an institution name between 1 and 100 characters.".into());
    }
    let key = name.to_lowercase();
    Ok((name, key))
}
pub fn save_error(error: sqlx::Error) -> String {
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

pub async fn list_institutions(pool: &SqlitePool) -> Result<InstitutionDirectory, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let institutions =
        sqlx::query_as("SELECT * FROM institutions ORDER BY is_archived,name COLLATE NOCASE,id")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let accounts = sqlx::query_as("SELECT id,name,type AS kind,institution_id,(SELECT logo_asset_id FROM account_logos WHERE account_id=accounts.id) AS logo_asset_id FROM accounts")
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(InstitutionDirectory {
        institutions,
        accounts,
    })
}
pub async fn delete_institution(pool: &SqlitePool, id: String) -> Result<(), String> {
    delete(pool, &id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Transactions reference source/destination accounts, not institutions directly.
    // Blocking every account (including archived ones) protects both history sides.
    let removed = sqlx::query("DELETE FROM institutions WHERE id=? AND NOT EXISTS(SELECT 1 FROM accounts WHERE institution_id=institutions.id)")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    if removed.rows_affected() == 0 {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM institutions WHERE id=?)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        return Err(if exists {
            "This institution has linked accounts, including possibly archived accounts or transaction history. Archive it instead."
        } else { "This institution no longer exists. Reload Settings." }.into());
    }
    crate::logos::prune(&mut tx).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn save_institution(
    pool: &SqlitePool,
    input: SaveInstitution,
) -> Result<Institution, String> {
    save(pool, input).await
}
