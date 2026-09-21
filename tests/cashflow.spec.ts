import { expect, test } from '@playwright/test'
import { addMonths, currentCycle, cycleLabel, cycleTotals, itemAmount, plannerItems, openingForCycle, plannerCycles, parsePlannerAmount, plannerMoney, type PlannerCardTransaction, type PlannerInstallment, type PlannerCategoryId, type PlannerData, type PlannerItem } from '../src/lib/cashflow'
function item(id: string, category_id: PlannerCategoryId): PlannerItem {
  return { id, category_id, name: id, description: '', card_name: '', transaction_category_id: null, schedule_amount: null, schedule_start: null, schedule_end: null }
}
function data(): PlannerData { return { incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], source_currency: 'THB', categories: [], items: [], amounts: [], months: [], opening: null, period_start_day: 25, expense_categories: [] } }

test('acceptance arithmetic deducts payroll once and separates card payments from cash expenses', () => {
  const input = data()
  const entries: [string, PlannerCategoryId, string][] = [['salary', 'income', '50000'], ['tax', 'deductions', '2000'], ['social', 'deductions', '750'], ['mortgage', 'debt', '12000'], ['card installments', 'installments', '3000'], ['other card', 'cards', '4000'], ['water', 'expenses', '500']]
  input.items = entries.map(([id, category]) => item(id, category))
  input.amounts = entries.map(([item_id, , amount]) => ({ item_id, month: '2026-12', amount: parsePlannerAmount(amount) }))
  input.opening = { month: '2026-12', amount: '1000000' }
  const cycles = plannerCycles(input, '2026-12')
  expect(cycles.map(c => c.month)).toEqual(['2026-12', '2027-01', '2027-02', '2027-03', '2027-04', '2027-05', '2027-06'])
  expect(cycles[0]).toMatchObject({ netIncome: 4725000n, expenses: 1950000n, outflows: 2225000n, surplus: 2775000n, closing: 3775000n })
  expect(cycles[1].opening).toBe(3775000n)
  expect(plannerCycles(input, '2027-06')[0].opening).toBe(3775000n)
  expect(input.months).toEqual([])
})

test('recurring and installment schedules stop inclusively and zero overrides only one cycle', () => {
  const input = data(), installment = { ...item('laptop', 'installments'), schedule_start: '2026-12', schedule_end: '2027-02', schedule_amount: '300000' }
  input.items = [installment, { ...item('salary', 'income'), schedule_start: '2026-12', schedule_end: '2027-04', schedule_amount: '5000000' }]
  input.opening = { month: '2026-12', amount: '0' }
  input.amounts = [{ item_id: 'laptop', month: '2027-01', amount: '0' }]
  expect(itemAmount(input, installment, '2026-11').value).toBeNull()
  expect(itemAmount(input, installment, '2026-12').value).toBe(300000n)
  expect(itemAmount(input, installment, '2027-01').value).toBe(0n)
  expect(itemAmount(input, installment, '2027-02').value).toBe(300000n)
  expect(itemAmount(input, installment, '2027-03').value).toBeNull()
  expect(openingForCycle(input, '2027-03')).toBe(14400000n)
  const serialized = JSON.stringify(input)
  plannerCycles(input, '2028-01'); plannerCycles(input, '2026-12')
  expect(JSON.stringify(input)).toBe(serialized)
  expect(openingForCycle(input, '2028-01')).toBe(24400000n)
})

test('missing opening, explicit zero, negative cash and exact large satang remain distinct', () => {
  const input = data(); input.items = [item('water', 'expenses')]
  input.amounts = [{ item_id: 'water', month: '2026-12', amount: '9007199254740993' }]
  expect(plannerCycles(input, '2026-12')[0].closing).toBeNull()
  input.opening = { month: '2026-12', amount: '0' }
  expect(plannerCycles(input, '2026-12')[0].closing).toBe(-9007199254740993n)
  expect(openingForCycle(input, '2026-11')).toBeNull()
  expect(plannerMoney(-9007199254740993n)).toBe('−90,071,992,547,409.93')
  expect(cycleTotals(input, '2027-01').surplus).toBe(0n)
})

