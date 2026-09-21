-- Monthly schedules describe repayments; they never create ledger entries.
CREATE TABLE installments (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    debt_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    monthly_amount INTEGER NOT NULL CHECK (typeof(monthly_amount) = 'integer' AND monthly_amount > 0),
    installment_count INTEGER NOT NULL CHECK (installment_count BETWEEN 1 AND 600),
    first_due_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (account_id <> debt_account_id)
);
CREATE INDEX installments_debt_account_idx ON installments(debt_account_id);
