CREATE TABLE income_deductions (
    id TEXT PRIMARY KEY NOT NULL,
    income_id TEXT NOT NULL REFERENCES incomes(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount >= 0)
);
CREATE INDEX income_deductions_income_idx ON income_deductions(income_id);
CREATE TRIGGER income_deductions_salary_insert BEFORE INSERT ON income_deductions
WHEN NOT EXISTS(SELECT 1 FROM incomes WHERE id=NEW.income_id AND type='salary')
BEGIN SELECT RAISE(ABORT, 'Deductions require a salary source.'); END;
CREATE TRIGGER income_deductions_salary_update BEFORE UPDATE OF income_id ON income_deductions
WHEN NOT EXISTS(SELECT 1 FROM incomes WHERE id=NEW.income_id AND type='salary')
BEGIN SELECT RAISE(ABORT, 'Deductions require a salary source.'); END;
CREATE TRIGGER income_salary_type_update BEFORE UPDATE OF type ON incomes
WHEN NEW.type <> 'salary' AND EXISTS(SELECT 1 FROM income_deductions WHERE income_id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'Remove salary deductions before changing income type.'); END;
CREATE TABLE planner_deduction_amounts (
    deduction_id TEXT NOT NULL REFERENCES income_deductions(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(deduction_id, month)
);
-- Existing entered deductions cannot be assigned to a salary automatically.
-- Retain all amounts and customizations; remove only untouched default placeholders.
DELETE FROM planner_items WHERE category_id='deductions' AND schedule_amount IS NULL
    AND card_name='' AND transaction_category_id IS NULL
    AND NOT EXISTS(SELECT 1 FROM planner_amounts WHERE item_id=planner_items.id)
    AND (
      (id='deductions-0' AND name='Withholding tax' AND description='Tax withheld from salary or other income. Enter your amount; no tax rate is assumed.') OR
      (id='deductions-1' AND name='Social security' AND description='Employee social security contribution deducted from income.') OR
      (id='deductions-2' AND name='Provident fund' AND description='Employee provident fund contribution deducted from salary.') OR
      (id='deductions-3' AND name='Payroll loan payments' AND description='Loan payments deducted through payroll. Do not enter these again under debt payments.') OR
      (id='deductions-4' AND name='Other deductions' AND description='Other amounts deducted before income reaches you.')
    );
