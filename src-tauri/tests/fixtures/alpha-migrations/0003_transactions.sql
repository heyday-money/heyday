ALTER TABLE accounts ADD COLUMN current_balance INTEGER NOT NULL DEFAULT 0;
UPDATE accounts SET current_balance = opening_balance;

CREATE TABLE transactions (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('income', 'expense', 'transfer', 'repayment')),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    date TEXT NOT NULL,
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 1 AND 200),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((type IN ('transfer', 'repayment') AND destination_account_id IS NOT NULL AND destination_account_id <> account_id)
        OR (type IN ('income', 'expense') AND destination_account_id IS NULL))
);
CREATE INDEX transactions_date_idx ON transactions(date DESC, created_at DESC);
CREATE INDEX transactions_account_idx ON transactions(account_id);
CREATE INDEX transactions_destination_idx ON transactions(destination_account_id);
