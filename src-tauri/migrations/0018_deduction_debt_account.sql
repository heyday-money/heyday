-- Expected payroll repayments link to debt without posting ledger movements.
ALTER TABLE income_deductions ADD COLUMN debt_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT;
CREATE INDEX income_deductions_debt_idx ON income_deductions(debt_account_id);
CREATE TRIGGER deduction_debt_insert BEFORE INSERT ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;
CREATE TRIGGER deduction_debt_update BEFORE UPDATE OF debt_account_id ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;
CREATE TRIGGER deduction_account_type_update BEFORE UPDATE OF type ON accounts
WHEN NEW.type NOT IN ('loan','credit_card') AND EXISTS(SELECT 1 FROM income_deductions WHERE debt_account_id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'Unlink salary deductions before changing the debt account type.'); END;
