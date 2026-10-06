-- Beta v1 baseline. Alpha databases use a separate storage generation.
-- Future releases append migrations; never change this file after beta ships.

CREATE TABLE "accounts" (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    type TEXT NOT NULL CHECK (type IN ('cash', 'bank', 'wallet', 'credit_card', 'loan', 'investment')),
    opening_balance INTEGER NOT NULL DEFAULT 0,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
, institution TEXT, notes TEXT, credit_limit INTEGER CHECK (
    credit_limit IS NULL OR (type = 'credit_card' AND credit_limit >= 0)
), statement_day INTEGER CHECK (
    statement_day IS NULL OR (type = 'credit_card' AND statement_day BETWEEN 1 AND 31)
), payment_due_day INTEGER CHECK (
    payment_due_day IS NULL OR (type IN ('credit_card', 'loan') AND payment_due_day BETWEEN 1 AND 31)
), current_balance INTEGER NOT NULL DEFAULT 0, loan_type TEXT
    CHECK (loan_type IS NULL OR (type = 'loan' AND loan_type IN (
        'mortgage', 'auto_loan', 'student_loan', 'personal_loan', 'medical_debt', 'other_debt'
    ))), last_four TEXT CHECK (
    last_four IS NULL OR (length(last_four) = 4 AND last_four NOT GLOB '*[^0-9]*')
), interest_rate_ten_thousandths INTEGER CHECK (
    interest_rate_ten_thousandths IS NULL OR (
        typeof(interest_rate_ten_thousandths) = 'integer'
        AND type IN ('credit_card', 'loan')
        AND interest_rate_ten_thousandths BETWEEN 0 AND 1000000
    )
), monthly_installment INTEGER CHECK (
    monthly_installment IS NULL OR (
        typeof(monthly_installment) = 'integer'
        AND type = 'loan' AND monthly_installment >= 0
    )
), initial_loan_amount INTEGER CHECK (
    initial_loan_amount IS NULL OR (
        type = 'loan' AND typeof(initial_loan_amount) = 'integer'
        AND initial_loan_amount >= 0
    )
), institution_id TEXT REFERENCES institutions(id));

CREATE TABLE card_limit_groups (
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 name_key TEXT NOT NULL UNIQUE,
 credit_limit INTEGER NOT NULL CHECK(credit_limit >= 0)
);

CREATE TABLE card_limit_members (
 account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE RESTRICT,
 group_id TEXT NOT NULL REFERENCES card_limit_groups(id) ON DELETE CASCADE
);

CREATE TABLE card_payment_allocations (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE
);

CREATE TABLE card_payment_plans (
 statement_id TEXT PRIMARY KEY REFERENCES card_statements(id) ON DELETE CASCADE,
 account_id TEXT NOT NULL REFERENCES accounts(id), date TEXT NOT NULL,
 mode TEXT NOT NULL CHECK(mode IN ('full','minimum','custom')),
 target INTEGER NOT NULL CHECK(target >= 0)
);

CREATE TABLE card_statement_entries (
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE,
 transaction_id TEXT NOT NULL, PRIMARY KEY(statement_id,transaction_id)
);

CREATE TABLE card_statement_installments (
 statement_id TEXT NOT NULL REFERENCES card_statements(id) ON DELETE CASCADE,
 installment_id TEXT NOT NULL REFERENCES installments(id) ON DELETE CASCADE,
 date TEXT NOT NULL, PRIMARY KEY(installment_id,date)
);

CREATE TABLE card_statements (
 id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id),
 start_date TEXT NOT NULL, end_date TEXT NOT NULL, due_date TEXT NOT NULL,
 amount INTEGER NOT NULL CHECK(amount >= 0), minimum INTEGER NOT NULL CHECK(minimum >= 0 AND minimum <= amount),
 needs_review INTEGER NOT NULL DEFAULT 0 CHECK(needs_review IN (0,1)),
 CHECK(start_date <= end_date AND end_date < due_date)
);

