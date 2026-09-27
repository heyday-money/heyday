CREATE TABLE card_statements (
 id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id),
 start_date TEXT NOT NULL, end_date TEXT NOT NULL, due_date TEXT NOT NULL,
 amount INTEGER NOT NULL CHECK(amount >= 0), minimum INTEGER NOT NULL CHECK(minimum >= 0 AND minimum <= amount),
 needs_review INTEGER NOT NULL DEFAULT 0 CHECK(needs_review IN (0,1)),
 CHECK(start_date <= end_date AND end_date < due_date)
);
CREATE INDEX card_statement_dates ON card_statements(account_id,start_date,end_date);
CREATE TABLE card_statement_entries (
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE,
 transaction_id TEXT NOT NULL, PRIMARY KEY(statement_id,transaction_id)
);
CREATE TABLE card_payment_plans (
 statement_id TEXT PRIMARY KEY REFERENCES card_statements(id) ON DELETE CASCADE,
 account_id TEXT NOT NULL REFERENCES accounts(id), date TEXT NOT NULL,
 mode TEXT NOT NULL CHECK(mode IN ('full','minimum','custom')),
 target INTEGER NOT NULL CHECK(target >= 0)
);
CREATE TABLE card_payment_allocations (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE
);
-- Explicit inclusion of a scheduled occurrence, never inferred from a payment.
CREATE TABLE card_statement_installments (
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE,
 installment_id TEXT NOT NULL REFERENCES installments(id) ON DELETE CASCADE,
 date TEXT NOT NULL, PRIMARY KEY(installment_id,date)
);
CREATE TRIGGER card_statement_transaction_deleted BEFORE DELETE ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE id IN
 (SELECT statement_id FROM card_statement_entries WHERE transaction_id=OLD.id);
END;
CREATE TRIGGER card_statement_backdated_transaction AFTER INSERT ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE (account_id=NEW.account_id OR account_id=NEW.destination_account_id)
 AND NEW.date BETWEEN start_date AND end_date;
END;
