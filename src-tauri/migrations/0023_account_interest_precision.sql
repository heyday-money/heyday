-- Account rates use ten-thousandths of a percentage point (1.1957% = 11957).
-- Installment reference rates retain their existing thousandths precision.
ALTER TABLE accounts ADD COLUMN interest_rate_ten_thousandths INTEGER CHECK (
    interest_rate_ten_thousandths IS NULL OR (
        typeof(interest_rate_ten_thousandths) = 'integer'
        AND type IN ('credit_card', 'loan')
        AND interest_rate_ten_thousandths BETWEEN 0 AND 1000000
    )
);
UPDATE accounts SET interest_rate_ten_thousandths = interest_rate_millis * 10;
ALTER TABLE accounts DROP COLUMN interest_rate_millis;