CREATE TABLE categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))
, icon TEXT NOT NULL DEFAULT 'tag');

CREATE TABLE income_deductions (
    id TEXT PRIMARY KEY NOT NULL,
    income_id TEXT NOT NULL REFERENCES incomes(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 2000),
    amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount >= 0)
, debt_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT);

CREATE TABLE incomes (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    destination_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    type TEXT NOT NULL CHECK (type IN ('salary', 'variable', 'investment', 'other')),
    estimated_amount INTEGER NOT NULL CHECK (estimated_amount >= 0),
    recurrence_frequency TEXT NOT NULL DEFAULT 'monthly' CHECK (recurrence_frequency = 'monthly'),
    recurrence_day_of_month INTEGER NOT NULL CHECK (recurrence_day_of_month BETWEEN 1 AND 31),
    is_auto_create_transaction INTEGER NOT NULL DEFAULT 0 CHECK (is_auto_create_transaction IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE installments (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    debt_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    monthly_amount INTEGER NOT NULL CHECK (typeof(monthly_amount) = 'integer' AND monthly_amount > 0),
    installment_count INTEGER NOT NULL CHECK (installment_count BETWEEN 1 AND 600),
    first_due_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, purchase_kind TEXT NOT NULL DEFAULT 'existing_purchase'
    CHECK (purchase_kind IN ('existing_purchase', 'new_purchase')), purchase_transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL, interest_rate_millis INTEGER CHECK (
    interest_rate_millis IS NULL OR (typeof(interest_rate_millis) = 'integer' AND interest_rate_millis >= 0)
),
    CHECK (account_id <> debt_account_id)
);

CREATE TABLE institutions (
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 name_key TEXT NOT NULL UNIQUE,
 short_name TEXT,
 bank_code TEXT,
 swift_code TEXT,
 logo TEXT,
 is_archived INTEGER NOT NULL DEFAULT 0 CHECK(is_archived IN (0,1))
, logo_asset_id TEXT REFERENCES logo_assets(id), logo_mode TEXT NOT NULL DEFAULT 'default'
 CHECK(logo_mode IN ('default','custom','none') AND ((logo_mode='custom')=(logo_asset_id IS NOT NULL))));

CREATE TABLE loan_contracts (
 id TEXT PRIMARY KEY,
 account_id TEXT NOT NULL REFERENCES loan_facilities(account_id),
 name TEXT NOT NULL,
 principal INTEGER NOT NULL CHECK(typeof(principal)='integer' AND principal>0),
 remaining_principal INTEGER NOT NULL CHECK(typeof(remaining_principal)='integer' AND remaining_principal BETWEEN 0 AND principal),
 borrowing_date TEXT NOT NULL,
 receiving_account_id TEXT NOT NULL REFERENCES accounts(id),
 payment_account_id TEXT NOT NULL REFERENCES accounts(id),
 interest_rate INTEGER NOT NULL CHECK(interest_rate BETWEEN 0 AND 1000000),
 monthly_amount INTEGER NOT NULL CHECK(typeof(monthly_amount)='integer' AND monthly_amount>0),
 installment_count INTEGER NOT NULL CHECK(installment_count BETWEEN 1 AND 600),
 first_due_date TEXT NOT NULL,
 schedule_end TEXT,
 notes TEXT NOT NULL DEFAULT '',
 borrowing_kind TEXT NOT NULL CHECK(borrowing_kind IN ('new','existing')),
 borrowing_transaction_id TEXT UNIQUE REFERENCES transactions(id) ON DELETE SET NULL,
 needs_review INTEGER NOT NULL DEFAULT 0 CHECK(needs_review IN (0,1))
);

CREATE TABLE loan_facilities (
 account_id TEXT PRIMARY KEY REFERENCES accounts(id),
 credit_limit INTEGER NOT NULL CHECK(typeof(credit_limit)='integer' AND credit_limit>=0)
);

CREATE TABLE "loan_payment_parts" (
 transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
 payment_id TEXT NOT NULL,
 contract_id TEXT REFERENCES loan_contracts(id),
 component TEXT NOT NULL CHECK(component IN ('principal','interest','fee')),
 amount INTEGER NOT NULL CHECK(typeof(amount)='integer' AND amount>0),
 loan_account_id TEXT REFERENCES accounts(id),
 CHECK ((contract_id IS NOT NULL AND loan_account_id IS NULL) OR
        (contract_id IS NULL AND loan_account_id IS NOT NULL)),
 UNIQUE(payment_id,component)
);

CREATE TABLE loan_payoffs (
    account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    paid_off_on TEXT NOT NULL
);

CREATE TABLE logo_assets (
 id TEXT PRIMARY KEY,
 content_hash TEXT NOT NULL UNIQUE,
 mime_type TEXT NOT NULL CHECK(mime_type='image/png'),
 width INTEGER NOT NULL CHECK(width BETWEEN 1 AND 128),
 height INTEGER NOT NULL CHECK(height BETWEEN 1 AND 128),
 data BLOB NOT NULL CHECK(length(data) BETWEEN 1 AND 102400),
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE payees (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    name_key TEXT NOT NULL UNIQUE,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1))
, logo_asset_id TEXT REFERENCES logo_assets(id));

CREATE TABLE payment_plans (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    type TEXT NOT NULL CHECK (type IN ('expense', 'repayment')),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((type = 'expense' AND destination_account_id IS NULL)
        OR (type = 'repayment' AND destination_account_id IS NOT NULL AND destination_account_id <> account_id AND category_id IS NULL))
);

CREATE TABLE planner_amounts (
 item_id TEXT NOT NULL REFERENCES planner_items(id) ON DELETE CASCADE,
 month TEXT NOT NULL,
 amount INTEGER NOT NULL CHECK(amount >= 0),
 PRIMARY KEY(item_id, month)
);

CREATE TABLE planner_categories (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, subtotal TEXT NOT NULL, position INTEGER NOT NULL);

CREATE TABLE planner_debt_amounts (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(account_id, month)
);

CREATE TABLE planner_deduction_amounts (
    deduction_id TEXT NOT NULL REFERENCES income_deductions(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(deduction_id, month)
);

CREATE TABLE planner_income_amounts (
    income_id TEXT NOT NULL REFERENCES incomes(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(income_id, month)
);

CREATE TABLE planner_installment_amounts (
    installment_id TEXT NOT NULL REFERENCES installments(id) ON DELETE CASCADE,
    month TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK(amount >= 0),
    PRIMARY KEY(installment_id, month)
);

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

CREATE TABLE planner_months (month TEXT PRIMARY KEY NOT NULL, status TEXT NOT NULL CHECK(status IN ('tracking','forecast','complete')));

CREATE TABLE planner_opening (id INTEGER PRIMARY KEY CHECK(id = 1), month TEXT NOT NULL, amount INTEGER NOT NULL);

CREATE TABLE reconciliation_entries (
    reconciliation_id INTEGER NOT NULL REFERENCES reconciliations(id) ON DELETE CASCADE,
    transaction_id TEXT NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    type TEXT NOT NULL,
    balance_change INTEGER NOT NULL CHECK (typeof(balance_change) = 'integer'),
    PRIMARY KEY (reconciliation_id, transaction_id)
);

CREATE TABLE reconciliations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    completed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    confirmed_balance INTEGER NOT NULL CHECK (typeof(confirmed_balance) = 'integer'),
    opening_balance INTEGER NOT NULL CHECK (typeof(opening_balance) = 'integer'),
    needs_review INTEGER NOT NULL DEFAULT 0 CHECK (needs_review IN (0, 1))
);

CREATE TABLE selective_defaults (
    account_id TEXT NOT NULL REFERENCES accounts(id),
    start_month TEXT NOT NULL CHECK(length(start_month)=7 AND start_month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' AND substr(start_month,1,4)>='0001' AND substr(start_month,6,2) BETWEEN '01' AND '12'),
    end_month TEXT CHECK(end_month IS NULL OR (length(end_month)=7 AND end_month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' AND substr(end_month,1,4)>='0001' AND substr(end_month,6,2) BETWEEN '01' AND '12' AND end_month>=start_month)),
    PRIMARY KEY(account_id,start_month)
);

CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    currency TEXT CHECK (currency IS NULL OR (length(currency) = 3 AND currency = upper(currency))),
    period_start_day INTEGER NOT NULL DEFAULT 1 CHECK (period_start_day BETWEEN 1 AND 31)
);

CREATE TABLE subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    frequency TEXT NOT NULL CHECK (frequency IN ('monthly', 'yearly')),
    first_billing_date TEXT NOT NULL,
    end_date TEXT,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, logo_asset_id TEXT REFERENCES logo_assets(id), managed_via TEXT
 CHECK(managed_via IS NULL OR managed_via IN ('apple_app_store','google_play','website','in_app','other')), management_url TEXT
 CHECK(management_url IS NULL OR length(management_url) BETWEEN 1 AND 2048),
    CHECK (end_date IS NULL OR end_date >= first_billing_date)
);

CREATE TABLE transaction_verifications (
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'uncleared' CHECK (status IN ('uncleared', 'cleared', 'reconciled')),
    reconciliation_id INTEGER REFERENCES reconciliations(id) ON DELETE RESTRICT,
    PRIMARY KEY (transaction_id, account_id),
    CHECK ((status = 'reconciled' AND reconciliation_id IS NOT NULL)
        OR (status <> 'reconciled' AND reconciliation_id IS NULL))
);

CREATE TABLE "transactions" (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('income', 'expense', 'transfer', 'repayment')),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    amount INTEGER NOT NULL CHECK (typeof(amount) = 'integer' AND amount > 0),
    date TEXT NOT NULL,
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 0 AND 200),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, payee_id TEXT REFERENCES payees(id) ON DELETE RESTRICT
    CHECK (payee_id IS NULL OR type = 'expense'), category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT
    CHECK (category_id IS NULL OR type = 'expense'), income_source_id TEXT REFERENCES incomes(id),
    CHECK ((type IN ('transfer', 'repayment') AND destination_account_id IS NOT NULL AND destination_account_id <> account_id)
        OR (type IN ('income', 'expense') AND destination_account_id IS NULL))
);

CREATE INDEX accounts_institution_id ON accounts(institution_id);

CREATE INDEX card_limit_members_group ON card_limit_members(group_id);

CREATE INDEX card_statement_dates ON card_statements(account_id,start_date,end_date);

CREATE INDEX income_deductions_debt_idx ON income_deductions(debt_account_id);

CREATE INDEX income_deductions_income_idx ON income_deductions(income_id);

CREATE INDEX incomes_destination_account_idx ON incomes(destination_account_id);

CREATE INDEX installments_debt_account_idx ON installments(debt_account_id);

CREATE INDEX loan_contract_account ON loan_contracts(account_id);

CREATE INDEX loan_payment_group ON loan_payment_parts(payment_id);

CREATE UNIQUE INDEX one_open_selective_default ON selective_defaults(account_id) WHERE end_month IS NULL;

CREATE INDEX payment_plans_date_idx ON payment_plans(date);

CREATE INDEX reconciliation_entries_transaction_idx ON reconciliation_entries(transaction_id);

CREATE INDEX reconciliations_account_idx ON reconciliations(account_id, id);

CREATE INDEX subscriptions_account_idx ON subscriptions(account_id);

CREATE INDEX transaction_verifications_account_idx ON transaction_verifications(account_id, status);

CREATE INDEX transactions_account_idx ON transactions(account_id);

CREATE INDEX transactions_category_idx ON transactions(category_id);

CREATE INDEX transactions_date_idx ON transactions(date DESC, created_at DESC);

CREATE INDEX transactions_destination_idx ON transactions(destination_account_id);

CREATE INDEX transactions_income_source ON transactions(income_source_id);

CREATE INDEX transactions_payee_idx ON transactions(payee_id);

-- Initial settings and bundled reference data only; no personal records.

INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-0', 'Water', 'water', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-1', 'Electricity', 'electricity', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-2', 'Telephone', 'telephone', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-3', 'Internet', 'internet', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-4', 'Food / household supplies', 'food / household supplies', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-5', 'Transport / fuel', 'transport / fuel', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-6', 'Insurance', 'insurance', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-7', 'Family / dependents', 'family / dependents', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-8', 'Memberships / subscriptions', 'memberships / subscriptions', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('planner-expense-9', 'Medical / healthcare', 'medical / healthcare', 0, 'tag');
INSERT INTO "categories" ("id", "name", "name_key", "is_archived", "icon") VALUES ('default-fee-interest', 'Fee/Interest', 'fee/interest', 0, 'tag');

INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-002', 'Bangkok Bank Public Company Limited', 'bangkok bank public company limited', 'BBL', '002', 'BKKBTHBK', 'bbl.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-004', 'Kasikornbank Public Company Limited', 'kasikornbank public company limited', 'KBANK', '004', 'KASITHBK', 'kbank.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-006', 'Krungthai Bank Public Company Limited', 'krungthai bank public company limited', 'KTB', '006', 'KRUTTHBK', 'ktb.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-014', 'The Siam Commercial Bank Public Company Limited', 'the siam commercial bank public company limited', 'SCB', '014', 'SICOTHBK', 'scb.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-024', 'United Overseas Bank (Thai) Public Company Limited', 'united overseas bank (thai) public company limited', 'UOB', '024', 'UOVBTHBK', 'uob.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-025', 'Bank of Ayudhya Public Company Limited', 'bank of ayudhya public company limited', 'BAY', '025', 'AYUDTHBK', 'bay.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-033', 'Government Housing Bank', 'government housing bank', 'GHB', '033', 'GHBATHBK', 'ghb.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-067', 'TISCO Bank Public Company Limited', 'tisco bank public company limited', 'TISCO', '067', 'TISCTHBK', 'tisco.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-069', 'Kiatnakin Phatra Bank Public Company Limited', 'kiatnakin phatra bank public company limited', 'KKP', '069', 'KKPBTHBK', 'kkp.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-011', 'TMBThanachart Bank Public Company Limited', 'tmbthanachart bank public company limited', 'TTB', '011', NULL, 'ttb.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-020', 'Standard Chartered Bank (Thai) Public Company Limited', 'standard chartered bank (thai) public company limited', 'SCBT', '020', NULL, 'standard-chartred.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-030', 'Government Savings Bank', 'government savings bank', 'GSB', '030', NULL, 'gsb.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-031', 'The Hongkong and Shanghai Banking Corporation Limited', 'the hongkong and shanghai banking corporation limited', 'HSBC', '031', NULL, 'hsbc.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-034', 'Bank for Agriculture and Agricultural Cooperatives', 'bank for agriculture and agricultural cooperatives', 'BAAC', '034', NULL, 'baac.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-065', 'Thanachart Bank Public Company Limited (legacy)', 'thanachart bank public company limited (legacy)', 'TBANK', '065', NULL, 'thanachart.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-066', 'Islamic Bank of Thailand', 'islamic bank of thailand', 'IBANK', '066', NULL, 'ibank.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-070', 'Industrial and Commercial Bank of China (Thai) Public Company Limited', 'industrial and commercial bank of china (thai) public company limited', 'ICBC', '070', NULL, 'icbc.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('bank-073', 'Land and Houses Bank Public Company Limited', 'land and houses bank public company limited', 'LH Bank', '073', NULL, 'lh.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('wallet-truemoney', 'TrueMoney', 'truemoney', 'TrueMoney', NULL, NULL, 'truemoney.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('wallet-line-pay', 'LINE Pay', 'line pay', 'LINE Pay', NULL, NULL, 'line-pay.svg', 0, NULL, 'default');
INSERT INTO "institutions" ("id", "name", "name_key", "short_name", "bank_code", "swift_code", "logo", "is_archived", "logo_asset_id", "logo_mode") VALUES ('wallet-grab-pay', 'GrabPay', 'grabpay', 'GrabPay', NULL, NULL, 'grab-pay.svg', 0, NULL, 'default');

INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('income', 'Gross Income', 'Total Gross Income', 0);
INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('deductions', 'Income Deductions', 'Total Deductions', 1);
INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('debt', 'Debt Payments', 'Total Debt Payments', 2);
INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('installments', 'Card Installments', 'Total Card Installments', 3);
INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('cards', 'Credit Cards', 'Total Card Payments', 4);
INSERT INTO "planner_categories" ("id", "name", "subtotal", "position") VALUES ('expenses', 'General Expenses', 'Total General Expenses', 5);

INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-0', 'expenses', 'Water', 'Water bill paid in this cycle.', '', 'planner-expense-0', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-1', 'expenses', 'Electricity', 'Electricity bill paid in this cycle.', '', 'planner-expense-1', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-2', 'expenses', 'Telephone', 'Mobile or telephone bill.', '', 'planner-expense-2', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-3', 'expenses', 'Internet', 'Home or other internet service.', '', 'planner-expense-3', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-4', 'expenses', 'Food / household supplies', 'Food, groceries and household supplies.', '', 'planner-expense-4', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-5', 'expenses', 'Transport / fuel', 'Transport, fuel, parking and related costs.', '', 'planner-expense-5', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-6', 'expenses', 'Insurance', 'Insurance premiums paid in this cycle.', '', 'planner-expense-6', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-7', 'expenses', 'Family / dependents', 'Financial support for family or dependents.', '', 'planner-expense-7', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-8', 'expenses', 'Memberships / subscriptions', 'Membership and subscription payments.', '', 'planner-expense-8', NULL, NULL, NULL);
INSERT INTO "planner_items" ("id", "category_id", "name", "description", "card_name", "transaction_category_id", "schedule_amount", "schedule_start", "schedule_end") VALUES ('expenses-9', 'expenses', 'Medical / healthcare', 'Medical and healthcare payments.', '', 'planner-expense-9', NULL, NULL, NULL);

INSERT INTO "settings" ("id", "currency", "period_start_day") VALUES (1, NULL, 1);

CREATE TRIGGER card_limit_member_type BEFORE INSERT ON card_limit_members
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='credit_card')
BEGIN SELECT RAISE(ABORT,'Shared limits require credit card accounts'); END;

CREATE TRIGGER card_statement_backdated_transaction AFTER INSERT ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE (account_id=NEW.account_id OR account_id=NEW.destination_account_id)
 AND end_date>=NEW.date;
END;

CREATE TRIGGER card_statement_installment_edited AFTER UPDATE ON installments BEGIN
 UPDATE card_statements SET needs_review=1 WHERE id IN (SELECT statement_id FROM card_statement_installments WHERE installment_id=OLD.id);
END;

CREATE TRIGGER card_statement_installment_removed BEFORE DELETE ON installments BEGIN
 UPDATE card_statements SET needs_review=1 WHERE id IN (SELECT statement_id FROM card_statement_installments WHERE installment_id=OLD.id);
END;

CREATE TRIGGER card_statement_transaction_deleted BEFORE DELETE ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE (account_id=OLD.account_id OR account_id=OLD.destination_account_id) AND end_date>=OLD.date;
END;

CREATE TRIGGER deduction_account_type_update BEFORE UPDATE OF type ON accounts
WHEN NEW.type NOT IN ('loan','credit_card') AND EXISTS(SELECT 1 FROM income_deductions WHERE debt_account_id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'Unlink salary deductions before changing the debt account type.'); END;

CREATE TRIGGER deduction_debt_insert BEFORE INSERT ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;

CREATE TRIGGER deduction_debt_update BEFORE UPDATE OF debt_account_id ON income_deductions
WHEN NEW.debt_account_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.debt_account_id AND type IN ('loan','credit_card'))
BEGIN SELECT RAISE(ABORT, 'Deductions must link to a debt account.'); END;

