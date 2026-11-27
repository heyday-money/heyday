-- Expand only the account-type CHECK, preserving the table, IDs, foreign keys and
-- triggers. SQLite's documented schema-text procedure avoids DROP TABLE cascades
-- inside SQLx's mandatory migration transaction. RESET reparses this connection;
-- subsequent CREATE TABLE statements advance the schema cookie for other connections.
CREATE TABLE tracking_migration_guard (ok INTEGER CHECK(ok=1));
INSERT INTO tracking_migration_guard
SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='accounts'
AND instr(sql, 'type IN (''cash'', ''bank'', ''wallet'', ''credit_card'', ''loan'', ''investment'')') > 0;
PRAGMA writable_schema=ON;
UPDATE sqlite_schema SET sql=replace(sql,
 'type IN (''cash'', ''bank'', ''wallet'', ''credit_card'', ''loan'', ''investment'')',
 'type IN (''cash'', ''bank'', ''wallet'', ''credit_card'', ''loan'', ''investment'', ''tracking'')')
WHERE type='table' AND name='accounts';
PRAGMA writable_schema=RESET;
DROP TABLE tracking_migration_guard;

CREATE TABLE tracking_accounts (
 account_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
 subtype TEXT NOT NULL CHECK(subtype IN ('receivable','vehicle','property','insurance_cash_value','other_asset')),
 opening_date TEXT NOT NULL CHECK(length(opening_date)=10),
 debtor TEXT CHECK(debtor IS NULL OR length(trim(debtor)) BETWEEN 1 AND 100),
 due_date TEXT CHECK(due_date IS NULL OR length(due_date)=10),
 linked_loan_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
 logo_asset_id TEXT REFERENCES logo_assets(id) ON DELETE RESTRICT,
 revision INTEGER NOT NULL DEFAULT 0 CHECK(typeof(revision)='integer' AND revision>=0),
 CHECK(subtype='receivable' OR (debtor IS NULL AND due_date IS NULL)),
 CHECK(linked_loan_id IS NULL OR subtype IN ('vehicle','property','other_asset')),
 CHECK(linked_loan_id IS NULL OR linked_loan_id<>account_id)
);
CREATE TABLE tracking_events (
 id TEXT PRIMARY KEY NOT NULL,
 account_id TEXT NOT NULL REFERENCES tracking_accounts(account_id) ON DELETE RESTRICT,
 revision INTEGER NOT NULL CHECK(typeof(revision)='integer' AND revision>0),
 date TEXT NOT NULL CHECK(length(date)=10),
 kind TEXT NOT NULL CHECK(kind IN ('value','reversal')),
 previous_value INTEGER NOT NULL CHECK(typeof(previous_value)='integer' AND previous_value>=0),
 next_value INTEGER NOT NULL CHECK(typeof(next_value)='integer' AND next_value>=0),
 notes TEXT NOT NULL DEFAULT '' CHECK(length(notes)<=1000),
 reverses_id TEXT UNIQUE REFERENCES tracking_events(id) DEFERRABLE INITIALLY DEFERRED,
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
 UNIQUE(account_id,revision),
 CHECK((kind='reversal')=(reverses_id IS NOT NULL))
);
CREATE INDEX tracking_events_history ON tracking_events(account_id,revision);
CREATE TRIGGER tracking_event_immutable BEFORE UPDATE ON tracking_events
BEGIN SELECT RAISE(ABORT,'Tracking history cannot be overwritten.'); END;
CREATE TRIGGER tracking_identity_immutable BEFORE UPDATE OF account_id,subtype,opening_date ON tracking_accounts
WHEN OLD.account_id IS NOT NEW.account_id OR OLD.subtype IS NOT NEW.subtype OR OLD.opening_date IS NOT NEW.opening_date
BEGIN SELECT RAISE(ABORT,'Tracking identity and opening date cannot be changed.'); END;
CREATE TRIGGER tracking_metadata_account BEFORE INSERT ON tracking_accounts
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='tracking')
BEGIN SELECT RAISE(ABORT,'Tracking metadata requires a Tracking account.'); END;
CREATE TRIGGER tracking_account_opening_immutable BEFORE UPDATE OF type,opening_balance ON accounts
WHEN (OLD.type='tracking' OR NEW.type='tracking') AND (OLD.type IS NOT NEW.type OR OLD.opening_balance IS NOT NEW.opening_balance)
BEGIN SELECT RAISE(ABORT,'Tracking type and opening value cannot be changed.'); END;
CREATE TRIGGER tracking_nonnegative_insert BEFORE INSERT ON accounts
WHEN NEW.type='tracking' AND (typeof(NEW.opening_balance)<>'integer' OR NEW.opening_balance<0 OR typeof(NEW.current_balance)<>'integer' OR NEW.current_balance<0)
BEGIN SELECT RAISE(ABORT,'Tracking values must be nonnegative integers.'); END;
CREATE TRIGGER tracking_nonnegative_update BEFORE UPDATE ON accounts
WHEN NEW.type='tracking' AND (typeof(NEW.current_balance)<>'integer' OR NEW.current_balance<0)
BEGIN SELECT RAISE(ABORT,'Tracking values must be nonnegative integers.'); END;
-- No unsupported records may enter protocol 5, even if sync is paused.
CREATE TRIGGER tracking_create_sync_gate BEFORE INSERT ON accounts
WHEN NEW.type='tracking' AND EXISTS(SELECT 1 FROM cloud_sync_binding)
BEGIN SELECT RAISE(ABORT,'Tracking cloud sync is not available yet.'); END;
CREATE TRIGGER tracking_binding_sync_gate BEFORE INSERT ON cloud_sync_binding
WHEN EXISTS(SELECT 1 FROM accounts WHERE type='tracking')
BEGIN SELECT RAISE(ABORT,'Tracking cloud sync is not available yet.'); END;
CREATE TRIGGER tracking_transaction_insert BEFORE INSERT ON transactions
WHEN EXISTS(SELECT 1 FROM accounts WHERE type='tracking' AND id IN (NEW.account_id,NEW.destination_account_id))
BEGIN SELECT RAISE(ABORT,'Use a dedicated Tracking action.'); END;
CREATE TRIGGER tracking_transaction_update BEFORE UPDATE ON transactions
WHEN EXISTS(SELECT 1 FROM accounts WHERE type='tracking' AND id IN (NEW.account_id,NEW.destination_account_id))
BEGIN SELECT RAISE(ABORT,'Use a dedicated Tracking action.'); END;
