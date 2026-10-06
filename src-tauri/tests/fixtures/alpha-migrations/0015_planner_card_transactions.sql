-- Recorded bill payments have no installment split. Label their full cash amount clearly.
UPDATE planner_categories SET name='Recorded credit card payments', subtotal='Total recorded card payments' WHERE id='cards';
DELETE FROM planner_items
WHERE category_id='cards' AND schedule_amount IS NULL AND card_name='' AND transaction_category_id IS NULL
  AND description='Cash paid toward the bill, excluding installments above. May include purchases, interest, fees or earlier unpaid balances.'
  AND NOT EXISTS (SELECT 1 FROM planner_amounts WHERE item_id=planner_items.id)
  AND ((id='cards-0' AND name='Card 1') OR (id='cards-1' AND name='Card 2')
    OR (id='cards-2' AND name='Card 3') OR (id='cards-3' AND name='Card 4'));
