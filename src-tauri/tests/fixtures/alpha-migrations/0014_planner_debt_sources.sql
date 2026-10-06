-- Overrides are local planning amounts, never ledger payments.
CREATE TABLE planner_debt_amounts (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(account_id, month)
);
CREATE TABLE planner_installment_amounts (
    installment_id TEXT NOT NULL REFERENCES installments(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(installment_id, month)
);
-- Preserve all edited placeholders, schedules and explicit amounts, including zero.
DELETE FROM planner_items
WHERE schedule_amount IS NULL AND card_name = '' AND transaction_category_id IS NULL
  AND NOT EXISTS (SELECT 1 FROM planner_amounts WHERE item_id = planner_items.id)
  AND (
    (category_id = 'debt' AND (
      (id = 'debt-0' AND name = 'Mortgage / home loan' AND description = 'Payment including principal and interest, not outstanding debt.') OR
      (id = 'debt-1' AND name = 'Vehicle loan / hire purchase' AND description = 'Vehicle loan payment including principal and interest.') OR
      (id = 'debt-2' AND name = 'Personal loan 1' AND description = 'Rename to identify the lender. Enter the payment, not the balance.') OR
      (id = 'debt-3' AND name = 'Personal loan 2' AND description = 'Monthly payment for another personal loan.') OR
      (id = 'debt-4' AND name = 'Other debt' AND description = 'Other payments not already deducted through payroll.')
    )) OR
    (category_id = 'installments' AND description = 'Enter the card and purchase name. Scheduled payments include the first payment cycle.' AND (
      (id = 'installments-0' AND name = 'Card / installment 1') OR
      (id = 'installments-1' AND name = 'Card / installment 2') OR
      (id = 'installments-2' AND name = 'Card / installment 3') OR
      (id = 'installments-3' AND name = 'Card / installment 4') OR
      (id = 'installments-4' AND name = 'Card / installment 5')
    ))
  );
