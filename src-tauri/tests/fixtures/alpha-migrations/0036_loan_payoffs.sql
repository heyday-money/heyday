CREATE TABLE loan_payoffs (
    account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    paid_off_on TEXT NOT NULL
);

-- A corrected/deleted payment must never leave a nonzero loan hidden as paid off.
CREATE TRIGGER reopen_paid_off_loan
AFTER UPDATE OF current_balance ON accounts
WHEN NEW.current_balance != 0 AND EXISTS (SELECT 1 FROM loan_payoffs WHERE account_id=NEW.id)
BEGIN
    UPDATE accounts SET is_archived=0, updated_at=CURRENT_TIMESTAMP WHERE id=NEW.id;
    DELETE FROM loan_payoffs WHERE account_id=NEW.id;
END;
