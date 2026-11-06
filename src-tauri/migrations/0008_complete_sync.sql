-- Complete financial sync is separately consented. Keep existing history/IDs.
ALTER TABLE cloud_sync_binding ADD COLUMN full_enabled INTEGER NOT NULL DEFAULT 0 CHECK(full_enabled IN (0,1));
ALTER TABLE reconciliations ADD COLUMN sync_id TEXT;
UPDATE reconciliations SET sync_id=lower(hex(randomblob(16)));
CREATE UNIQUE INDEX reconciliation_sync_identity ON reconciliations(sync_id);
CREATE TRIGGER reconciliation_sync_identity AFTER INSERT ON reconciliations WHEN NEW.sync_id IS NULL BEGIN
 UPDATE reconciliations SET sync_id=lower(hex(randomblob(16))) WHERE id=NEW.id;
END;
-- Financial upgrade bookkeeping is retained in backups, without keys or ownership.
CREATE TABLE loan_sync_reservations (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 contract_id TEXT NOT NULL REFERENCES loan_contracts(id) ON DELETE CASCADE,
 amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount>0)
);

CREATE TRIGGER full_sync_logo_insert AFTER INSERT ON logo_assets
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:logo:'||NEW.content_hash 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_logo_update AFTER UPDATE ON logo_assets
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.content_hash IS NOT NEW.content_hash OR OLD.mime_type IS NOT NEW.mime_type OR OLD.width IS NOT NEW.width OR OLD.height IS NOT NEW.height OR OLD.data IS NOT NEW.data)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:logo:'||OLD.content_hash 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:logo:'||NEW.content_hash 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_logo_delete AFTER DELETE ON logo_assets
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:logo:'||OLD.content_hash 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_institution_insert AFTER INSERT ON institutions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:institution:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_institution_update AFTER UPDATE ON institutions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.short_name IS NOT NEW.short_name OR OLD.bank_code IS NOT NEW.bank_code OR OLD.swift_code IS NOT NEW.swift_code OR OLD.logo IS NOT NEW.logo OR OLD.is_archived IS NOT NEW.is_archived OR OLD.logo_asset_id IS NOT NEW.logo_asset_id OR OLD.logo_mode IS NOT NEW.logo_mode)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:institution:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:institution:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_institution_delete AFTER DELETE ON institutions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:institution:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payee_insert AFTER INSERT ON payees
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payee:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payee_update AFTER UPDATE ON payees
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.is_archived IS NOT NEW.is_archived OR OLD.logo_asset_id IS NOT NEW.logo_asset_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payee:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payee:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payee_delete AFTER DELETE ON payees
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payee:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_provider_insert AFTER INSERT ON subscription_providers
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:provider:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_provider_update AFTER UPDATE ON subscription_providers
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.is_archived IS NOT NEW.is_archived OR OLD.builtin_icon IS NOT NEW.builtin_icon OR OLD.logo_mode IS NOT NEW.logo_mode OR OLD.logo_asset_id IS NOT NEW.logo_asset_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:provider:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:provider:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_provider_delete AFTER DELETE ON subscription_providers
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:provider:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_account_link_insert AFTER INSERT ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:account-link:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_account_link_update AFTER UPDATE ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.institution_id IS NOT NEW.institution_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:account-link:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:account-link:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_account_link_delete AFTER DELETE ON accounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:account-link:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_facility_insert AFTER INSERT ON loan_facilities
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:facility:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_facility_update AFTER UPDATE ON loan_facilities
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.account_id IS NOT NEW.account_id OR OLD.credit_limit IS NOT NEW.credit_limit)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:facility:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:facility:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_facility_delete AFTER DELETE ON loan_facilities
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:facility:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_contract_insert AFTER INSERT ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:contract:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_contract_update AFTER UPDATE ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.account_id IS NOT NEW.account_id OR OLD.name IS NOT NEW.name OR OLD.principal IS NOT NEW.principal OR OLD.remaining_principal IS NOT NEW.remaining_principal OR OLD.borrowing_date IS NOT NEW.borrowing_date OR OLD.receiving_account_id IS NOT NEW.receiving_account_id OR OLD.payment_account_id IS NOT NEW.payment_account_id OR OLD.interest_rate IS NOT NEW.interest_rate OR OLD.monthly_amount IS NOT NEW.monthly_amount OR OLD.installment_count IS NOT NEW.installment_count OR OLD.first_due_date IS NOT NEW.first_due_date OR OLD.schedule_end IS NOT NEW.schedule_end OR OLD.notes IS NOT NEW.notes OR OLD.borrowing_kind IS NOT NEW.borrowing_kind OR OLD.borrowing_transaction_id IS NOT NEW.borrowing_transaction_id OR OLD.needs_review IS NOT NEW.needs_review)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:contract:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:contract:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_contract_delete AFTER DELETE ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:contract:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_insert AFTER INSERT ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_update AFTER UPDATE ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.account_id IS NOT NEW.account_id OR OLD.debt_account_id IS NOT NEW.debt_account_id OR OLD.monthly_amount IS NOT NEW.monthly_amount OR OLD.installment_count IS NOT NEW.installment_count OR OLD.first_due_date IS NOT NEW.first_due_date OR OLD.purchase_kind IS NOT NEW.purchase_kind OR OLD.purchase_transaction_id IS NOT NEW.purchase_transaction_id OR OLD.interest_rate_millis IS NOT NEW.interest_rate_millis)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_delete AFTER DELETE ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_subscription_insert AFTER INSERT ON subscriptions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:subscription:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_subscription_update AFTER UPDATE ON subscriptions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.account_id IS NOT NEW.account_id OR OLD.category_id IS NOT NEW.category_id OR OLD.amount IS NOT NEW.amount OR OLD.frequency IS NOT NEW.frequency OR OLD.first_billing_date IS NOT NEW.first_billing_date OR OLD.end_date IS NOT NEW.end_date OR OLD.is_active IS NOT NEW.is_active OR OLD.logo_asset_id IS NOT NEW.logo_asset_id OR OLD.managed_via IS NOT NEW.managed_via OR OLD.management_url IS NOT NEW.management_url OR OLD.provider_id IS NOT NEW.provider_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:subscription:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:subscription:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_subscription_delete AFTER DELETE ON subscriptions
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:subscription:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payment_plan_insert AFTER INSERT ON payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payment-plan:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payment_plan_update AFTER UPDATE ON payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.type IS NOT NEW.type OR OLD.account_id IS NOT NEW.account_id OR OLD.destination_account_id IS NOT NEW.destination_account_id OR OLD.category_id IS NOT NEW.category_id OR OLD.amount IS NOT NEW.amount OR OLD.date IS NOT NEW.date)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payment-plan:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payment-plan:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payment_plan_delete AFTER DELETE ON payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payment-plan:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_item_insert AFTER INSERT ON planner_items
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-item:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_item_update AFTER UPDATE ON planner_items
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.category_id IS NOT NEW.category_id OR OLD.name IS NOT NEW.name OR OLD.description IS NOT NEW.description OR OLD.card_name IS NOT NEW.card_name OR OLD.transaction_category_id IS NOT NEW.transaction_category_id OR OLD.schedule_amount IS NOT NEW.schedule_amount OR OLD.schedule_start IS NOT NEW.schedule_start OR OLD.schedule_end IS NOT NEW.schedule_end)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-item:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-item:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_item_delete AFTER DELETE ON planner_items
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-item:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_amount_insert AFTER INSERT ON planner_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-amount:'||NEW.item_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_amount_update AFTER UPDATE ON planner_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.item_id IS NOT NEW.item_id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-amount:'||OLD.item_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-amount:'||NEW.item_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_planner_amount_delete AFTER DELETE ON planner_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:planner-amount:'||OLD.item_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_debt_amount_insert AFTER INSERT ON planner_debt_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:debt-amount:'||NEW.account_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_debt_amount_update AFTER UPDATE ON planner_debt_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.account_id IS NOT NEW.account_id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:debt-amount:'||OLD.account_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:debt-amount:'||NEW.account_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_debt_amount_delete AFTER DELETE ON planner_debt_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:debt-amount:'||OLD.account_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_amount_insert AFTER INSERT ON planner_installment_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment-amount:'||NEW.installment_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_amount_update AFTER UPDATE ON planner_installment_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.installment_id IS NOT NEW.installment_id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment-amount:'||OLD.installment_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment-amount:'||NEW.installment_id||':'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installment_amount_delete AFTER DELETE ON planner_installment_amounts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:installment-amount:'||OLD.installment_id||':'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_cycle_insert AFTER INSERT ON planner_months
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:cycle:'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_cycle_update AFTER UPDATE ON planner_months
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.month IS NOT NEW.month OR OLD.status IS NOT NEW.status)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:cycle:'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:cycle:'||NEW.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_cycle_delete AFTER DELETE ON planner_months
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:cycle:'||OLD.month 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_opening_insert AFTER INSERT ON planner_opening
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:opening:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_opening_update AFTER UPDATE ON planner_opening
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.month IS NOT NEW.month OR OLD.amount IS NOT NEW.amount)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:opening:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:opening:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_opening_delete AFTER DELETE ON planner_opening
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:opening:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_settings_insert AFTER INSERT ON settings
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:settings:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_settings_update AFTER UPDATE ON settings
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.currency IS NOT NEW.currency OR OLD.period_start_day IS NOT NEW.period_start_day)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:settings:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:settings:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_settings_delete AFTER DELETE ON settings
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:settings:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_insert AFTER INSERT ON card_limit_groups
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_update AFTER UPDATE ON card_limit_groups
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.name IS NOT NEW.name OR OLD.credit_limit IS NOT NEW.credit_limit)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_delete AFTER DELETE ON card_limit_groups
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_statement_insert AFTER INSERT ON card_statements
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_statement_update AFTER UPDATE ON card_statements
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.id IS NOT NEW.id OR OLD.account_id IS NOT NEW.account_id OR OLD.start_date IS NOT NEW.start_date OR OLD.end_date IS NOT NEW.end_date OR OLD.due_date IS NOT NEW.due_date OR OLD.amount IS NOT NEW.amount OR OLD.minimum IS NOT NEW.minimum OR OLD.needs_review IS NOT NEW.needs_review)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_statement_delete AFTER DELETE ON card_statements
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payoff_insert AFTER INSERT ON loan_payoffs
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payoff:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payoff_update AFTER UPDATE ON loan_payoffs
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.account_id IS NOT NEW.account_id OR OLD.paid_off_on IS NOT NEW.paid_off_on)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payoff:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payoff:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_payoff_delete AFTER DELETE ON loan_payoffs
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:payoff:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_insert AFTER INSERT ON reconciliations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||NEW.sync_id WHERE NEW.sync_id IS NOT NULL
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_update AFTER UPDATE ON reconciliations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.sync_id IS NOT NEW.sync_id OR OLD.account_id IS NOT NEW.account_id OR OLD.completed_at IS NOT NEW.completed_at OR OLD.confirmed_balance IS NOT NEW.confirmed_balance OR OLD.opening_balance IS NOT NEW.opening_balance OR OLD.needs_review IS NOT NEW.needs_review)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||OLD.sync_id WHERE OLD.sync_id IS NOT NULL
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||NEW.sync_id WHERE NEW.sync_id IS NOT NULL
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_delete AFTER DELETE ON reconciliations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||OLD.sync_id WHERE OLD.sync_id IS NOT NULL
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_verification_insert AFTER INSERT ON transaction_verifications
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:verification:'||NEW.transaction_id||':'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_verification_update AFTER UPDATE ON transaction_verifications
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
AND (OLD.transaction_id IS NOT NEW.transaction_id OR OLD.account_id IS NOT NEW.account_id OR OLD.status IS NOT NEW.status OR OLD.reconciliation_id IS NOT NEW.reconciliation_id)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:verification:'||OLD.transaction_id||':'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:verification:'||NEW.transaction_id||':'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_verification_delete AFTER DELETE ON transaction_verifications
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:verification:'||OLD.transaction_id||':'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_members_insert AFTER INSERT ON card_limit_members
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||NEW.group_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_members_update AFTER UPDATE ON card_limit_members
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||OLD.group_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||NEW.group_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_limit_members_delete AFTER DELETE ON card_limit_members
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:card-limit:'||OLD.group_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_allocations_insert AFTER INSERT ON card_payment_allocations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_allocations_update AFTER UPDATE ON card_payment_allocations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_allocations_delete AFTER DELETE ON card_payment_allocations
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_plans_insert AFTER INSERT ON card_payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_plans_update AFTER UPDATE ON card_payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_payment_plans_delete AFTER DELETE ON card_payment_plans
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_entries_insert AFTER INSERT ON card_statement_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_entries_update AFTER UPDATE ON card_statement_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_entries_delete AFTER DELETE ON card_statement_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_installments_insert AFTER INSERT ON card_statement_installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_installments_update AFTER UPDATE ON card_statement_installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||NEW.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_card_statement_installments_delete AFTER DELETE ON card_statement_installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:statement:'||OLD.statement_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_entries_insert AFTER INSERT ON reconciliation_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||(SELECT sync_id FROM reconciliations WHERE id=NEW.reconciliation_id) WHERE EXISTS(SELECT 1 FROM reconciliations WHERE id=NEW.reconciliation_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_entries_update AFTER UPDATE ON reconciliation_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||(SELECT sync_id FROM reconciliations WHERE id=OLD.reconciliation_id) WHERE EXISTS(SELECT 1 FROM reconciliations WHERE id=OLD.reconciliation_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||(SELECT sync_id FROM reconciliations WHERE id=NEW.reconciliation_id) WHERE EXISTS(SELECT 1 FROM reconciliations WHERE id=NEW.reconciliation_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_reconciliation_entries_delete AFTER DELETE ON reconciliation_entries
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:reconciliation:'||(SELECT sync_id FROM reconciliations WHERE id=OLD.reconciliation_id) WHERE EXISTS(SELECT 1 FROM reconciliations WHERE id=OLD.reconciliation_id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_selective_defaults_insert AFTER INSERT ON selective_defaults
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:default-history:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_selective_defaults_update AFTER UPDATE ON selective_defaults
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:default-history:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:default-history:'||NEW.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_selective_defaults_delete AFTER DELETE ON selective_defaults
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'metadata:default-history:'||OLD.account_id 
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_loan_contracts_link_insert AFTER INSERT ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=NEW.borrowing_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_loan_contracts_link_update AFTER UPDATE ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=OLD.borrowing_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=NEW.borrowing_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_loan_contracts_link_delete AFTER DELETE ON loan_contracts
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=OLD.borrowing_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installments_link_insert AFTER INSERT ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=NEW.purchase_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installments_link_update AFTER UPDATE ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=OLD.purchase_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=NEW.purchase_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;

CREATE TRIGGER full_sync_installments_link_delete AFTER DELETE ON installments
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0 AND full_enabled=1)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) SELECT 'transaction:'||group_id FROM ledger_sync_members WHERE transaction_id=OLD.purchase_transaction_id
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
