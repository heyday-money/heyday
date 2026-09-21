CREATE TABLE transactions_new (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('income', 'expense', 'transfer', 'repayment')),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    date TEXT NOT NULL,
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 0 AND 200),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((type IN ('transfer', 'repayment') AND destination_account_id IS NOT NULL AND destination_account_id <> account_id)
        OR (type IN ('income', 'expense') AND destination_account_id IS NULL))
);

INSERT INTO transactions_new (id, type, account_id, destination_account_id, amount, date, description, created_at)
SELECT id, type, account_id, destination_account_id, amount, date, description, created_at FROM transactions;
DROP TABLE transactions;
ALTER TABLE transactions_new RENAME TO transactions;

CREATE INDEX transactions_date_idx ON transactions(date DESC, created_at DESC);
CREATE INDEX transactions_account_idx ON transactions(account_id);
CREATE INDEX transactions_destination_idx ON transactions(destination_account_id);
