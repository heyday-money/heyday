-- Plans are forecasts, never account ledger entries.
CREATE TABLE payment_plans (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    type TEXT NOT NULL CHECK (type IN ('expense', 'repayment')),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((type = 'expense' AND destination_account_id IS NULL)
        OR (type = 'repayment' AND destination_account_id IS NOT NULL AND destination_account_id <> account_id AND category_id IS NULL))
);
CREATE INDEX payment_plans_date_idx ON payment_plans(date);
