CREATE TABLE account_logos (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    logo_asset_id TEXT NOT NULL REFERENCES logo_assets(id)
);
CREATE TRIGGER account_logo_cash_insert BEFORE INSERT ON account_logos
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='cash')
BEGIN SELECT RAISE(ABORT, 'Only cash accounts support an account logo.'); END;
CREATE TRIGGER account_logo_cash_update BEFORE UPDATE ON account_logos
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='cash')
BEGIN SELECT RAISE(ABORT, 'Only cash accounts support an account logo.'); END;
CREATE TRIGGER full_sync_account_logo_insert AFTER INSERT ON account_logos
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:account-logo:'||NEW.account_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER full_sync_account_logo_update AFTER UPDATE ON account_logos
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:account-logo:'||NEW.account_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER full_sync_account_logo_delete AFTER DELETE ON account_logos
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:account-logo:'||OLD.account_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
