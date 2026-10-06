-- Store thousandths of a percentage point, preserving existing rates exactly.
ALTER TABLE accounts ADD COLUMN interest_rate_millis INTEGER CHECK (
    interest_rate_millis IS NULL OR (typeof(interest_rate_millis) = 'integer'
    AND type IN ('credit_card', 'loan') AND interest_rate_millis BETWEEN 0 AND 100000)
);
UPDATE accounts SET interest_rate_millis = interest_rate_bps * 10;
ALTER TABLE accounts DROP COLUMN interest_rate_bps;

ALTER TABLE installments ADD COLUMN interest_rate_millis INTEGER CHECK (
    interest_rate_millis IS NULL OR (typeof(interest_rate_millis) = 'integer' AND interest_rate_millis >= 0)
);
UPDATE installments SET interest_rate_millis = interest_rate_bps * 10;
ALTER TABLE installments DROP COLUMN interest_rate_bps;
