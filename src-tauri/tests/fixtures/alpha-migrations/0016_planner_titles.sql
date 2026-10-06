-- Presentation-only labels; stable IDs and all financial data remain unchanged.
UPDATE planner_categories SET name='Gross Income', subtotal='Total Gross Income' WHERE id='income';
UPDATE planner_categories SET name='Income Deductions', subtotal='Total Deductions' WHERE id='deductions';
UPDATE planner_categories SET name='Debt Payments', subtotal='Total Debt Payments' WHERE id='debt';
UPDATE planner_categories SET name='Card Installments', subtotal='Total Card Installments' WHERE id='installments';
UPDATE planner_categories SET name='Credit Cards', subtotal='Total Card Payments' WHERE id='cards';
UPDATE planner_categories SET name='General Expenses', subtotal='Total General Expenses' WHERE id='expenses';
