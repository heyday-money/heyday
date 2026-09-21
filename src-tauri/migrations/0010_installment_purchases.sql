-- Existing schedules must not create debt during upgrade.
ALTER TABLE installments ADD COLUMN purchase_kind TEXT NOT NULL DEFAULT 'existing_purchase'
    CHECK (purchase_kind IN ('existing_purchase', 'new_purchase'));
ALTER TABLE installments ADD COLUMN purchase_transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL;