test('payday cycles clamp independently across February and year boundaries', () => {
  expect(currentCycle(25, new Date(2027, 0, 24))).toBe('2026-12')
  expect(currentCycle(25, new Date(2027, 0, 25))).toBe('2027-01')
  expect(cycleLabel('2026-12', 25)).toBe('25 Dec 2026 – 24 Jan 2027')
  for (const day of [29, 30, 31]) {
    expect(cycleLabel('2024-02', day)).toBe(`29 Feb 2024 – ${day - 1} Mar 2024`)
    expect(cycleLabel('2025-02', day)).toBe(`28 Feb 2025 – ${day - 1} Mar 2025`)
  }
  expect(addMonths('2026-12', 6)).toBe('2027-06')
  expect(() => parsePlannerAmount('1.001')).toThrow()
  expect(() => parsePlannerAmount('-1')).toThrow()
})

test('Income sources feed payday cycles with month-end clamping and exact carry-forward', () => {
  const input = data()
  input.source_currency = 'THB'
  input.period_start_day = 31
  input.incomes = [{ id: 'salary', name: 'Salary from Income', estimated_amount: '5000000', recurrence_day_of_month: 30, is_active: true, destination_account_name: 'Bank', account_archived: false }]
  input.opening = { month: '2024-01', amount: '0' }
  // Jan 31–Feb 28 excludes both Jan 30 and Feb 29. Feb 29–Mar 30 includes two occurrences.
  expect(cycleTotals(input, '2024-01').netIncome).toBe(0n)
  expect(cycleTotals(input, '2024-02').netIncome).toBe(10000000n)
  expect(openingForCycle(input, '2024-03')).toBe(10000000n)
  input.items = [item('extra', 'income')]
  input.amounts = [{ item_id: 'extra', month: '2024-02', amount: '100000' }, { item_id: 'income:salary', month: '2024-02', amount: '0' }]
  expect(cycleTotals(input, '2024-02').netIncome).toBe(100000n)
  expect(openingForCycle(input, '2024-03')).toBe(100000n)
  expect(cycleTotals(input, '2024-03').netIncome).toBe(0n)
  expect(cycleTotals(input, '2024-04').netIncome).toBe(10000000n)
  input.incomes[0].estimated_amount = '6000000'
  expect(cycleTotals(input, '2024-02').netIncome).toBe(100000n)
  expect(cycleTotals(input, '2024-04').netIncome).toBe(12000000n)
})

test('inactive, unavailable and non-THB income definitions do not generate cashflow', () => {
  const input = data()
  input.source_currency = 'THB'
  const income = { id: 'salary', name: 'Salary', estimated_amount: '9007199254740993', recurrence_day_of_month: 25, is_active: true, destination_account_name: 'Bank', account_archived: false }
  input.incomes = [income]
  expect(cycleTotals(input, '2026-12').netIncome).toBe(9007199254740993n)
  income.is_active = false
  expect(cycleTotals(input, '2026-12').netIncome).toBe(0n)
  income.is_active = true; income.account_archived = true
  expect(cycleTotals(input, '2026-12').netIncome).toBe(0n)
  income.account_archived = false; input.source_currency = 'USD'
  expect(cycleTotals(input, '2026-12').netIncome).toBe(0n)
})

function linkedPlan(id: string, overrides: Partial<PlannerInstallment> = {}): PlannerInstallment {
  return { id, name: id, debt_account_id: 'card', debt_account_name: 'Visa', debt_account_type: 'credit_card', account_name: 'Bank', monthly_amount: '300000', installment_count: 3, first_due_date: '2026-12-24', accounts_available: true, ...overrides }
}

test('loan accounts supply payment rows without treating debt balances as outflows', () => {
  const input = data()
  input.debt_accounts = [{ id: 'loan', name: 'Mortgage', loan_type: 'mortgage', current_balance: '9007199254740993', notes: null, is_archived: false }]
  expect(plannerItems(input)).toHaveLength(1)
  expect(itemAmount(input, plannerItems(input)[0], '2026-12').value).toBeNull()
  expect(cycleTotals(input, '2026-12').expenses).toBe(0n)
  input.amounts = [{ item_id: 'debt:loan', month: '2026-12', amount: '1200000' }]
  input.opening = { month: '2026-12', amount: '2000000' }
  expect(cycleTotals(input, '2026-12').buckets.debt).toBe(1200000n)
  expect(plannerCycles(input, '2026-12')[0].closing).toBe(800000n)
  expect(openingForCycle(input, '2027-02')).toBe(800000n)
})

