-- Additive follow-up to the immutable complete-sync migration.
-- Native safety effects caused by downloaded transactions must also reach other
-- devices (e.g. a reconciliation created while the deleting device was offline).
CREATE TRIGGER full_sync_remote_statement_review AFTER UPDATE OF needs_review ON card_statements
WHEN OLD.needs_review=0 AND NEW.needs_review=1 AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:statement:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER full_sync_remote_reconciliation_review AFTER UPDATE OF needs_review ON reconciliations
WHEN OLD.needs_review=0 AND NEW.needs_review=1 AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:reconciliation:'||NEW.sync_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER full_sync_remote_borrowing_reversal AFTER UPDATE OF needs_review ON loan_contracts
WHEN OLD.needs_review=0 AND NEW.needs_review=1 AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:contract:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER full_sync_remote_purchase_removal AFTER UPDATE OF purchase_transaction_id ON installments
WHEN OLD.purchase_transaction_id IS NOT NULL AND NEW.purchase_transaction_id IS NULL AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:installment:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER full_sync_remote_allocation_removal AFTER DELETE ON card_payment_allocations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:statement:'||OLD.statement_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER full_sync_remote_coverage_removal AFTER DELETE ON card_statement_installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=1 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:statement:'||OLD.statement_id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
