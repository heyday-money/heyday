-- Confirmed payroll snapshots are independent of editable salary definitions.
CREATE TABLE salary_payments (
 id TEXT PRIMARY KEY NOT NULL,
 income_id TEXT NOT NULL REFERENCES incomes(id),
 occurrence TEXT NOT NULL CHECK(length(occurrence)=7),
 date TEXT NOT NULL,
 gross INTEGER NOT NULL CHECK(typeof(gross)='integer' AND gross>=0),
 net INTEGER NOT NULL CHECK(typeof(net)='integer' AND net>=0 AND net<=gross),
 breakdown TEXT NOT NULL,
 UNIQUE(income_id, occurrence)
);
CREATE TABLE salary_payment_transactions (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 payment_id TEXT NOT NULL REFERENCES salary_payments(id) ON DELETE CASCADE,
 role TEXT NOT NULL CHECK(role IN ('net','principal'))
);
CREATE INDEX salary_payment_group ON salary_payment_transactions(payment_id);