test('linked card installments follow payday dates, end on time, and overrides replace one cycle', () => {
  const input = data()
  input.installments = [linkedPlan('laptop'), linkedPlan('phone', { monthly_amount: '100000', installment_count: 1, first_due_date: '2026-12-25' })]
  input.opening = { month: '2026-11', amount: '2000000' }
  expect(cycleTotals(input, '2026-11').buckets.installments).toBe(300000n)
  expect(cycleTotals(input, '2026-12').buckets.installments).toBe(400000n)
  expect(cycleTotals(input, '2027-01').buckets.installments).toBe(300000n)
  expect(cycleTotals(input, '2027-02').buckets.installments).toBe(0n)
  expect(cycleTotals(input, '2026-12').buckets.cards).toBe(0n)
  input.amounts = [{ item_id: 'installment:laptop', month: '2026-12', amount: '0' }]
  expect(cycleTotals(input, '2026-12').buckets.installments).toBe(100000n)
  expect(cycleTotals(input, '2027-01').buckets.installments).toBe(300000n)
  expect(openingForCycle(input, '2027-06')).toBe(1300000n)
  input.installments[0].monthly_amount = '400000'
  expect(cycleTotals(input, '2026-12').buckets.installments).toBe(100000n)
})

test('legacy loan schedules appear once under debt and unavailable or non-THB schedules are excluded', () => {
  const input = data()
  input.debt_accounts = [{ id: 'loan', name: 'Old loan', loan_type: null, current_balance: '99999999', notes: '', is_archived: false }]
  input.installments = [linkedPlan('legacy', { debt_account_id: 'loan', debt_account_type: 'loan' }), linkedPlan('unavailable', { accounts_available: false })]
  const rows = plannerItems(input)
  expect(rows.filter(row => row.category_id === 'debt')).toHaveLength(1)
  expect(rows.some(row => row.id === 'installment:legacy')).toBe(false)
  expect(cycleTotals(input, '2026-12').buckets.debt).toBe(300000n)
  expect(cycleTotals(input, '2026-12').buckets.installments).toBe(0n)
  input.amounts = [{ item_id: 'debt:loan', month: '2026-12', amount: '100000' }]
  expect(cycleTotals(input, '2026-12').expenses).toBe(100000n)
  input.debt_accounts[0].is_archived = true
  expect(cycleTotals(input, '2027-01').expenses).toBe(0n)
  expect(cycleTotals(input, '2026-12').expenses).toBe(100000n)
  input.source_currency = 'USD'
  expect(cycleTotals(input, '2026-12').expenses).toBe(0n)
})

test('linked installments clamp due dates independently at leap-year month ends', () => {
  const input = data()
  input.period_start_day = 31
  input.installments = [linkedPlan('leap', { first_due_date: '2024-01-31', installment_count: 3 })]
  for (const month of ['2024-01', '2024-02', '2024-03']) expect(cycleTotals(input, month).buckets.installments).toBe(300000n)
  expect(cycleTotals(input, '2024-04').buckets.installments).toBe(0n)
})

function cardPayment(id: string, amount: string, date = '2026-12-25', overrides: Partial<PlannerCardTransaction> = {}): PlannerCardTransaction {
  return { id, type: 'repayment', account_id: 'bank', account_type: 'bank', destination_account_id: 'card', destination_account_type: 'credit_card', amount, date, ...overrides }
}

test('recorded card payments sum per card and payday cycle without counting purchases or debt transfers', () => {
  const input = data()
  input.credit_cards = [{ id: 'card', name: 'Visa', is_archived: false }, { id: 'other', name: 'Mastercard', is_archived: true }]
  input.card_transactions = [cardPayment('first', '100000'), cardPayment('second', '250000', '2027-01-24', { type: 'transfer' }), cardPayment('next', '70000', '2027-01-25'), cardPayment('other', '40000', '2026-12-25', { destination_account_id: 'other' }),
    cardPayment('purchase', '9999999', '2026-12-25', { type: 'expense', account_id: 'card', account_type: 'credit_card', destination_account_id: null, destination_account_type: null }),
    cardPayment('debt-transfer', '9999999', '2026-12-25', { account_type: 'loan' }),
    cardPayment('refund', '9999999', '2026-12-25', { type: 'income', account_id: 'card', account_type: 'credit_card', destination_account_id: null, destination_account_type: null })]
  const rows = plannerItems(input)
  expect(itemAmount(input, rows.find(row => row.id === 'card:card')!, '2026-12').value).toBe(350000n)
  expect(itemAmount(input, rows.find(row => row.id === 'card:other')!, '2026-12').value).toBe(40000n)
  expect(cycleTotals(input, '2026-12').buckets.cards).toBe(390000n)
  expect(cycleTotals(input, '2027-01').buckets.cards).toBe(70000n)
  input.opening = { month: '2026-12', amount: '1000000' }
  expect(openingForCycle(input, '2027-05')).toBe(540000n)
  input.source_currency = 'USD'
  expect(cycleTotals(input, '2026-12').buckets.cards).toBe(0n)
})

