CREATE TABLE reconciliations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    completed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    confirmed_balance INTEGER NOT NULL CHECK (typeof(confirmed_balance) = 'integer'),
    opening_balance INTEGER NOT NULL CHECK (typeof(opening_balance) = 'integer'),
    needs_review INTEGER NOT NULL DEFAULT 0 CHECK (needs_review IN (0, 1))
);
CREATE INDEX reconciliations_account_idx ON reconciliations(account_id, id);

CREATE TABLE transaction_verifications (
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'uncleared' CHECK (status IN ('uncleared', 'cleared', 'reconciled')),
    reconciliation_id INTEGER REFERENCES reconciliations(id) ON DELETE RESTRICT,
    PRIMARY KEY (transaction_id, account_id),
    CHECK ((status = 'reconciled' AND reconciliation_id IS NOT NULL)
        OR (status <> 'reconciled' AND reconciliation_id IS NULL))
);
CREATE INDEX transaction_verifications_account_idx ON transaction_verifications(account_id, status);

-- Retain immutable evidence even when the original transaction is deleted.
-- transaction_id is intentionally not a foreign key to the live ledger.
CREATE TABLE reconciliation_entries (
    reconciliation_id INTEGER NOT NULL REFERENCES reconciliations(id) ON DELETE CASCADE,
    transaction_id TEXT NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    type TEXT NOT NULL,
    balance_change INTEGER NOT NULL CHECK (typeof(balance_change) = 'integer'),
    PRIMARY KEY (reconciliation_id, transaction_id)
);
CREATE INDEX reconciliation_entries_transaction_idx ON reconciliation_entries(transaction_id);

INSERT INTO transaction_verifications (transaction_id, account_id)
SELECT id, account_id FROM transactions;
INSERT INTO transaction_verifications (transaction_id, account_id)
SELECT id, destination_account_id FROM transactions WHERE destination_account_id IS NOT NULL;

CREATE TRIGGER transaction_verification_insert AFTER INSERT ON transactions
BEGIN
    INSERT INTO transaction_verifications (transaction_id, account_id) VALUES (NEW.id, NEW.account_id);
    INSERT INTO transaction_verifications (transaction_id, account_id)
    SELECT NEW.id, NEW.destination_account_id WHERE NEW.destination_account_id IS NOT NULL;
END;

CREATE TRIGGER transaction_verification_owner_insert BEFORE INSERT ON transaction_verifications
WHEN NOT EXISTS (SELECT 1 FROM transactions WHERE id = NEW.transaction_id AND (account_id = NEW.account_id OR destination_account_id = NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'Verification must belong to a participating account.'); END;

CREATE TRIGGER transaction_verification_owner_update BEFORE UPDATE ON transaction_verifications
WHEN NOT EXISTS (SELECT 1 FROM transactions WHERE id = NEW.transaction_id AND (account_id = NEW.account_id OR destination_account_id = NEW.account_id))
    OR (NEW.reconciliation_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM reconciliations WHERE id = NEW.reconciliation_id AND account_id = NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'Verification and reconciliation must belong to the same account.'); END;
