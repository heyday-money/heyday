use serde::Serialize;
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection, SqlitePool};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use tauri_plugin_dialog::DialogExt;
use tempfile::TempDir;
use tokio::sync::{RwLock, RwLockReadGuard};

static DATABASE_OPERATIONS: RwLock<()> = RwLock::const_new(());
// Fingerprint of the alpha initial migration, for a clear fresh-start error.
const ALPHA_INITIAL_CHECKSUM: &[u8] = &[0x70, 0xf9, 0x9b, 0x4a, 0x12, 0x21, 0x10, 0xee, 0xba, 0x4b, 0x47, 0xc9, 0x02, 0xac, 0x9c, 0x06, 0x96, 0x0e, 0xfe, 0x54, 0x38, 0x2e, 0x89, 0xed, 0x31, 0xaa, 0x5d, 0x1b, 0xbe, 0x23, 0x6c, 0x03, 0xe3, 0x15, 0x53, 0x4b, 0x88, 0x96, 0x4d, 0xf0, 0xd0, 0xbc, 0x3a, 0x69, 0x96, 0xb5, 0xca, 0xee];
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
const CONFIRMATION: &str = "RESTORE BACKUP";

// Every native database command holds a shared guard for its entire operation.
// Restore waits for existing operations and rejects new ones rather than replaying
// writes from stale forms against a different set of financial records.
pub fn operation() -> Result<RwLockReadGuard<'static, ()>, String> {
    DATABASE_OPERATIONS.try_read().map_err(|_| {
        "Database restore is in progress. Wait for the app to reload before trying again.".into()
    })
}
#[derive(Debug, Clone, Serialize)]
pub struct BackupPreview {
    token: String,
    filename: String,
    schema_version: i64,
    accounts: i64,
    transactions: i64,
    currency: Option<String>,
}
#[derive(Debug)]
struct PreparedBackup {
    directory: TempDir,
    preview: BackupPreview,
}
#[derive(Default)]
pub struct BackupState {
    pending: Mutex<Option<PreparedBackup>>,
}
#[derive(Serialize)]
pub struct RestoreResult {
    recovery_path: String,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
async fn connect(path: &Path, read_only: bool) -> Result<SqliteConnection, String> {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(read_only)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(10))
            .pragma("trusted_schema", "OFF"),
    )
    .await
    .map_err(|e| format!("Could not open the backup database: {e}"))
}
async fn integrity(conn: &mut SqliteConnection) -> Result<(), String> {
    let result: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| "The selected file is corrupt or is not a SQLite backup.".to_string())?;
    if result != ["ok"] {
        return Err(
            "The backup failed its integrity check. Current data has not been changed.".into(),
        );
    }
    if !sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(conn)
        .await
        .map_err(err)?
        .is_empty()
    {
        return Err("The backup contains broken record references.".into());
    }
    Ok(())
}
async fn schema(conn: &mut SqliteConnection) -> Result<Vec<(String, String, String)>, String> {
    sqlx::query_as("SELECT type,name,sql FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' AND sql IS NOT NULL ORDER BY type,name").fetch_all(conn).await.map_err(err)
}
async fn validate(conn: &mut SqliteConnection) -> Result<i64, String> {
    integrity(conn).await?;
    let history: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version,checksum,success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *conn)
            .await
            .map_err(|_| {
                "This is not a supported Heyday Money backup (migration history is missing)."
                    .to_string()
            })?;
    if history.is_empty() {
        return Err("This backup has no Heyday Money migration history.".into());
    }
    if history[0].0 == 1 && history[0].1.as_slice() == ALPHA_INITIAL_CHECKSUM {
        return Err("Alpha backups cannot be restored into this fresh beta database. Your original alpha database and backup files remain unchanged.".into());
    }
    let known: Vec<_> = MIGRATOR.iter().collect();
    if history.len() > known.len() || history.last().unwrap().0 > known.last().unwrap().version {
        return Err(
            "This backup was created by a newer app. Update Heyday Money before restoring it."
                .into(),
        );
    }
    for (index, (version, checksum, success)) in history.iter().enumerate() {
        if !success
            || *version != known[index].version
            || checksum.as_slice() != known[index].checksum.as_ref()
        {
            return Err(
                "The backup's migration history is incomplete or incompatible with this app."
                    .into(),
            );
        }
    }
    // Compare the actual schema, not just user-editable migration metadata. This
    // also rejects extra triggers, views and tables before trusting file contents.
    let mut reference = SqliteConnection::connect("sqlite::memory:")
        .await
        .map_err(err)?;
    let prefix = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            known[..history.len()]
                .iter()
                .map(|m| (*m).clone())
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    prefix.run_direct(&mut reference).await.map_err(err)?;
    if schema(conn).await? != schema(&mut reference).await? {
        return Err("The backup schema does not match a supported Heyday Money database.".into());
    }
    let settings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE id=1")
        .fetch_one(&mut *conn)
        .await
        .map_err(err)?;
    if settings != 1 {
        return Err("The backup is missing its application settings.".into());
    }
    Ok(history.last().unwrap().0)
}
async fn vacuum(conn: &mut SqliteConnection, path: &Path) -> Result<(), String> {
    sqlx::query("VACUUM INTO ?")
        .bind(
            path.to_str()
                .ok_or("Choose a file with a supported path.")?,
        )
        .execute(conn)
        .await
        .map_err(|e| format!("Could not create the database snapshot: {e}"))?;
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(err)
}
async fn prepare(source: &Path) -> Result<PreparedBackup, String> {
    if !source.is_file() {
        return Err("Choose an existing Heyday Money backup file.".into());
    }
    let mut original = connect(source, true).await?;
    validate(&mut original).await?;
    let directory = tempfile::tempdir().map_err(err)?;
    let path = directory.path().join("restore.db");
    vacuum(&mut original, &path).await?;
    original.close().await.map_err(err)?;
    let mut staged = connect(&path, false).await?;
    let schema_version = validate(&mut staged).await?;
    MIGRATOR
        .run_direct(&mut staged)
        .await
        .map_err(|e| format!("Could not upgrade this backup: {e}"))?;
    validate(&mut staged).await?;
    let preview = BackupPreview {
        token: sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
            .fetch_one(&mut staged)
            .await
            .map_err(err)?,
        filename: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        schema_version,
        accounts: sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
            .fetch_one(&mut staged)
            .await
            .map_err(err)?,
        transactions: sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&mut staged)
            .await
            .map_err(err)?,
        currency: sqlx::query_scalar("SELECT currency FROM settings WHERE id=1")
            .fetch_one(&mut staged)
            .await
            .map_err(err)?,
    };
    staged.close().await.map_err(err)?;
    Ok(PreparedBackup { directory, preview })
}
fn protect_live_path(pool: &SqlitePool, target: &Path) -> Result<(), String> {
    let options = pool.connect_options();
    let live = options.get_filename();
    let parent = target
        .parent()
        .ok_or("Choose a backup destination folder.")?
        .canonicalize()
        .map_err(err)?;
    let normalized = parent.join(target.file_name().ok_or("Choose a backup filename.")?);
    let live = live.canonicalize().map_err(err)?;
    let resolved = if target.exists() {
        target.canonicalize().map_err(err)?
    } else {
        normalized
    };
    if resolved == live
        || resolved == PathBuf::from(format!("{}-wal", live.display()))
        || resolved == PathBuf::from(format!("{}-shm", live.display()))
        || resolved == PathBuf::from(format!("{}-journal", live.display()))
    {
        return Err(
            "Choose a backup file outside the active database and its journal files.".into(),
        );
    }
    Ok(())
}
async fn export(pool: &SqlitePool, target: &Path) -> Result<(), String> {
    protect_live_path(pool, target)?;
    // Snapshot next to the destination; rename publishes it atomically only after
    // completion. An export failure never truncates an existing backup.
    let stage = tempfile::tempdir_in(target.parent().ok_or("Choose a destination folder.")?)
        .map_err(err)?;
    let snapshot = stage.path().join("export.db");
    let mut conn = pool.acquire().await.map_err(err)?;
    vacuum(&mut conn, &snapshot).await?;
    std::fs::rename(snapshot, target).map_err(|e| format!("Could not save the backup: {e}"))
}
async fn replace(
    pool: &SqlitePool,
    backup: &PreparedBackup,
    recovery_dir: &Path,
) -> Result<PathBuf, String> {
    // Caller holds the exclusive database-operation guard. No app command can
    // write between the recovery snapshot and the atomic replacement.
    std::fs::create_dir_all(recovery_dir)
        .map_err(|e| format!("Could not create the recovery folder. Restore stopped: {e}"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let recovery = recovery_dir.join(format!("before-restore-{stamp}.db"));
    let mut live = pool.acquire().await.map_err(err)?;
    vacuum(&mut live, &recovery).await?;
    drop(live);
    // Use a dedicated connection: pooled connections keep foreign keys enabled.
    // Disable triggers only transactionally while copying exact validated data;
    // restoring history must not replay ledger or statement-review side effects.
    let mut conn = SqliteConnection::connect_with(
        &pool.connect_options().as_ref().clone().foreign_keys(false),
    )
    .await
    .map_err(err)?;
    sqlx::query("ATTACH DATABASE ? AS backup")
        .bind(
            backup
                .directory
                .path()
                .join("restore.db")
                .to_str()
                .ok_or("Invalid backup path.")?,
        )
        .execute(&mut conn)
        .await
        .map_err(err)?;
    let result: Result<(), String> = async {
        let mut tx = conn.begin().await.map_err(err)?;
        let triggers: Vec<(String,String)> = sqlx::query_as("SELECT name,sql FROM main.sqlite_schema WHERE type='trigger' ORDER BY name").fetch_all(&mut *tx).await.map_err(err)?;
        let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM main.sqlite_schema WHERE type='table' AND name NOT GLOB 'sqlite_*' AND name<>'_sqlx_migrations' ORDER BY name").fetch_all(&mut *tx).await.map_err(err)?;
        for (name,_) in &triggers { sqlx::query(&format!("DROP TRIGGER {}",quote(name))).execute(&mut *tx).await.map_err(err)?; }
        for name in &tables { sqlx::query(&format!("DELETE FROM main.{}",quote(name))).execute(&mut *tx).await.map_err(err)?; }
        for name in &tables { sqlx::query(&format!("INSERT INTO main.{0} SELECT * FROM backup.{0}",quote(name))).execute(&mut *tx).await.map_err(err)?; }
        sqlx::query("DELETE FROM main.sqlite_sequence").execute(&mut *tx).await.map_err(err)?;
        sqlx::query("INSERT INTO main.sqlite_sequence SELECT * FROM backup.sqlite_sequence").execute(&mut *tx).await.map_err(err)?;
        for (_,sql) in &triggers { sqlx::query(sql).execute(&mut *tx).await.map_err(err)?; }
        if !sqlx::query("PRAGMA main.foreign_key_check").fetch_all(&mut *tx).await.map_err(err)?.is_empty() { return Err("Restored records have invalid references. Restore rolled back.".into()); }
        tx.commit().await.map_err(err)
    }.await;
    // Explicit close also completes any queued transaction rollback before the
    // maintenance guard is released, including error/cancellation paths.
    let _ = conn.close().await;
    result.map_err(|e| format!("Could not restore the backup. Original data remains available; recovery snapshot: {}. {e}",recovery.display()))?;
    Ok(recovery)
}
#[tauri::command]
pub async fn export_backup(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Option<String>, String> {
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Export Heyday Money backup")
            .add_filter("Heyday Money backup", &["db"])
            .set_file_name("heyday-money-backup.db")
            .blocking_save_file()
    })
    .await
    .map_err(err)?;
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = path.into_path().map_err(err)?;
    let _guard = operation()?;
    export(pool.inner(), &path).await?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
#[tauri::command]
pub async fn preview_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, BackupState>,
) -> Result<Option<BackupPreview>, String> {
    state.pending.lock().map_err(err)?.take();
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Choose Heyday Money backup")
            .add_filter("SQLite backup", &["db", "sqlite", "sqlite3"])
            .blocking_pick_file()
    })
    .await
    .map_err(err)?;
    let Some(path) = selected else {
        return Ok(None);
    };
    let prepared = prepare(&path.into_path().map_err(err)?).await?;
    let preview = prepared.preview.clone();
    *state.pending.lock().map_err(err)? = Some(prepared);
    Ok(Some(preview))
}
#[tauri::command]
pub fn cancel_backup_restore(state: tauri::State<'_, BackupState>) -> Result<(), String> {
    state.pending.lock().map_err(err)?.take();
    Ok(())
}
#[tauri::command]
pub async fn restore_backup(
    pool: tauri::State<'_, SqlitePool>,
    state: tauri::State<'_, BackupState>,
    token: String,
    confirmation: String,
) -> Result<RestoreResult, String> {
    if confirmation != CONFIRMATION {
        return Err("Type RESTORE BACKUP to confirm replacement of your current data.".into());
    }
    let prepared = {
        let mut pending = state.pending.lock().map_err(err)?;
        if pending.as_ref().is_none_or(|p| p.preview.token != token) {
            return Err("The backup selection expired. Choose the file again.".into());
        }
        pending.take().unwrap()
    };
    let _guard = DATABASE_OPERATIONS.write().await;
    // Never trust a path supplied by the webview: restore only our staged file.
    let mut checked = connect(&prepared.directory.path().join("restore.db"), true).await?;
    validate(&mut checked).await?;
    checked.close().await.map_err(err)?;
    let directory = pool
        .connect_options()
        .get_filename()
        .parent()
        .ok_or("Database folder unavailable.")?
        .join("backups");
    let recovery = match replace(pool.inner(), &prepared, &directory).await {
        Ok(path) => path,
        Err(error) => {
            *state.pending.lock().map_err(err)? = Some(prepared);
            return Err(error);
        }
    };
    Ok(RestoreResult {
        recovery_path: recovery.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, Row};
    async fn database(path: &Path) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        pool
    }
    async fn populate(pool: &SqlitePool) {
        sqlx::raw_sql("UPDATE settings SET currency='THB',period_start_day=28;
        INSERT INTO logo_assets VALUES('logo','hash','image/png',1,1,X'89504E47','2026-01-01');
        INSERT INTO institutions(id,name,name_key,logo_mode,logo_asset_id) VALUES('custom','Custom Bank','custom bank','custom','logo');
        INSERT INTO accounts(id,name,type,opening_balance,current_balance,institution_id,is_archived,loan_type) VALUES('bank','Bank','bank',9007199254740993,9007199254740993,'custom',0,NULL),('card','Card','credit_card',100000,70000,NULL,0,NULL),('loan','Paid loan','loan',0,0,NULL,1,'personal_loan');
        INSERT INTO loan_payoffs VALUES('loan','2026-01-01');
        INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('payee','Shop','shop','logo');
        INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('income','Salary','bank','salary',500000,28);
        INSERT INTO income_deductions(id,income_id,name,amount) VALUES('deduction','income','Tax',10000);
        INSERT INTO transactions(id,type,account_id,destination_account_id,amount,date,description,income_source_id) VALUES('receipt','income','bank',NULL,490000,'2026-01-28','Salary','income'),('payment','repayment','bank','card',30000,'2026-02-13','Card payment',NULL);
        INSERT INTO transactions(id,type,account_id,amount,date,description,payee_id,category_id) VALUES('expense','expense','card',10000,'2026-01-10','Purchase','payee','planner-expense-0');
        INSERT INTO installments(id,name,account_id,debt_account_id,monthly_amount,installment_count,first_due_date,purchase_transaction_id) VALUES('installment','Phone','bank','card',1000,6,'2026-01-28','expense');
        INSERT INTO subscriptions(id,name,account_id,amount,frequency,first_billing_date,logo_asset_id,managed_via,management_url) VALUES('subscription','Music','bank',1000,'monthly','2026-01-01','logo','apple_app_store','https://example.com/subscriptions');
        INSERT INTO payment_plans(id,name,type,account_id,amount,date) VALUES('plan','Rent','expense','bank',100000,'2099-01-01');
        INSERT INTO planner_items(id,category_id,name,schedule_amount,schedule_start,schedule_end) VALUES('manual','expenses','Cash expense',100,'2026-01','2026-12');
        INSERT INTO planner_amounts VALUES('manual','2026-02',0);
        INSERT INTO planner_income_amounts VALUES('income','2026-02',0);
        INSERT INTO planner_deduction_amounts VALUES('deduction','2026-02',20000);
        INSERT INTO planner_installment_amounts VALUES('installment','2026-02',0);
        INSERT INTO planner_debt_amounts VALUES('loan','2026-01',100);
        INSERT INTO planner_months VALUES('2026-01','complete');
        INSERT INTO planner_opening VALUES(1,'2026-01',-100);
        INSERT INTO card_statements VALUES('statement','card','2026-01-01','2026-01-23','2026-02-13',100000,10000,0);
        INSERT INTO card_statement_entries VALUES('statement','expense');
        INSERT INTO card_statement_installments VALUES('statement','installment','2026-01-28');
        INSERT INTO card_payment_plans VALUES('statement','bank','2026-02-13','full',100000);
        INSERT INTO card_payment_allocations VALUES('payment','statement');
        INSERT INTO card_limit_groups VALUES('group','Shared','shared',500000);
        INSERT INTO card_limit_members VALUES('card','group');
        INSERT INTO reconciliations(id,account_id,confirmed_balance,opening_balance) VALUES(30,'bank',9007199254740993,9007199254740993);
        UPDATE transaction_verifications SET status='reconciled',reconciliation_id=30 WHERE transaction_id='receipt' AND account_id='bank';
        INSERT INTO reconciliation_entries VALUES(30,'receipt','2026-01-28','Salary','income',490000);
        INSERT INTO loan_facilities VALUES('loan',1000000);
        INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','loan','Historic contract',10000,0,'2025-01-01','bank','bank',7500,1000,10,'2025-02-01','existing');").execute(pool).await.unwrap();
    }
    // Compare every data table, including exact integers, blob bytes, timestamps,
    // explicit zeros, archived accounts and foreign-key identities.
    async fn contents(pool: &SqlitePool) -> Vec<(String, Vec<String>)> {
        let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_schema WHERE type='table' AND (name NOT GLOB 'sqlite_*' OR name='sqlite_sequence') AND name<>'_sqlx_migrations' ORDER BY name").fetch_all(pool).await.unwrap();
        let mut result = Vec::new();
        for table in tables {
            let columns = sqlx::query(&format!("PRAGMA table_info({})", quote(&table)))
                .fetch_all(pool)
                .await
                .unwrap();
            let values = columns
                .iter()
                .map(|c| format!("quote({})", quote(c.get::<String, _>("name").as_str())))
                .collect::<Vec<_>>()
                .join(" || ',' || ");
            let mut rows: Vec<String> =
                sqlx::query_scalar(&format!("SELECT {values} FROM {}", quote(&table)))
                    .fetch_all(pool)
                    .await
                    .unwrap();
            rows.sort();
            result.push((table, rows));
        }
        result
    }
    #[tokio::test]
    async fn round_trip_preserves_all_tables_and_recovery_restores_previous_data() {
        let directory = tempfile::tempdir().unwrap();
        let source = database(&directory.path().join("source.db")).await;
        populate(&source).await;
        let expected = contents(&source).await;
        let file = directory.path().join("export.db");
        export(&source, &file).await.unwrap();
        let prepared = prepare(&file).await.unwrap();
        assert_eq!(prepared.preview.accounts, 3);
        assert_eq!(prepared.preview.transactions, 3);
        assert_eq!(prepared.preview.currency.as_deref(), Some("THB"));
        let target = database(&directory.path().join("target.db")).await;
        sqlx::query("UPDATE settings SET currency='USD',period_start_day=1")
            .execute(&target)
            .await
            .unwrap();
        let previous = contents(&target).await;
        let recovery = replace(&target, &prepared, &directory.path().join("backups"))
            .await
            .unwrap();
        assert_eq!(contents(&target).await, expected);
        let mut connection = target.acquire().await.unwrap();
        validate(&mut connection).await.unwrap();
        drop(connection);
        let recovered = prepare(&recovery).await.unwrap();
        replace(&target, &recovered, &directory.path().join("backups"))
            .await
            .unwrap();
        assert_eq!(contents(&target).await, previous);
        // Cached pooled connections continue to work after restore.
        sqlx::query("UPDATE settings SET period_start_day=25")
            .execute(&target)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT period_start_day FROM settings")
                .fetch_one(&target)
                .await
                .unwrap(),
            25
        );
    }
    #[tokio::test]
    async fn corrupt_foreign_newer_and_tampered_backups_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.db");
        std::fs::write(&file, b"not sqlite").unwrap();
        assert!(prepare(&file).await.is_err());
        let foreign = dir.path().join("foreign.db");
        let mut conn = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&foreign)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE something(id INTEGER)")
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        assert!(prepare(&foreign)
            .await
            .unwrap_err()
            .contains("not a supported"));
        let path = dir.path().join("valid.db");
        let pool = database(&path).await;
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(999,'new',1,X'00',0)").execute(&pool).await.unwrap();
        assert!(prepare(&path).await.unwrap_err().contains("newer app"));
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version=999")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum=X'00' WHERE version=1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(prepare(&path).await.unwrap_err().contains("incompatible"));
        sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=1")
            .bind(MIGRATOR.iter().next().unwrap().checksum.as_ref())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER unexpected AFTER UPDATE ON settings BEGIN SELECT 1; END")
            .execute(&pool)
            .await
            .unwrap();
        assert!(prepare(&path).await.unwrap_err().contains("schema"));
        sqlx::query("DROP TRIGGER unexpected")
            .execute(&pool)
            .await
            .unwrap();
        let mut invalid = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .foreign_keys(false),
        )
        .await
        .unwrap();
        sqlx::query("INSERT INTO incomes(id,name,destination_account_id,type,estimated_amount,recurrence_day_of_month) VALUES('bad','Broken','missing','salary',100,1)").execute(&mut invalid).await.unwrap();
        invalid.close().await.unwrap();
        assert!(prepare(&path)
            .await
            .unwrap_err()
            .contains("broken record references"));
    }
    #[tokio::test]
    async fn alpha_backup_is_rejected_without_changing_source_or_beta_data() {
        let alpha = sqlx::migrate!("./tests/fixtures/alpha-migrations");
        assert_eq!(alpha.iter().next().unwrap().checksum.as_ref(), ALPHA_INITIAL_CHECKSUM);
        for count in [1, 6, 40] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("alpha.db");
            let mut conn = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path).create_if_missing(true)).await.unwrap();
            let prefix = sqlx::migrate::Migrator {
                migrations: Cow::Owned(alpha.iter().take(count).cloned().collect()),
                ..sqlx::migrate::Migrator::DEFAULT
            };
            prefix.run_direct(&mut conn).await.unwrap();
            sqlx::query("INSERT INTO accounts(id,name,type,opening_balance) VALUES('old','Old bank','bank',9007199254740993)").execute(&mut conn).await.unwrap();
            conn.close().await.unwrap();
            let before = std::fs::read(&path).unwrap();
            let target = database(&dir.path().join("beta.db")).await;
            populate(&target).await;
            let target_before = contents(&target).await;
            assert!(prepare(&path).await.unwrap_err().contains("Alpha backups cannot be restored"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(contents(&target).await, target_before);
        }
    }
    #[tokio::test]
    async fn failed_export_and_restore_keep_existing_files_and_live_data_usable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.db");
        let pool = database(&path).await;
        populate(&pool).await;
        let expected = contents(&pool).await;
        assert!(export(&pool, &path).await.is_err());
        for suffix in ["-wal", "-shm", "-journal"] {
            assert!(export(&pool, &PathBuf::from(format!("{}{suffix}", path.display()))).await.is_err());
        }
        assert!(export(&pool, &dir.path().join("missing/out.db"))
            .await
            .is_err());
        let output = dir.path().join("directory.db");
        std::fs::create_dir(&output).unwrap();
        assert!(export(&pool, &output).await.is_err());
        assert!(output.is_dir());
        let file = dir.path().join("export.db");
        export(&pool, &file).await.unwrap();
        let prepared = prepare(&file).await.unwrap();
        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        assert!(replace(&pool, &prepared, &blocked).await.is_err());
        assert_eq!(contents(&pool).await, expected);
        // A write error after deleting existing rows must roll the entire restore back.
        sqlx::query("CREATE TABLE extra(id TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO extra VALUES('keep')")
            .execute(&pool)
            .await
            .unwrap();
        let before = contents(&pool).await;
        assert!(replace(&pool, &prepared, &dir.path().join("backups"))
            .await
            .is_err());
        assert_eq!(contents(&pool).await, before);
        let mut connection = pool.acquire().await.unwrap();
        integrity(&mut connection).await.unwrap();
        drop(connection);
        sqlx::query("UPDATE settings SET period_start_day=29")
            .execute(&pool)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn cancelling_preparation_removes_staged_copy_without_changing_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.db");
        let pool = database(&path).await;
        let prepared = prepare(&path).await.unwrap();
        let stage = prepared.directory.path().to_path_buf();
        let state = BackupState {
            pending: Mutex::new(Some(prepared)),
        };
        state.pending.lock().unwrap().take();
        assert!(!stage.exists());
        assert!(path.exists());
        let mut conn = pool.acquire().await.unwrap();
        validate(&mut conn).await.unwrap();
    }
    #[tokio::test]
    async fn failed_snapshot_does_not_overwrite_existing_export() {
        let dir = tempfile::tempdir().unwrap();
        let pool = database(&dir.path().join("live.db")).await;
        let file = dir.path().join("existing.db");
        std::fs::write(&file, b"keep existing export").unwrap();
        pool.close().await;
        assert!(export(&pool, &file).await.is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"keep existing export");
    }
    #[tokio::test]
    async fn restore_gate_waits_for_inflight_operations_and_rejects_new_commands() {
        let read = operation().unwrap();
        assert!(DATABASE_OPERATIONS.try_write().is_err());
        drop(read);
        let exclusive = DATABASE_OPERATIONS.write().await;
        assert!(operation().is_err());
        drop(exclusive);
        assert!(operation().is_ok());
    }
}
