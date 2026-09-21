-- Independent THB planning figures; never mutate account balances or transactions.
CREATE TABLE planner_categories (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, subtotal TEXT NOT NULL, position INTEGER NOT NULL);
CREATE TABLE planner_items (
 id TEXT PRIMARY KEY NOT NULL,
 category_id TEXT NOT NULL REFERENCES planner_categories(id),
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
 card_name TEXT NOT NULL DEFAULT '' CHECK(length(card_name) <= 100),
 transaction_category_id TEXT REFERENCES categories(id),
 schedule_amount INTEGER CHECK(schedule_amount >= 0),
 schedule_start TEXT,
 schedule_end TEXT,
 CHECK ((schedule_amount IS NULL AND schedule_start IS NULL AND schedule_end IS NULL) OR
 (schedule_amount IS NOT NULL AND schedule_start IS NOT NULL AND schedule_end IS NOT NULL AND schedule_end >= schedule_start))
);
CREATE TABLE planner_amounts (
 item_id TEXT NOT NULL REFERENCES planner_items(id) ON DELETE CASCADE,
 month TEXT NOT NULL,
 amount INTEGER NOT NULL CHECK(amount >= 0),
 PRIMARY KEY(item_id, month)
);
CREATE TABLE planner_months (month TEXT PRIMARY KEY NOT NULL, status TEXT NOT NULL CHECK(status IN ('tracking','forecast','complete')));
CREATE TABLE planner_opening (id INTEGER PRIMARY KEY CHECK(id = 1), month TEXT NOT NULL, amount INTEGER NOT NULL);
INSERT INTO planner_categories VALUES ('income', 'Gross income', 'Total gross income', 0);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('income-0','income','Salary','Salary before tax and payroll deductions.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('income-1','income','Overtime / commissions','Overtime, commissions and variable compensation before deductions.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('income-2','income','Bonus','Bonus received in this cycle.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('income-3','income','Side income / freelance','Side income or freelance receipts before withholding tax.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('income-4','income','Other income','Other income received during this cycle.',NULL);
INSERT INTO planner_categories VALUES ('deductions', 'Income deductions', 'Total deductions', 1);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('deductions-0','deductions','Withholding tax','Tax withheld from salary or other income. Enter your amount; no tax rate is assumed.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('deductions-1','deductions','Social security','Employee social security contribution deducted from income.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('deductions-2','deductions','Provident fund','Employee provident fund contribution deducted from salary.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('deductions-3','deductions','Payroll loan payments','Loan payments deducted through payroll. Do not enter these again under debt payments.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('deductions-4','deductions','Other deductions','Other amounts deducted before income reaches you.',NULL);
INSERT INTO planner_categories VALUES ('debt', 'Debt payments', 'Total debt payments', 2);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('debt-0','debt','Mortgage / home loan','Payment including principal and interest, not outstanding debt.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('debt-1','debt','Vehicle loan / hire purchase','Vehicle loan payment including principal and interest.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('debt-2','debt','Personal loan 1','Rename to identify the lender. Enter the payment, not the balance.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('debt-3','debt','Personal loan 2','Monthly payment for another personal loan.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('debt-4','debt','Other debt','Other payments not already deducted through payroll.',NULL);
INSERT INTO planner_categories VALUES ('installments', 'Credit card installments', 'Total card installments', 3);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('installments-0','installments','Card / installment 1','Enter the card and purchase name. Scheduled payments include the first payment cycle.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('installments-1','installments','Card / installment 2','Enter the card and purchase name. Scheduled payments include the first payment cycle.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('installments-2','installments','Card / installment 3','Enter the card and purchase name. Scheduled payments include the first payment cycle.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('installments-3','installments','Card / installment 4','Enter the card and purchase name. Scheduled payments include the first payment cycle.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('installments-4','installments','Card / installment 5','Enter the card and purchase name. Scheduled payments include the first payment cycle.',NULL);
INSERT INTO planner_categories VALUES ('cards', 'Other credit card payments', 'Total other card payments', 4);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('cards-0','cards','Card 1','Cash paid toward the bill, excluding installments above. May include purchases, interest, fees or earlier unpaid balances.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('cards-1','cards','Card 2','Cash paid toward the bill, excluding installments above. May include purchases, interest, fees or earlier unpaid balances.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('cards-2','cards','Card 3','Cash paid toward the bill, excluding installments above. May include purchases, interest, fees or earlier unpaid balances.',NULL);
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('cards-3','cards','Card 4','Cash paid toward the bill, excluding installments above. May include purchases, interest, fees or earlier unpaid balances.',NULL);
INSERT INTO planner_categories VALUES ('expenses', 'General expenses', 'Total general expenses', 5);
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-0','Water','water');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-0','expenses','Water','Water bill paid in this cycle.',(SELECT id FROM categories WHERE name_key='water'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-1','Electricity','electricity');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-1','expenses','Electricity','Electricity bill paid in this cycle.',(SELECT id FROM categories WHERE name_key='electricity'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-2','Telephone','telephone');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-2','expenses','Telephone','Mobile or telephone bill.',(SELECT id FROM categories WHERE name_key='telephone'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-3','Internet','internet');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-3','expenses','Internet','Home or other internet service.',(SELECT id FROM categories WHERE name_key='internet'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-4','Food / household supplies','food / household supplies');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-4','expenses','Food / household supplies','Food, groceries and household supplies.',(SELECT id FROM categories WHERE name_key='food / household supplies'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-5','Transport / fuel','transport / fuel');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-5','expenses','Transport / fuel','Transport, fuel, parking and related costs.',(SELECT id FROM categories WHERE name_key='transport / fuel'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-6','Insurance','insurance');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-6','expenses','Insurance','Insurance premiums paid in this cycle.',(SELECT id FROM categories WHERE name_key='insurance'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-7','Family / dependents','family / dependents');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-7','expenses','Family / dependents','Financial support for family or dependents.',(SELECT id FROM categories WHERE name_key='family / dependents'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-8','Memberships / subscriptions','memberships / subscriptions');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-8','expenses','Memberships / subscriptions','Membership and subscription payments.',(SELECT id FROM categories WHERE name_key='memberships / subscriptions'));
INSERT OR IGNORE INTO categories(id,name,name_key) VALUES ('planner-expense-9','Medical / healthcare','medical / healthcare');
INSERT INTO planner_items(id,category_id,name,description,transaction_category_id) VALUES ('expenses-9','expenses','Medical / healthcare','Medical and healthcare payments.',(SELECT id FROM categories WHERE name_key='medical / healthcare'));