CREATE TRIGGER income_deductions_salary_insert BEFORE INSERT ON income_deductions
WHEN NOT EXISTS(SELECT 1 FROM incomes WHERE id=NEW.income_id AND type='salary')
BEGIN SELECT RAISE(ABORT, 'Deductions require a salary source.'); END;

CREATE TRIGGER income_deductions_salary_update BEFORE UPDATE OF income_id ON income_deductions
WHEN NOT EXISTS(SELECT 1 FROM incomes WHERE id=NEW.income_id AND type='salary')
BEGIN SELECT RAISE(ABORT, 'Deductions require a salary source.'); END;

CREATE TRIGGER income_salary_type_update BEFORE UPDATE OF type ON incomes
WHEN NEW.type <> 'salary' AND EXISTS(SELECT 1 FROM income_deductions WHERE income_id=NEW.id)
BEGIN SELECT RAISE(ABORT, 'Remove salary deductions before changing income type.'); END;

CREATE TRIGGER institution_rename AFTER UPDATE OF name ON institutions BEGIN
 UPDATE accounts SET institution=NEW.name WHERE institution_id=NEW.id;
END;

CREATE TRIGGER loan_facility_type BEFORE INSERT ON loan_facilities
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='loan' AND loan_type='personal_loan')
BEGIN SELECT RAISE(ABORT,'Revolving facilities require a Personal Loan account.'); END;

