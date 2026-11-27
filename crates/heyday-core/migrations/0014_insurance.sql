-- Policies and complete payment history travel as one encrypted aggregate.
CREATE TABLE insurance_policies (
 id TEXT PRIMARY KEY NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE TABLE insurance_links (
 policy_id TEXT NOT NULL REFERENCES insurance_policies(id) ON DELETE CASCADE,
 account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
 role TEXT NOT NULL CHECK(role IN ('asset','cash_value','payment')),
 PRIMARY KEY(policy_id,account_id,role)
);
CREATE UNIQUE INDEX insurance_cash_value_owner ON insurance_links(account_id) WHERE role='cash_value';
CREATE TABLE insurance_sources (
 policy_id TEXT PRIMARY KEY NOT NULL REFERENCES insurance_policies(id) ON DELETE CASCADE,
 subscription_id TEXT UNIQUE REFERENCES subscriptions(id) ON DELETE RESTRICT,
 plan_id TEXT UNIQUE REFERENCES payment_plans(id) ON DELETE RESTRICT,
 item_id TEXT UNIQUE REFERENCES planner_items(id) ON DELETE RESTRICT,
 CHECK((subscription_id IS NOT NULL)+(plan_id IS NOT NULL)+(item_id IS NOT NULL)=1)
);
CREATE TABLE insurance_entries (
 transaction_id TEXT PRIMARY KEY NOT NULL REFERENCES transactions(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
 policy_id TEXT NOT NULL REFERENCES insurance_policies(id) ON DELETE RESTRICT,
 occurrence TEXT NOT NULL,
 UNIQUE(policy_id,occurrence)
);
CREATE TRIGGER insurance_entry_immutable BEFORE UPDATE ON transactions
WHEN EXISTS(SELECT 1 FROM insurance_entries WHERE transaction_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'Reverse this premium from Insurance history.'); END;
CREATE TRIGGER insurance_dirty_insert AFTER INSERT ON insurance_policies
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:insurance:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER insurance_dirty_update AFTER UPDATE ON insurance_policies
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:insurance:'||NEW.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER insurance_dirty_delete AFTER DELETE ON insurance_policies
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN INSERT INTO cloud_sync_dirty(record_id) VALUES('metadata:insurance:'||OLD.id) ON CONFLICT(record_id) DO UPDATE SET revision=revision+1; END;
CREATE TABLE insurance_categories (
 policy_id TEXT NOT NULL REFERENCES insurance_policies(id) ON DELETE CASCADE,
 category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
 PRIMARY KEY(policy_id,category_id)
);
