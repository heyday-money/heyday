-- Loan payment estimates do not change balances or record transactions.
ALTER TABLE accounts ADD COLUMN monthly_installment INTEGER CHECK (
    monthly_installment IS NULL OR (
        typeof(monthly_installment) = 'integer'
        AND type = 'loan' AND monthly_installment >= 0
    )
);
