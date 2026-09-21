-- Unknown rates on existing plans remain unset, distinct from an explicit 0%.
ALTER TABLE installments ADD COLUMN interest_rate_bps INTEGER
    CHECK (interest_rate_bps IS NULL OR (typeof(interest_rate_bps) = 'integer' AND interest_rate_bps >= 0));
