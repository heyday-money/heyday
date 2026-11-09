-- Keep the original recurrence constraint and foreign keys intact.
-- The API exposes this planning mode as recurrence_frequency = 'irregular'.
ALTER TABLE incomes ADD COLUMN is_irregular INTEGER NOT NULL DEFAULT 0
    CHECK (is_irregular IN (0, 1) AND (is_irregular = 0 OR estimated_amount = 0));

DROP TRIGGER income_sync_update;
CREATE TRIGGER income_sync_update AFTER UPDATE OF name,destination_account_id,type,estimated_amount,recurrence_frequency,recurrence_day_of_month,is_auto_create_transaction,is_active,is_irregular ON incomes
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND income_enabled=1)
AND (OLD.is_irregular IS NOT NEW.is_irregular OR OLD.name IS NOT NEW.name OR OLD.destination_account_id IS NOT NEW.destination_account_id OR OLD.type IS NOT NEW.type
 OR OLD.estimated_amount IS NOT NEW.estimated_amount OR OLD.recurrence_frequency IS NOT NEW.recurrence_frequency
 OR OLD.recurrence_day_of_month IS NOT NEW.recurrence_day_of_month OR OLD.is_auto_create_transaction IS NOT NEW.is_auto_create_transaction OR OLD.is_active IS NOT NEW.is_active)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income:'||NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
