//! Cross-domain tests use migrated SQLite databases and real encryption/queues.
use crate::{
    cloud_sync_ledger as ledger, cloud_sync_metadata as meta,
    cloud_sync_protocol::{self as protocol, Change},
    cloud_sync_store as store,
    transactions::{self, NewTransaction},
};
use serde_json::json;
use sqlx::SqlitePool;
use zeroize::Zeroizing;
const ORIGIN: &str = "https://example.test";
#[tokio::test]
async fn rotated_keys_keep_financial_ids_balances_and_offline_edits_across_three_databases() {
    use crate::{
        cloud_crypto as crypto,
        cloud_key_protocol::{Ring, Rotation},
    };
    async fn deliver_epoch(p: &SqlitePool, ring: &Ring, changes: &[Change]) {
        let start = store::binding(p).await.unwrap().unwrap().head();
        let mut end = start.clone();
        let mut decoded = Vec::new();
        for c in changes {
            end = protocol::verify_change(c, &end).unwrap();
            decoded.push((c.clone(), ring.decode(c).unwrap()));
        }
        store::apply_page(p, &start, &decoded, &end, "test", true)
            .await
            .unwrap();
    }
    async fn drain_epoch(p: &SqlitePool, ring: &Ring) -> Vec<Change> {
        let (identity, _) = ring.identity().unwrap();
        let mut changes = Vec::new();
        loop {
            let bound = store::binding(p).await.unwrap().unwrap();
            let Some(request) =
                store::prepare_epoch(p, &ring.key, &ring.envelope.key_id, &identity, &bound)
                    .await
                    .unwrap()
            else {
                break;
            };
            let c = Change {
                sequence: (request.previous.number().unwrap() + 1).to_string(),
                version: (protocol::number(&request.base_version).unwrap() + 1).to_string(),
                request,
            };
            deliver_epoch(p, ring, &[c.clone()]).await;
            changes.push(c);
        }
        changes
    }
    let root = Zeroizing::new([9; 32]);
    let old_envelope = crypto::wrap("alice", "dataset", &root, &Zeroizing::new([10; 32])).unwrap();
    let old = Ring::open("alice", root, &old_envelope, None).unwrap();
    let a = db(true).await;
    let b = db(true).await;
    let c = db(true).await;
    for p in [&a, &b, &c] {
        sqlx::query("UPDATE cloud_sync_binding SET key_id=?")
            .bind(&old_envelope.key_id)
            .execute(p)
            .await
            .unwrap();
    }
    seed(&a).await;
    deliver_epoch(&a, &old, &[]).await;
    let initial = drain_epoch(&a, &old).await;
    deliver_epoch(&b, &old, &initial).await;
    assert_clean(&a).await;
    assert_clean(&b).await;
    let boundary = store::binding(&a).await.unwrap().unwrap().head();
    // A queued request was never accepted before the other device rotated.
    tx(&a, "expense", "bank", None, "1234", "2024-05-10").await;
    let bound = store::binding(&a).await.unwrap().unwrap();
    let pending = store::prepare_epoch(&a, &old.key, &old.envelope.key_id, &old.key, &bound)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.payload.key_id, old.envelope.key_id);
    let next = Zeroizing::new([11; 32]);
    let envelope = crypto::wrap("alice", "dataset", &next, &Zeroizing::new([12; 32])).unwrap();
    let rotation = Rotation::create("alice", &old, &next, envelope.clone(), &boundary).unwrap();
    let recovered = Ring::open("alice", next, &envelope, Some(&rotation)).unwrap();
    // The network layer calls this only for a definitive KEY_CHANGED rejection.
    store::discard_pending(&a).await.unwrap();
    let queued = store::status(&a, ORIGIN, "alice").await.unwrap().pending;
    assert!(queued > 0);
    let changes = drain_epoch(&a, &recovered).await;
    assert!(changes
        .iter()
        .all(|c| c.request.payload.key_id == envelope.key_id));
    deliver_epoch(&b, &recovered, &changes).await;
    let mut full = initial;
    full.extend(changes);
    deliver_epoch(&c, &recovered, &full).await;
    for p in [&a, &b, &c] {
        assert_clean(p).await;
    }
    let accounts = |p: SqlitePool| async move {
        sqlx::query_as::<_, (String, i64)>("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap()
    };
    assert_eq!(accounts(a.clone()).await, accounts(b.clone()).await);
    assert_eq!(accounts(a.clone()).await, accounts(c.clone()).await);
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions")
        .fetch_one(&a)
        .await
        .unwrap();
    for p in [&b, &c] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions")
                .fetch_one(p)
                .await
                .unwrap(),
            count
        );
    }
    for p in [&a, &b, &c] {
        assert_eq!(
            store::binding(p).await.unwrap().unwrap().key_id,
            old.envelope.key_id
        );
    }
}
async fn db(full: bool) -> SqlitePool {
    let p = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(":memory:")
                .foreign_keys(true),
        )
        .await
        .unwrap();
    crate::database::MIGRATOR.run(&p).await.unwrap();
    connect(&p, full).await;
    p
}
async fn connect(p: &SqlitePool, full: bool) {
    store::enable(p, ORIGIN, "alice", "dataset", "key")
        .await
        .unwrap();
    ledger::enable(p).await.unwrap();
    crate::cloud_sync_income::enable(p).await.unwrap();
    crate::cloud_sync_planner::enable(p).await.unwrap();
    if full {
        meta::enable(p).await.unwrap();
    }
}
async fn deliver(p: &SqlitePool, changes: &[Change]) -> Result<(), String> {
    let head = store::binding(p).await?.unwrap().head();
    let mut end = head.clone();
    let mut records = vec![];
    for c in changes {
        end = protocol::verify_change(c, &end)?;
        records.push((
            c.clone(),
            store::Record::decode(c, &Zeroizing::new([9; 32]), "dataset", "key")?,
        ));
    }
    store::apply_page(p, &head, &records, &end, "test", true).await
}
async fn drain(p: &SqlitePool) -> Vec<Change> {
    let mut out = vec![];
    loop {
        let bound = store::binding(p).await.unwrap().unwrap();
        let Some(m) = store::prepare(p, &Zeroizing::new([9; 32]), &bound)
            .await
            .unwrap()
        else {
            break;
        };
        let c = Change {
            sequence: (m.previous.number().unwrap() + 1).to_string(),
            version: (protocol::number(&m.base_version).unwrap() + 1).to_string(),
            request: m,
        };
        deliver(p, &[c.clone()]).await.unwrap();
        out.push(c);
        assert!(out.len() < 1000);
    }
    out
}
async fn choose(p: &SqlitePool, id: &str, choice: &str) {
    let c = store::status(p, ORIGIN, "alice")
        .await
        .unwrap()
        .conflicts
        .into_iter()
        .find(|c| c.record_id == id)
        .unwrap();
    store::resolve(p, id, c.local_revision, c.remote_version, choice, None)
        .await
        .unwrap();
}
async fn assert_clean(p: &SqlitePool) {
    let s = store::status(p, ORIGIN, "alice").await.unwrap();
    let ids: Vec<String> = sqlx::query_scalar("SELECT record_id FROM cloud_sync_dirty")
        .fetch_all(p)
        .await
        .unwrap();
    let conflicts: Vec<String> =
        sqlx::query_scalar("SELECT record_id FROM cloud_sync_shadow WHERE conflict=1")
            .fetch_all(p)
            .await
            .unwrap();
    assert_eq!(
        (s.pending, s.conflict_count),
        (0, 0),
        "pending: {ids:?}; conflicts: {conflicts:?}"
    );
}
async fn tx(
    p: &SqlitePool,
    kind: &str,
    source: &str,
    dest: Option<&str>,
    amount: &str,
    date: &str,
) -> String {
    let input:NewTransaction=serde_json::from_value(json!({"type":kind,"account_id":source,"destination_account_id":dest,"amount":amount,"date":date,"description":"Test payment","currency":"THB","payee_id":null,"category_id":null,"income_source_id":null,"cleared_account_ids":[]})).unwrap();
    transactions::insert(p, input).await.unwrap().id
}
async fn seed(p: &SqlitePool) -> (String, String, String) {
    sqlx::raw_sql("UPDATE settings SET currency='THB',period_start_day=25; INSERT INTO accounts(id,name,type,loan_type,opening_balance,current_balance,institution,institution_id) VALUES('bank','Bank','bank',NULL,1000000,1000000,'Bangkok Bank Public Company Limited','bank-002'),('loan','Loan','loan','personal_loan',100000,100000,NULL,NULL),('card','Card','credit_card',NULL,0,0,NULL,NULL),('paid','Paid loan','loan','other_debt',0,0,NULL,NULL); UPDATE accounts SET is_archived=1 WHERE id='paid'; INSERT INTO loan_payoffs VALUES('paid','2024-01-01'); INSERT INTO loan_facilities VALUES('loan',500000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','loan','Existing borrowing',100000,100000,'2024-01-01','bank','bank',75000,5000,24,'2024-02-01','existing');").execute(p).await.unwrap();
    let purchase = tx(p, "expense", "card", None, "5000", "2024-01-15").await;
    let payment = tx(p, "repayment", "bank", Some("card"), "1000", "2024-02-15").await;
    let input=serde_json::from_value(json!({"contract_id":"contract","account_id":"bank","date":"2024-03-01","total":"5150","principal":"5000","interest":"100","fee":"50","currency":"THB"})).unwrap();
    crate::loans::repay(p, input).await.unwrap();
    let loan_payment: String =
        sqlx::query_scalar("SELECT payment_id FROM loan_payment_parts LIMIT 1")
            .fetch_one(p)
            .await
            .unwrap();
    sqlx::query("INSERT INTO installments(id,name,account_id,debt_account_id,monthly_amount,installment_count,first_due_date,purchase_kind,purchase_transaction_id) VALUES('installment','Phone','bank','card',2000,3,'2024-01-31','new_purchase',?)").bind(&purchase).execute(p).await.unwrap();
    sqlx::raw_sql("INSERT INTO card_limit_groups VALUES('limit','Shared','shared',9007199254740993); INSERT INTO card_limit_members VALUES('card','limit'); INSERT INTO card_statements VALUES('statement','card','2024-01-01','2024-01-31','2024-02-20',5000,500,0); INSERT INTO card_payment_plans VALUES('statement','bank','2024-02-18','full',5000); INSERT INTO card_statement_installments VALUES('statement','installment','2024-01-31'); INSERT INTO selective_defaults VALUES('loan','2025-01','2025-03'); INSERT INTO payment_plans(id,name,type,account_id,amount,date) VALUES('plan','Rent','expense','bank',80000,'2026-12-01'); INSERT INTO subscriptions(id,name,account_id,amount,frequency,first_billing_date,provider_id,managed_via,management_url) VALUES('subscription','Music','bank',12900,'monthly','2024-01-01','provider-spotify','website','https://example.test/manage'); INSERT INTO planner_items(id,category_id,name,description,schedule_amount,schedule_start,schedule_end) VALUES('manual','expenses','Holiday','Saved plan',10000,'2026-11','2027-01'); INSERT INTO planner_amounts VALUES('manual','2026-11',9007199254740993); INSERT INTO planner_debt_amounts VALUES('loan','2026-11',0); INSERT INTO planner_installment_amounts VALUES('installment','2024-01',123); INSERT INTO planner_months VALUES('2026-11','complete'); INSERT INTO planner_opening VALUES(1,'2026-01',-9007199254740993); INSERT INTO reconciliations(id,account_id,completed_at,confirmed_balance,opening_balance) VALUES(1,'bank','2024-02-15T12:00:00.000Z',999000,1000000);").execute(p).await.unwrap();
    sqlx::query("INSERT INTO card_statement_entries VALUES('statement',?)")
        .bind(&purchase)
        .execute(p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO card_payment_allocations VALUES(?,'statement')")
        .bind(&payment)
        .execute(p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO reconciliation_entries VALUES(1,?,'2024-02-15','Card payment','repayment',-1000)").bind(&payment).execute(p).await.unwrap();
    sqlx::query("UPDATE transaction_verifications SET status='reconciled',reconciliation_id=1 WHERE transaction_id=? AND account_id='bank'").bind(&payment).execute(p).await.unwrap();
    let mut bytes = vec![];
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[240, 10, 10, 255]).unwrap();
    }
    use sha2::Digest;
    let hash = format!("{:x}", sha2::Sha256::digest(&bytes));
    sqlx::query("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES('local-logo',?,'image/png',1,1,?)").bind(hash).bind(bytes).execute(p).await.unwrap();
    sqlx::raw_sql("INSERT INTO payees(id,name,name_key,logo_asset_id) VALUES('payee','Shop','shop','local-logo'); UPDATE institutions SET logo_mode='custom',logo_asset_id='local-logo' WHERE id='bank-002'; UPDATE subscription_providers SET logo_mode='custom',logo_asset_id='local-logo' WHERE id='provider-spotify'; UPDATE subscriptions SET logo_asset_id='local-logo' WHERE id='subscription';").execute(p).await.unwrap();
    sqlx::raw_sql("INSERT INTO accounts(id,name,type) VALUES('cash','Cash','cash'); INSERT INTO account_logos VALUES('cash','local-logo')").execute(p).await.unwrap();
    (purchase, payment, loan_payment)
}
async fn metadata_records(p: &SqlitePool) -> Vec<store::Record> {
    let mut conn = p.acquire().await.unwrap();
    let mut result = vec![];
    for spec in meta::SPECS {
        let keys: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT DISTINCT {} FROM {} WHERE {}",
            spec.keys
                .split(',')
                .map(|k| format!("CAST({k} AS TEXT)"))
                .collect::<Vec<_>>()
                .join("||':'||"),
            spec.table,
            if spec.kind == "account-link" {
                "institution_id IS NOT NULL"
            } else {
                "true"
            }
        ))
        .fetch_all(&mut *conn)
        .await
        .unwrap();
        for k in keys {
            result.push(
                meta::current(&mut conn, &format!("{}:{k}", spec.kind))
                    .await
                    .unwrap(),
            );
        }
    }
    result.sort_by_key(|r| r.key());
    result
}

