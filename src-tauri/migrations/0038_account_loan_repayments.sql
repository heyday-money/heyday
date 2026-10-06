-- Preserve contract payments while allowing a split against a standalone loan.
CREATE TABLE loan_payment_parts_new (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 payment_id TEXT NOT NULL,
 contract_id TEXT REFERENCES loan_contracts(id),
 component TEXT NOT NULL CHECK(component IN ('principal','interest','fee')),
 amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount>0),
 loan_account_id TEXT REFERENCES accounts(id),
 CHECK ((contract_id IS NOT NULL AND loan_account_id IS NULL) OR
        (contract_id IS NULL AND loan_account_id IS NOT NULL)),
 UNIQUE(payment_id,component)
);
INSERT INTO loan_payment_parts_new(transaction_id,payment_id,contract_id,component,amount)
SELECT transaction_id,payment_id,contract_id,component,amount FROM loan_payment_parts;
DROP TABLE loan_payment_parts;
ALTER TABLE loan_payment_parts_new RENAME TO loan_payment_parts;
CREATE INDEX loan_payment_group ON loan_payment_parts(payment_id);
