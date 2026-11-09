-- Expanding an existing category-only connection requires fresh consent.
ALTER TABLE cloud_sync_binding ADD COLUMN financial_enabled INTEGER NOT NULL DEFAULT 0 CHECK(financial_enabled IN (0,1));

-- Financial metadata, retained in backups: transaction grouping and historical
-- labels for imported ledger entries whose planning definitions are device-local.
CREATE TABLE ledger_sync_members (
 transaction_id TEXT PRIMARY KEY NOT NULL,
 group_id TEXT NOT NULL
);
CREATE INDEX ledger_sync_group ON ledger_sync_members(group_id);
CREATE TABLE ledger_sync_imports (
 group_id TEXT PRIMARY KEY NOT NULL,
 payload TEXT NOT NULL
);
INSERT INTO ledger_sync_members(transaction_id,group_id)
 SELECT t.id,COALESCE('salary-'||s.payment_id,'loan-'||p.payment_id,t.id)
 FROM transactions t LEFT JOIN salary_payment_transactions s ON s.transaction_id=t.id
 LEFT JOIN loan_payment_parts p ON p.transaction_id=t.id;

CREATE TRIGGER ledger_sync_transaction_insert AFTER INSERT ON transactions BEGIN
 INSERT INTO ledger_sync_members(transaction_id,group_id) VALUES(NEW.id,NEW.id)
 ON CONFLICT(transaction_id) DO NOTHING;
 INSERT INTO cloud_sync_dirty(record_id)
 SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=NEW.id
 AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER ledger_sync_transaction_delete BEFORE DELETE ON transactions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=OLD.id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER ledger_sync_transaction_cleanup AFTER DELETE ON transactions BEGIN
 DELETE FROM ledger_sync_imports WHERE group_id=(SELECT group_id FROM ledger_sync_members WHERE transaction_id=OLD.id)
 AND NOT EXISTS(SELECT 1 FROM ledger_sync_members m JOIN transactions t ON t.id=m.transaction_id WHERE m.group_id=ledger_sync_imports.group_id);
END;
-- Group membership is assigned in the same business transaction as the entries.
CREATE TRIGGER ledger_sync_salary_member AFTER INSERT ON salary_payment_transactions BEGIN
 DELETE FROM cloud_sync_dirty WHERE record_id='transaction:'||NEW.transaction_id;
 UPDATE ledger_sync_members SET group_id='salary-'||NEW.payment_id WHERE transaction_id=NEW.transaction_id;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:salary-'||NEW.payment_id
 WHERE EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER ledger_sync_loan_member AFTER INSERT ON loan_payment_parts
WHEN NOT EXISTS(SELECT 1 FROM salary_payment_transactions WHERE transaction_id=NEW.transaction_id)
BEGIN
 DELETE FROM cloud_sync_dirty WHERE record_id='transaction:'||NEW.transaction_id;
 UPDATE ledger_sync_members SET group_id='loan-'||NEW.payment_id WHERE transaction_id=NEW.transaction_id;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:loan-'||NEW.payment_id
 WHERE EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER ledger_sync_account_insert AFTER INSERT ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('account:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
-- Balances and timestamps are deliberately excluded: balances are derived.
CREATE TRIGGER ledger_sync_account_update AFTER UPDATE OF name,type,opening_balance,is_archived,institution,notes,credit_limit,statement_day,payment_due_day,loan_type,last_four,interest_rate_ten_thousandths,monthly_installment,initial_loan_amount ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
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
CREATE TRIGGER ledger_sync_account_delete AFTER DELETE ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND financial_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('account:'||OLD.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
