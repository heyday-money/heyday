-- Separate facility metadata preserves existing account constraints and balances.
CREATE TABLE loan_facilities (
 account_id TEXT PRIMARY KEY REFERENCES accounts(id),
 credit_limit INTEGER NOT NULL CHECK(typeof(credit_limit)='integer' AND credit_limit>=0)
);
CREATE TABLE loan_contracts (
 id TEXT PRIMARY KEY,
 account_id TEXT NOT NULL REFERENCES loan_facilities(account_id),
 name TEXT NOT NULL,
 principal INTEGER NOT NULL CHECK(typeof(principal)='integer' AND principal>0),
 remaining_principal INTEGER NOT NULL CHECK(typeof(remaining_principal)='integer' AND remaining_principal BETWEEN 0 AND principal),
 borrowing_date TEXT NOT NULL,
 receiving_account_id TEXT NOT NULL REFERENCES accounts(id),
 payment_account_id TEXT NOT NULL REFERENCES accounts(id),
 interest_rate INTEGER NOT NULL CHECK(interest_rate BETWEEN 0 AND 1000000),
 monthly_amount INTEGER NOT NULL CHECK(typeof(monthly_amount)='integer' AND monthly_amount>0),
 installment_count INTEGER NOT NULL CHECK(installment_count BETWEEN 1 AND 600),
 first_due_date TEXT NOT NULL,
 schedule_end TEXT,
 notes TEXT NOT NULL DEFAULT '',
 borrowing_kind TEXT NOT NULL CHECK(borrowing_kind IN ('new','existing')),
 borrowing_transaction_id TEXT UNIQUE REFERENCES transactions(id) ON DELETE SET NULL,
 needs_review INTEGER NOT NULL DEFAULT 0 CHECK(needs_review IN (0,1))
);
CREATE INDEX loan_contract_account ON loan_contracts(account_id);
CREATE TABLE loan_payment_parts (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 payment_id TEXT NOT NULL,
 contract_id TEXT NOT NULL REFERENCES loan_contracts(id),
 component TEXT NOT NULL CHECK(component IN ('principal','interest','fee')),
 amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount>0),
 UNIQUE(payment_id,component)
);
CREATE INDEX loan_payment_group ON loan_payment_parts(payment_id);
CREATE TRIGGER loan_facility_type BEFORE INSERT ON loan_facilities
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='loan' AND loan_type='personal_loan')
BEGIN SELECT RAISE(ABORT,'Revolving facilities require a Personal Loan account.'); END;
CREATE TRIGGER preserve_loan_facility_type BEFORE UPDATE OF type,loan_type ON accounts
WHEN EXISTS(SELECT 1 FROM loan_facilities WHERE account_id=OLD.id) AND (NEW.type!='loan' OR NEW.loan_type IS NOT 'personal_loan')
BEGIN SELECT RAISE(ABORT,'A revolving facility must remain a Personal Loan.'); END;
