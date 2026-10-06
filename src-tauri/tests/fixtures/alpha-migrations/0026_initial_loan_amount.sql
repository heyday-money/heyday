-- Reference amount only: never derive it from opening or current debt.
ALTER TABLE accounts ADD COLUMN initial_loan_amount INTEGER CHECK (
    initial_loan_amount IS NULL OR (
        type = 'loan' AND typeof(initial_loan_amount) = 'integer'
        AND initial_loan_amount >= 0
    )
);