#[tokio::test]
async fn complete_bootstrap_round_trips_every_metadata_kind_and_derived_balances() {
    let a = db(true).await;
    seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let changes = drain(&a).await;
    assert_clean(&a).await;
    let b = db(true).await;
    deliver(&b, &changes).await.unwrap();
    assert_clean(&b).await;
    let actual = metadata_records(&b).await;
    let expected = metadata_records(&a).await;
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .iter()
            .filter_map(|r| if let Some(store::Value::Metadata(v)) = &r.value {
                Some(v.kind.clone())
            } else {
                None
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        meta::SPECS.len()
    );
    let balances = |p: SqlitePool| async move {
        sqlx::query_as::<_, (String, i64)>("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap()
    };
    assert_eq!(balances(a.clone()).await, balances(b.clone()).await);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT remaining_principal FROM loan_contracts WHERE id='contract'"
        )
        .fetch_one(&b)
        .await
        .unwrap(),
        95000
    );
}
#[tokio::test]
async fn ledger_only_upgrade_adds_contracts_without_double_posting_and_can_reverse() {
    let a = db(false).await;
    let (_, _, payment) = seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let old = drain(&a).await;
    let b = db(false).await;
    deliver(&b, &old).await.unwrap();
    let before: Vec<(String, i64)> =
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&b)
            .await
            .unwrap();
    meta::enable(&a).await.unwrap();
    meta::enable(&b).await.unwrap();
    deliver(&b, &drain(&a).await).await.unwrap();
    // Payday and clearing state already existed on this device. Preserve those
    // choices until the user explicitly chooses their cloud versions.
    let conflicts = store::status(&b, ORIGIN, "alice").await.unwrap().conflicts;
    assert_eq!(conflicts.len(), 2);
    for c in conflicts {
        assert!(
            c.record_id == "metadata:settings:1"
                || c.record_id.starts_with("metadata:verification:")
        );
        choose(&b, &c.record_id, "remote").await;
    }
    deliver(&b, &[]).await.unwrap();
    assert_clean(&a).await;
    assert_clean(&b).await;
    assert_eq!(
        sqlx::query_as::<_, (String, i64)>("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&b)
            .await
            .unwrap(),
        before
    );
    let entry: String = sqlx::query_scalar(
        "SELECT transaction_id FROM loan_payment_parts WHERE payment_id=? AND component='fee'",
    )
    .bind(&payment)
    .fetch_one(&b)
    .await
    .unwrap();
    transactions::remove_confirmed(&b, &entry, true)
        .await
        .unwrap();
    deliver(&a, &drain(&b).await).await.unwrap();
    assert_clean(&a).await;
    for p in [&a, &b] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT remaining_principal FROM loan_contracts WHERE id='contract'"
            )
            .fetch_one(p)
            .await
            .unwrap(),
            100000
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM loan_sync_reservations")
                .fetch_one(p)
                .await
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn concurrent_contract_repayments_merge_principal_and_reverse_atomically() {
    let a = db(true).await;
    seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let b = db(true).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    for p in [&a, &b] {
        let input=serde_json::from_value(json!({"contract_id":"contract","account_id":"bank","date":"2024-04-01","total":"5000","principal":"5000","interest":"0","fee":"0","currency":"THB"})).unwrap();
        crate::loans::repay(p, input).await.unwrap();
    }
    deliver(&b, &drain(&a).await).await.unwrap();
    deliver(&a, &drain(&b).await).await.unwrap();
    assert_clean(&a).await;
    assert_clean(&b).await;
    for p in [&a, &b] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT remaining_principal FROM loan_contracts WHERE id='contract'"
            )
            .fetch_one(p)
            .await
            .unwrap(),
            85000
        );
    }
}
#[tokio::test]
async fn metadata_conflicts_keep_offline_edits_and_require_explicit_choice() {
    let a = db(true).await;
    seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let b = db(true).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    sqlx::query("UPDATE subscriptions SET amount=100 WHERE id='subscription'")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE subscriptions SET amount=200 WHERE id='subscription'")
        .execute(&b)
        .await
        .unwrap();
    deliver(&b, &drain(&a).await).await.unwrap();
    assert_eq!(
        store::status(&b, ORIGIN, "alice")
            .await
            .unwrap()
            .conflict_count,
        1
    );
    choose(&b, "metadata:subscription:subscription", "local").await;
    deliver(&a, &drain(&b).await).await.unwrap();
    assert_clean(&a).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT amount FROM subscriptions")
            .fetch_one(&a)
            .await
            .unwrap(),
        200
    );
    sqlx::query("DELETE FROM planner_items WHERE id='manual'")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE planner_amounts SET amount=0 WHERE item_id='manual'")
        .execute(&b)
        .await
        .unwrap();
    deliver(&b, &drain(&a).await).await.unwrap();
    assert!(
        store::status(&b, ORIGIN, "alice")
            .await
            .unwrap()
            .conflict_count
            >= 1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT amount FROM planner_amounts WHERE item_id='manual'")
            .fetch_one(&b)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn statement_and_reconciliation_follow_payment_deletion_and_keep_evidence() {
    let a = db(true).await;
    let (_, payment, _) = seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let b = db(true).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    assert!(transactions::remove_confirmed(&b, &payment, false)
        .await
        .is_err());
    transactions::remove_confirmed(&b, &payment, true)
        .await
        .unwrap();
    deliver(&a, &drain(&b).await).await.unwrap();
    // The other device separately confirms deletion of reconciled history.
    choose(&a, &format!("transaction:{payment}"), "remote").await;
    deliver(&a, &[]).await.unwrap();
    for p in [&a, &b] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT needs_review FROM reconciliations")
                .fetch_one(p)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reconciliation_entries")
                .fetch_one(p)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM card_payment_allocations")
                .fetch_one(p)
                .await
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn reconciliation_ids_are_remapped_and_logo_hash_deduplicates() {
    let a = db(true).await;
    seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let changes = drain(&a).await;
    let b = db(true).await;
    sqlx::query("INSERT INTO sqlite_sequence(name,seq) VALUES('reconciliations',40)")
        .execute(&b)
        .await
        .unwrap();
    let logo: (String, Vec<u8>) = sqlx::query_as("SELECT content_hash,data FROM logo_assets")
        .fetch_one(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO logo_assets(id,content_hash,mime_type,width,height,data) VALUES('other-local-id',?,'image/png',1,1,?)").bind(logo.0).bind(logo.1).execute(&b).await.unwrap();
    deliver(&b, &changes).await.unwrap();
    assert_clean(&b).await;
    let linked: (i64, String) = sqlx::query_as("SELECT id,sync_id FROM reconciliations")
        .fetch_one(&b)
        .await
        .unwrap();
    assert!(linked.0 > 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT sync_id FROM reconciliations")
            .fetch_one(&a)
            .await
            .unwrap(),
        linked.1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT reconciliation_id FROM transaction_verifications WHERE status='reconciled'"
        )
        .fetch_one(&b)
        .await
        .unwrap(),
        linked.0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT logo_asset_id FROM payees WHERE id='payee'")
            .fetch_one(&b)
            .await
            .unwrap(),
        "other-local-id"
    );
}

#[tokio::test]
async fn payroll_upgrade_maps_equal_contract_deductions_exactly_and_reverses() {
    let a = crate::sync::test_support::database().await;
    sqlx::raw_sql("UPDATE accounts SET loan_type='personal_loan' WHERE id='home'; INSERT INTO loan_facilities VALUES('home',3000000); INSERT INTO loan_contracts(id,account_id,name,principal,remaining_principal,borrowing_date,receiving_account_id,payment_account_id,interest_rate,monthly_amount,installment_count,first_due_date,borrowing_kind) VALUES('contract','home','Contract',2000000,2000000,'2023-01-01','bank','bank',0,920000,12,'2024-01-25','existing'); UPDATE income_deductions SET debt_account_id='home' WHERE id='student';").execute(&a).await.unwrap();
    let mut input = crate::sync::test_support::input();
    for line in input.deductions.iter_mut().take(2) {
        line.contract_id = Some("contract".into());
        line.debt_account_id = Some("home".into());
        line.amount = "100000".into();
        line.interest = "0".into();
        line.fee = "0".into();
    }
    crate::salary_payments::record(&a, input).await.unwrap();
    connect(&a, false).await;
    deliver(&a, &[]).await.unwrap();
    let b = db(false).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    let balances: Vec<(String, i64)> =
        sqlx::query_as("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&b)
            .await
            .unwrap();
    meta::enable(&a).await.unwrap();
    meta::enable(&b).await.unwrap();
    deliver(&b, &drain(&a).await).await.unwrap();
    assert_clean(&b).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
            .fetch_one(&b)
            .await
            .unwrap(),
        1800000
    );
    let parts = |p: SqlitePool| async move {
        sqlx::query_as::<_,(String,String,String,i64)>("SELECT transaction_id,payment_id,contract_id,amount FROM loan_payment_parts ORDER BY transaction_id").fetch_all(&p).await.unwrap()
    };
    assert_eq!(parts(a.clone()).await, parts(b.clone()).await);
    assert_eq!(
        sqlx::query_as::<_, (String, i64)>("SELECT id,current_balance FROM accounts ORDER BY id")
            .fetch_all(&b)
            .await
            .unwrap(),
        balances
    );
    let id: String = sqlx::query_scalar(
        "SELECT transaction_id FROM salary_payment_transactions WHERE role='net'",
    )
    .fetch_one(&b)
    .await
    .unwrap();
    transactions::remove_confirmed(&b, &id, true).await.unwrap();
    deliver(&a, &drain(&b).await).await.unwrap();
    for p in [&a, &b] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT remaining_principal FROM loan_contracts")
                .fetch_one(p)
                .await
                .unwrap(),
            2000000
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM loan_sync_reservations")
                .fetch_one(p)
                .await
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn new_borrowing_metadata_restores_and_reverses_on_another_device() {
    let a = db(true).await;
    sqlx::raw_sql("UPDATE settings SET currency='THB'; INSERT INTO accounts(id,name,type,loan_type) VALUES('bank','Bank','bank',NULL),('loan','Loan','loan','personal_loan'); INSERT INTO loan_facilities VALUES('loan',5000000);").execute(&a).await.unwrap();
    let input=serde_json::from_value(json!({"id":null,"account_id":"loan","name":"Borrowing 1","principal":"2000000","remaining_principal":"2000000","borrowing_date":"2024-01-31","receiving_account_id":"bank","payment_account_id":"bank","interest_rate":75000,"monthly_amount":"210000","installment_count":10,"first_due_date":"2024-02-29","schedule_end":null,"notes":"","borrowing_kind":"new","currency":"THB"})).unwrap();
    crate::loans::save(&a, input).await.unwrap();
    deliver(&a, &[]).await.unwrap();
    let b = db(true).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    assert_clean(&b).await;
    let id: String = sqlx::query_scalar("SELECT borrowing_transaction_id FROM loan_contracts")
        .fetch_one(&b)
        .await
        .unwrap();
    transactions::remove_confirmed(&b, &id, true).await.unwrap();
    deliver(&a, &drain(&b).await).await.unwrap();
    assert_clean(&a).await;
    for p in [&a, &b] {
        assert_eq!(sqlx::query_as::<_,(i64,bool,Option<String>)>("SELECT remaining_principal,needs_review,borrowing_transaction_id FROM loan_contracts").fetch_one(p).await.unwrap(),(0,true,None));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT sum(current_balance) FROM accounts")
                .fetch_one(p)
                .await
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn malformed_metadata_rolls_back_and_opt_in_preserves_pending_ciphertext() {
    let p = db(true).await;
    seed(&p).await;
    let records = metadata_records(&p).await;
    for (kind, field, bad) in [
        ("logo", "width", "128"),
        ("subscription", "management_url", "javascript:alert(1)"),
        ("planner-amount", "amount", "01"),
        ("contract", "basis_principal", "100001"),
    ] {
        let mut r = records
            .iter()
            .find(|r| matches!(&r.value,Some(store::Value::Metadata(v)) if v.kind==kind))
            .unwrap()
            .clone();
        if let Some(store::Value::Metadata(v)) = &mut r.value {
            v.fields.insert(field.into(), Some(bad.into()));
        }
        assert!(r.validate().is_err(), "{kind}");
    }
    let original = records
        .iter()
        .find(|r| r.id == "statement:statement")
        .unwrap();
    let mut invalid = original.clone();
    if let Some(store::Value::Metadata(v)) = &mut invalid.value {
        v.fields.insert("amount".into(), Some("6000".into()));
        v.children.get_mut("plan").unwrap()[0].insert("target".into(), Some("6000".into()));
        v.children.get_mut("allocations").unwrap().push(
            [(
                String::from("transaction_id"),
                Some(String::from("zz-missing")),
            )]
            .into(),
        );
    }
    assert!(
        !ledger::apply(&mut *p.acquire().await.unwrap(), &invalid, false)
            .await
            .unwrap()
    );
    assert_eq!(
        &meta::current(&mut *p.acquire().await.unwrap(), &original.id)
            .await
            .unwrap(),
        original
    );
    let legacy = db(false).await;
    deliver(&legacy, &[]).await.unwrap();
    let bound = store::binding(&legacy).await.unwrap().unwrap();
    let pending = store::prepare(&legacy, &Zeroizing::new([9; 32]), &bound)
        .await
        .unwrap()
        .unwrap();
    meta::enable(&legacy).await.unwrap();
    assert_eq!(store::pending(&legacy).await.unwrap().unwrap(), pending);
    let changes = drain(&p).await;
    let blocked = db(false).await;
    let head = store::binding(&blocked).await.unwrap().unwrap().head();
    assert_eq!(
        deliver(&blocked, &changes).await.unwrap_err(),
        meta::CONSENT
    );
    assert_eq!(
        store::binding(&blocked).await.unwrap().unwrap().head(),
        head
    );
}

#[tokio::test]
async fn late_reconciliation_evidence_is_retained_and_review_status_is_queued() {
    let a = db(true).await;
    let (_, payment, _) = seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    drain(&a).await;
    let mut record = metadata_records(&a)
        .await
        .into_iter()
        .find(|r| matches!(&r.value,Some(store::Value::Metadata(v)) if v.kind=="reconciliation"))
        .unwrap();
    record.id = "reconciliation:late-history".into();
    if let Some(store::Value::Metadata(v)) = &mut record.value {
        v.fields
            .insert("sync_id".into(), Some("late-history".into()));
        v.fields.insert("needs_review".into(), Some("0".into()));
    }
    transactions::remove_confirmed(&a, &payment, true)
        .await
        .unwrap();
    drain(&a).await;
    let head = store::binding(&a).await.unwrap().unwrap().head();
    let request = protocol::encrypt_with_protocol(
        &Zeroizing::new([9; 32]),
        "dataset",
        "key",
        &record.key(),
        0,
        &head,
        false,
        &serde_json::to_vec(&record).unwrap(),
        5,
    )
    .unwrap();
    let change = Change {
        sequence: (head.number().unwrap() + 1).to_string(),
        version: "1".into(),
        request,
    };
    deliver(&a, &[change]).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT needs_review FROM reconciliations WHERE sync_id='late-history'"
        )
        .fetch_one(&a)
        .await
        .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM reconciliation_entries e JOIN reconciliations r ON r.id=e.reconciliation_id WHERE r.sync_id='late-history'").fetch_one(&a).await.unwrap(),1);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM cloud_sync_dirty WHERE record_id='metadata:reconciliation:late-history'").fetch_one(&a).await.unwrap(),1);
    let updates = drain(&a).await;
    assert!(!updates.is_empty());
    assert_clean(&a).await;
}

#[tokio::test]
async fn catalog_rename_keeps_account_links_when_label_update_arrives_first() {
    let a = db(true).await;
    seed(&a).await;
    deliver(&a, &[]).await.unwrap();
    let b = db(true).await;
    deliver(&b, &drain(&a).await).await.unwrap();
    sqlx::query(
        "UPDATE institutions SET name='Renamed bank',name_key='renamed bank' WHERE id='bank-002'",
    )
    .execute(&a)
    .await
    .unwrap();
    deliver(&b, &drain(&a).await).await.unwrap();
    assert_clean(&b).await;
    assert_eq!(
        sqlx::query_as::<_, (String, String)>(
            "SELECT institution_id,institution FROM accounts WHERE id='bank'"
        )
        .fetch_one(&b)
        .await
        .unwrap(),
        ("bank-002".into(), "Renamed bank".into())
    );
}
