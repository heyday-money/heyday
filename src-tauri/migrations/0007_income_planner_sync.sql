-- Income forecast overrides require a separate opt-in on existing connections.
ALTER TABLE cloud_sync_binding ADD COLUMN planner_enabled INTEGER NOT NULL DEFAULT 0 CHECK(planner_enabled IN (0,1));

CREATE TRIGGER planner_sync_income_insert AFTER INSERT ON planner_income_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:income_'||NEW.month||'_'||NEW.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER planner_sync_income_update AFTER UPDATE ON planner_income_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
AND (OLD.income_id IS NOT NEW.income_id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:income_'||OLD.month||'_'||OLD.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:income_'||NEW.month||'_'||NEW.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER planner_sync_income_delete AFTER DELETE ON planner_income_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:income_'||OLD.month||'_'||OLD.income_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER planner_sync_deduction_insert AFTER INSERT ON planner_deduction_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:deduction_'||NEW.month||'_'||NEW.deduction_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER planner_sync_deduction_update AFTER UPDATE ON planner_deduction_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
AND (OLD.deduction_id IS NOT NEW.deduction_id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:deduction_'||OLD.month||'_'||OLD.deduction_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:deduction_'||NEW.month||'_'||NEW.deduction_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER planner_sync_deduction_delete AFTER DELETE ON planner_deduction_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND planner_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES('income-plan:deduction_'||OLD.month||'_'||OLD.deduction_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

