-- Reuse a matching category without changing its name, icon or archive state.
INSERT INTO categories (id, name, name_key, icon)
VALUES (lower(hex(randomblob(16))), 'Fee/Interest', 'fee/interest', 'tag')
ON CONFLICT(name_key) DO NOTHING;

-- Identify linked components by their relationship, never by description text.
-- Preserve categories the user has already assigned.
UPDATE transactions
SET category_id = (SELECT id FROM categories WHERE name_key = 'fee/interest')
WHERE type = 'expense' AND category_id IS NULL
  AND id IN (SELECT transaction_id FROM loan_payment_parts
             WHERE component IN ('interest', 'fee'));
