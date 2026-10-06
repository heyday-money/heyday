-- Shared account identity, with optional details constrained by account type.
ALTER TABLE accounts ADD COLUMN institution TEXT;
ALTER TABLE accounts ADD COLUMN last_four TEXT CHECK (
    last_four IS NULL OR (type IN ('bank', 'credit_card') AND length(last_four) = 4 AND last_four NOT GLOB '*[^0-9]*')
);
ALTER TABLE accounts ADD COLUMN notes TEXT;
ALTER TABLE accounts ADD COLUMN credit_limit INTEGER CHECK (
    credit_limit IS NULL OR (type = 'credit_card' AND credit_limit >= 0)
);
ALTER TABLE accounts ADD COLUMN statement_day INTEGER CHECK (
    statement_day IS NULL OR (type = 'credit_card' AND statement_day BETWEEN 1 AND 31)
);
ALTER TABLE accounts ADD COLUMN payment_due_day INTEGER CHECK (
    payment_due_day IS NULL OR (type IN ('credit_card', 'loan') AND payment_due_day BETWEEN 1 AND 31)
);
-- Annual percentage rate in hundredths of one percent: 1250 = 12.50%.
ALTER TABLE accounts ADD COLUMN interest_rate_bps INTEGER CHECK (
    interest_rate_bps IS NULL OR (type IN ('credit_card', 'loan') AND interest_rate_bps BETWEEN 0 AND 10000)
);
