-- Tracking uses one encrypted aggregate per asset, including its event history.
DROP TRIGGER tracking_create_sync_gate;
DROP TRIGGER tracking_binding_sync_gate;
DROP TRIGGER tracking_transaction_insert;
DROP TRIGGER tracking_transaction_update;
DROP TRIGGER tracking_event_immutable;
CREATE TABLE tracking_events_new (
 id TEXT PRIMARY KEY NOT NULL,
 account_id TEXT NOT NULL REFERENCES tracking_accounts(account_id) ON DELETE RESTRICT,
 revision INTEGER NOT NULL CHECK(typeof(revision)='integer' AND revision>0),
 date TEXT NOT NULL CHECK(length(date)=10),
 kind TEXT NOT NULL CHECK(kind IN ('value','reversal','lend','receive','write_off','buy','sell')),
 previous_value INTEGER NOT NULL CHECK(typeof(previous_value)='integer' AND previous_value>=0),
 next_value INTEGER NOT NULL CHECK(typeof(next_value)='integer' AND next_value>=0),
 notes TEXT NOT NULL DEFAULT '' CHECK(length(notes)<=1000),
 reverses_id TEXT UNIQUE REFERENCES tracking_events_new(id) DEFERRABLE INITIALLY DEFERRED,
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
 action_json TEXT NOT NULL CHECK(json_valid(action_json)),
 cash_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
 UNIQUE(account_id,revision),
 CHECK((kind='reversal')=(reverses_id IS NOT NULL))
);
INSERT INTO tracking_events_new(id,account_id,revision,date,kind,previous_value,next_value,notes,reverses_id,created_at,action_json)
SELECT id,account_id,revision,date,kind,previous_value,next_value,notes,reverses_id,created_at,
CASE WHEN kind='value' THEN json_object('kind','value','value',CAST(next_value AS TEXT)) ELSE json_object('kind','reverse','event_id',reverses_id) END FROM tracking_events;
DROP TABLE tracking_events;
ALTER TABLE tracking_events_new RENAME TO tracking_events;
CREATE INDEX tracking_events_history ON tracking_events(account_id,revision);
CREATE TRIGGER tracking_event_immutable BEFORE UPDATE ON tracking_events BEGIN SELECT RAISE(ABORT,'Tracking history cannot be overwritten.'); END;
CREATE TABLE tracking_entries (
 transaction_id TEXT PRIMARY KEY NOT NULL REFERENCES transactions(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
 event_id TEXT NOT NULL REFERENCES tracking_events(id) ON DELETE CASCADE,
 role TEXT NOT NULL CHECK(role IN ('principal','interest')),
 UNIQUE(event_id,role)
);
CREATE TRIGGER tracking_transaction_insert BEFORE INSERT ON transactions
WHEN EXISTS(SELECT 1 FROM accounts WHERE type='tracking' AND id IN (NEW.account_id,NEW.destination_account_id)) AND NOT EXISTS(SELECT 1 FROM tracking_entries WHERE transaction_id=NEW.id)
BEGIN SELECT RAISE(ABORT,'Use a dedicated Tracking action.'); END;
CREATE TRIGGER tracking_transaction_update BEFORE UPDATE ON transactions
WHEN EXISTS(SELECT 1 FROM tracking_entries WHERE transaction_id=OLD.id) OR EXISTS(SELECT 1 FROM accounts WHERE type='tracking' AND id IN (NEW.account_id,NEW.destination_account_id))
BEGIN SELECT RAISE(ABORT,'Use a dedicated Tracking action.'); END;
CREATE TRIGGER tracking_account_dirty_insert AFTER INSERT ON tracking_accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:tracking:'||NEW.account_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER tracking_account_dirty_update AFTER UPDATE ON tracking_accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:tracking:'||NEW.account_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER tracking_account_dirty_delete AFTER DELETE ON tracking_accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:tracking:'||OLD.account_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER tracking_base_dirty AFTER UPDATE OF name,notes,is_archived ON accounts
WHEN NEW.type='tracking' AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:tracking:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;

DROP TRIGGER ledger_sync_account_insert;
CREATE TRIGGER ledger_sync_account_insert AFTER INSERT ON accounts
WHEN NEW.type<>'tracking' AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('account:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

DROP TRIGGER ledger_sync_account_update;
CREATE TRIGGER ledger_sync_account_update AFTER UPDATE OF name,type,opening_balance,is_archived,institution,notes,credit_limit,statement_day,payment_due_day,loan_type,last_four,interest_rate_ten_thousandths,monthly_installment,initial_loan_amount ON accounts
WHEN NEW.type<>'tracking' AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
AND (OLD.name IS NOT NEW.name OR OLD.type IS NOT NEW.type OR OLD.opening_balance IS NOT NEW.opening_balance
 OR OLD.is_archived IS NOT NEW.is_archived OR OLD.institution IS NOT NEW.institution OR OLD.notes IS NOT NEW.notes
 OR OLD.credit_limit IS NOT NEW.credit_limit OR OLD.statement_day IS NOT NEW.statement_day
 OR OLD.payment_due_day IS NOT NEW.payment_due_day OR OLD.loan_type IS NOT NEW.loan_type OR OLD.last_four IS NOT NEW.last_four
 OR OLD.interest_rate_ten_thousandths IS NOT NEW.interest_rate_ten_thousandths
 OR OLD.monthly_installment IS NOT NEW.monthly_installment OR OLD.initial_loan_amount IS NOT NEW.initial_loan_amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('account:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

DROP TRIGGER ledger_sync_account_delete;
CREATE TRIGGER ledger_sync_account_delete AFTER DELETE ON accounts
WHEN OLD.type<>'tracking' AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('account:'||OLD.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