CREATE TRIGGER preserve_loan_facility_type BEFORE UPDATE OF type,loan_type ON accounts
WHEN EXISTS(SELECT 1 FROM loan_facilities WHERE account_id=OLD.id) AND (NEW.type!='loan' OR NEW.loan_type IS NOT 'personal_loan')
BEGIN SELECT RAISE(ABORT,'A revolving facility must remain a Personal Loan.'); END;

CREATE TRIGGER reopen_paid_off_loan
AFTER UPDATE OF current_balance ON accounts
WHEN NEW.current_balance != 0 AND EXISTS (SELECT 1 FROM loan_payoffs WHERE account_id=NEW.id)
BEGIN
    UPDATE accounts SET is_archived=0, updated_at=CURRENT_TIMESTAMP WHERE id=NEW.id;
    DELETE FROM loan_payoffs WHERE account_id=NEW.id;
END;

CREATE TRIGGER transaction_verification_insert AFTER INSERT ON transactions
BEGIN
    INSERT INTO transaction_verifications (transaction_id, account_id) VALUES (NEW.id, NEW.account_id);
    INSERT INTO transaction_verifications (transaction_id, account_id)
    SELECT NEW.id, NEW.destination_account_id WHERE NEW.destination_account_id IS NOT NULL;
END;

CREATE TRIGGER transaction_verification_owner_insert BEFORE INSERT ON transaction_verifications
WHEN NOT EXISTS (SELECT 1 FROM transactions WHERE id = NEW.transaction_id AND (account_id = NEW.account_id OR destination_account_id = NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'Verification must belong to a participating account.'); END;

CREATE TRIGGER transaction_verification_owner_update BEFORE UPDATE ON transaction_verifications
WHEN NOT EXISTS (SELECT 1 FROM transactions WHERE id = NEW.transaction_id AND (account_id = NEW.account_id OR destination_account_id = NEW.account_id))
    OR (NEW.reconciliation_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM reconciliations WHERE id = NEW.reconciliation_id AND account_id = NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'Verification and reconciliation must belong to the same account.'); END;
