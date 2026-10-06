-- Existing loans remain unclassified; preserve balances and references.
ALTER TABLE accounts ADD COLUMN loan_type TEXT
    CHECK (loan_type IS NULL OR (type = 'loan' AND loan_type IN (
        'mortgage', 'auto_loan', 'student_loan', 'personal_loan', 'medical_debt', 'other_debt'
    )));
