-- Cycle-specific overrides retain their source identity without copying income definitions.
CREATE TABLE planner_income_amounts (
    income_id TEXT NOT NULL REFERENCES incomes(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(income_id, month)
);
-- Remove only untouched placeholder rows. Preserve all user entries and customizations.
DELETE FROM planner_items
WHERE category_id = 'income' AND schedule_amount IS NULL AND card_name = ''
  AND transaction_category_id IS NULL
  AND NOT EXISTS (SELECT 1 FROM planner_amounts WHERE item_id = planner_items.id)
  AND (
    (id = 'income-0' AND name = 'Salary' AND description = 'Salary before tax and payroll deductions.') OR
    (id = 'income-1' AND name = 'Overtime / commissions' AND description = 'Overtime, commissions and variable compensation before deductions.') OR
    (id = 'income-2' AND name = 'Bonus' AND description = 'Bonus received in this cycle.') OR
    (id = 'income-3' AND name = 'Side income / freelance' AND description = 'Side income or freelance receipts before withholding tax.') OR
    (id = 'income-4' AND name = 'Other income' AND description = 'Other income received during this cycle.')
  );
