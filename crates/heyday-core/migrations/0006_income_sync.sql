-- Existing ledger consent does not opt users into uploading income definitions.
ALTER TABLE cloud_sync_binding ADD COLUMN income_enabled INTEGER NOT NULL DEFAULT 0 CHECK(income_enabled IN (0,1));

CREATE TRIGGER income_sync_insert AFTER INSERT ON incomes
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_update AFTER UPDATE OF name,destination_account_id,type,estimated_amount,recurrence_frequency,recurrence_day_of_month,is_auto_create_transaction,is_active ON incomes
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
AND (OLD.name IS NOT NEW.name OR OLD.destination_account_id IS NOT NEW.destination_account_id OR OLD.type IS NOT NEW.type
 OR OLD.estimated_amount IS NOT NEW.estimated_amount OR OLD.recurrence_frequency IS NOT NEW.recurrence_frequency
 OR OLD.recurrence_day_of_month IS NOT NEW.recurrence_day_of_month OR OLD.is_auto_create_transaction IS NOT NEW.is_auto_create_transaction OR OLD.is_active IS NOT NEW.is_active)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_delete AFTER DELETE ON incomes
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||OLD.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_deduction_insert AFTER INSERT ON income_deductions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||NEW.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_deduction_update AFTER UPDATE ON income_deductions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
AND (OLD.income_id IS NOT NEW.income_id OR OLD.name IS NOT NEW.name OR OLD.description IS NOT NEW.description OR OLD.amount IS NOT NEW.amount OR OLD.debt_account_id IS NOT NEW.debt_account_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||OLD.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'income:'||NEW.income_id WHERE NEW.income_id<>OLD.income_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_deduction_delete AFTER DELETE ON income_deductions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||OLD.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
-- Payslips with no net deposit or debt principal still need history and tombstones.
CREATE TRIGGER income_sync_payroll_insert AFTER INSERT ON salary_payments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('transaction:salary-'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER income_sync_payroll_delete AFTER DELETE ON salary_payments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('transaction:salary-'||OLD.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
