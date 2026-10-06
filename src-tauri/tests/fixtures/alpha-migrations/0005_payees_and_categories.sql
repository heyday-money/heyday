CREATE TABLE payees (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))
);
CREATE TABLE categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))
);
ALTER TABLE transactions ADD COLUMN payee_id TEXT REFERENCES payees(id) ON DELETE RESTRICT
    CHECK (payee_id IS NULL OR type = 'expense');
ALTER TABLE transactions ADD COLUMN category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT
    CHECK (category_id IS NULL OR type = 'expense');
CREATE INDEX transactions_payee_idx ON transactions(payee_id);
CREATE INDEX transactions_category_idx ON transactions(category_id);
