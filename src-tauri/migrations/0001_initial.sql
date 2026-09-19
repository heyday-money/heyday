CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    currency TEXT CHECK (currency IS NULL OR (length(currency) = 3 AND currency = upper(currency))),
    period_start_day INTEGER NOT NULL DEFAULT 1 CHECK (period_start_day BETWEEN 1 AND 31)
);

-- Currency is selected by the user before any financial records are created.
INSERT INTO settings (id) VALUES (1);

CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    type TEXT NOT NULL CHECK (type IN ('cash', 'bank', 'credit_card', 'loan', 'investment')),
    opening_balance INTEGER NOT NULL DEFAULT 0,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE incomes (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    destination_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    type TEXT NOT NULL CHECK (type IN ('salary', 'variable', 'investment', 'other')),
    estimated_amount INTEGER NOT NULL CHECK (estimated_amount >= 0),
    recurrence_frequency TEXT NOT NULL DEFAULT 'monthly' CHECK (recurrence_frequency = 'monthly'),
    recurrence_day_of_month INTEGER NOT NULL CHECK (recurrence_day_of_month BETWEEN 1 AND 31),
    is_auto_create_transaction INTEGER NOT NULL DEFAULT 0 CHECK (is_auto_create_transaction IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX incomes_destination_account_idx ON incomes(destination_account_id);
