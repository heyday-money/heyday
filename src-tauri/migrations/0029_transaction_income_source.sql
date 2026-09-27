ALTER TABLE transactions ADD COLUMN income_source_id TEXT REFERENCES incomes(id);
CREATE INDEX transactions_income_source ON transactions(income_source_id);