test('partial recorded bills replace installment forecasts once without marking schedules paid', () => {
  const input = data()
  input.credit_cards = [{ id: 'card', name: 'Visa', is_archived: false }]
  input.installments = [linkedPlan('laptop', { first_due_date: '2026-12-25', installment_count: 2 }), linkedPlan('phone', { first_due_date: '2026-12-25', installment_count: 2, monthly_amount: '100000' })]
  input.card_transactions = [cardPayment('partial-bill', '150000')]
  input.amounts = [{ item_id: 'installment:laptop', month: '2026-12', amount: '900000' }]
  input.opening = { month: '2026-12', amount: '1000000' }
  const before = JSON.stringify(input)
  expect(cycleTotals(input, '2026-12')).toMatchObject({ expenses: 150000n, buckets: { cards: 150000n, installments: 0n } })
  expect(cycleTotals(input, '2027-01').expenses).toBe(400000n)
  expect(openingForCycle(input, '2027-04')).toBe(450000n)
  expect(JSON.stringify(input)).toBe(before)
  input.card_transactions = []
  expect(cycleTotals(input, '2026-12').expenses).toBe(1000000n)
  expect(input.amounts[0].amount).toBe('900000')
})

test('card transaction totals and carry-forward preserve amounts beyond Number precision', () => {
  const input = data()
  input.credit_cards = [{ id: 'card', name: 'Visa', is_archived: false }]
  input.card_transactions = [cardPayment('large', '9007199254740993'), cardPayment('small', '7')]
  input.opening = { month: '2026-12', amount: '0' }
  expect(cycleTotals(input, '2026-12').buckets.cards).toBe(9007199254741000n)
  expect(openingForCycle(input, '2027-02')).toBe(-9007199254741000n)
})


test('salary deductions follow the income recurrence and are subtracted once while gross remains unchanged', () => {
  const input = data()
  input.incomes = [{ id: 'salary', name: 'Monthly Salary', estimated_amount: '5000000', recurrence_day_of_month: 25, is_active: true, destination_account_name: 'Bank', account_archived: false }]
  input.income_deductions = [{ id: 'tax', income_id: 'salary', name: 'Tax', description: '', amount: '200000' }, { id: 'social', income_id: 'salary', name: 'Social Security', description: '', amount: '75000' }]
  input.opening = { month: '2026-12', amount: '1000000' }
  expect(cycleTotals(input, '2026-12')).toMatchObject({ buckets: { income: 5000000n, deductions: 275000n }, netIncome: 4725000n, outflows: 275000n, surplus: 4725000n })
  expect(openingForCycle(input, '2027-02')).toBe(10450000n)
  input.amounts = [{ item_id: 'deduction:tax', month: '2026-12', amount: '0' }]
  expect(cycleTotals(input, '2026-12').netIncome).toBe(4925000n)
  expect(cycleTotals(input, '2027-01').netIncome).toBe(4725000n)
  expect(openingForCycle(input, '2027-02')).toBe(10650000n)
  input.incomes[0].is_active = false
  expect(cycleTotals(input, '2027-01').netIncome).toBe(0n)
  input.incomes[0].is_active = true; input.source_currency = 'USD'
  expect(cycleTotals(input, '2027-01').buckets.deductions).toBe(0n)
})

test('salary deductions clamp with salary dates across February and preserve precise amounts', () => {
  const input = data(); input.period_start_day = 31
  input.incomes = [{ id: 'salary', name: 'Salary', estimated_amount: '9007199254740993', recurrence_day_of_month: 30, is_active: true, destination_account_name: 'Bank', account_archived: false }]
  input.income_deductions = [{ id: 'tax', income_id: 'salary', name: 'Tax', description: '', amount: '9007199254740992' }]
  expect(cycleTotals(input, '2024-01').netIncome).toBe(0n)
  expect(cycleTotals(input, '2024-02').netIncome).toBe(2n)
  expect(cycleTotals(input, '2024-02').buckets.deductions).toBe(18014398509481984n)
})
