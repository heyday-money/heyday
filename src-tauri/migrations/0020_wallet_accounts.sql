-- Rebuild the type constraint atomically while retaining all account references.
PRAGMA defer_foreign_keys = ON;
-- Dropping the parent cascades to this table; retain its explicit overrides.
CREATE TEMP TABLE wallet_debt_overrides AS SELECT * FROM planner_debt_amounts;
DROP TRIGGER deduction_debt_insert;
DROP TRIGGER deduction_debt_update;
DROP TRIGGER deduction_account_type_update;
CREATE TABLE accounts_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    type TEXT NOT NULL CHECK (type IN ('cash', 'bank', 'wallet', 'credit_card', 'loan', 'investment')),
    opening_balance INTEGER NOT NULL DEFAULT 0,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
, institution TEXT, last_four TEXT CHECK (
    last_four IS NULL OR (type IN ('bank', 'credit_card') AND length(last_four) = 4 AND last_four NOT GLOB '*[^0-9]*')
), notes TEXT, credit_limit INTEGER CHECK (
    credit_limit IS NULL OR (type = 'credit_card' AND credit_limit >= 0)
), statement_day INTEGER CHECK (
    statement_day IS NULL OR (type = 'credit_card' AND statement_day BETWEEN 1 AND 31)
), payment_due_day INTEGER CHECK (
    payment_due_day IS NULL OR (type IN ('credit_card', 'loan') AND payment_due_day BETWEEN 1 AND 31)
), current_balance INTEGER NOT NULL DEFAULT 0, loan_type TEXT
    CHECK (loan_type IS NULL OR (type = 'loan' AND loan_type IN (
        'mortgage', 'auto_loan', 'student_loan', 'personal_loan', 'medical_debt', 'other_debt'
    ))), interest_rate_millis INTEGER CHECK (
    interest_rate_millis IS NULL OR (typeof(interest_rate_millis) = 'integer'
    AND type IN ('credit_card', 'loan') AND interest_rate_millis BETWEEN 0 AND 100000)
));

INSERT INTO accounts_new (id, name, type, opening_balance, is_archived, created_at, updated_at, institution, last_four, notes, credit_limit, statement_day, payment_due_day, current_balance, loan_type, interest_rate_millis)
SELECT id, name, type, opening_balance, is_archived, created_at, updated_at, institution, last_four, notes, credit_limit, statement_day, payment_due_day, current_balance, loan_type, interest_rate_millis FROM accounts;
DROP TABLE accounts;
ALTER TABLE accounts_new RENAME TO accounts;
INSERT INTO planner_debt_amounts SELECT * FROM wallet_debt_overrides;
DROP TABLE wallet_debt_overrides;
CREATE TRIGGER deduction_debt_insert BEFORE INSERT ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;
CREATE TRIGGER deduction_debt_update BEFORE UPDATE OF debt_account_id ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;
CREATE TRIGGER deduction_account_type_update BEFORE UPDATE OF type ON accounts
WHEN NEW.type NOT IN ('loan','credit_card') AND EXISTS(SELECT 1 FROM income_deductions WHERE debt_account_id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'Unlink salary deductions before changing the debt account type.'); END;
-- Validate every reference before clearing the deferred drop-table violations.
CREATE TEMP TABLE wallet_foreign_key_check (violations INTEGER CHECK (violations = 0));
INSERT INTO wallet_foreign_key_check SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE wallet_foreign_key_check;
PRAGMA defer_foreign_keys = OFF;
