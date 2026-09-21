import { expect, test } from '@playwright/test'
import { dateKey, monthlyOutlook, netWorth } from '../src/lib/financial'
import type { Account, FinancialData, Income, PaymentPlan, Transaction } from '../src/lib/desktop'

function account(id: string, type: Account['type'], balance: string): Account {
  return { id, name: id, type, current_balance: balance, opening_balance: balance, loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_bps: null }
}
function income(id: string, destination: string, day: number, amount = '1000', active = true): Income {
  return { id, name: id, type: 'salary', destination_account_id: destination, destination_account_name: destination, estimated_amount: amount, deductions_total: '0', recurrence_frequency: 'monthly', recurrence_day_of_month: day, is_active: active, is_auto_create_transaction: false, created_at: '', updated_at: '' }
}
function transaction(type: Transaction['type'], source: string, amount: string, destination: string | null = null): Transaction {
  return { id: `${type}-${source}-${amount}`, type, account_id: source, account_name: source, destination_account_id: destination, destination_account_name: destination, amount, date: '2026-01-15', description: '', payee_id: null, payee_name: null, category_id: null, category_name: null }
}
function plan(type: PaymentPlan['type'], date: string, amount: string): PaymentPlan {
  return { id: `${type}-${date}`, name: type, type, date, amount, account_id: 'bank', account_name: 'bank', destination_account_id: type === 'repayment' ? 'card' : null, destination_account_name: type === 'repayment' ? 'card' : null, category_id: null, category_name: null }
}
function data(): FinancialData {
  return { settings: { currency: 'THB', period_start_day: 1 }, accounts: [account('bank', 'bank', '10000'), account('cash', 'cash', '2000'), account('card', 'credit_card', '5000'), account('investment', 'investment', '50000')], incomes: [], transactions: [], plans: [], installments: [], subscriptions: [], categories: [] }
}

test('cash outlook separates actuals, expectations and plans without double-counting transfers or credit spending', () => {
  const input = data()
  input.transactions = [transaction('income', 'bank', '3000'), transaction('expense', 'bank', '1000'), transaction('repayment', 'bank', '500', 'card'), transaction('transfer', 'bank', '2000', 'cash'), transaction('expense', 'card', '700'), transaction('transfer', 'investment', '100', 'bank'), transaction('transfer', 'bank', '300', 'investment')]
  input.incomes = [income('Salary', 'bank', 25), income('Inactive', 'bank', 25, '999999', false), income('Investment only', 'investment', 25), income('Today already passed', 'bank', 15, '0')]
  input.plans = [plan('expense', '2026-01-20', '2000'), plan('repayment', '2026-01-25', '500'), plan('expense', '2026-02-10', '500'), plan('repayment', '2026-02-15', '250'), plan('expense', '2026-01-15', '999999')]
  const before = JSON.stringify(input)
  const periods = monthlyOutlook(input, new Date(2026, 0, 15))
  expect(periods).toHaveLength(7)
  expect(periods[0]).toMatchObject({ opening: 10700n, actualNet: 1300n, forecastNet: -1500n, closing: 10500n, partial: false })
  expect(periods[0].buckets.expenses.actual).toBe(1000n)
  expect(periods[0].buckets.repayments.actual).toBe(500n)
  expect(periods[1]).toMatchObject({ opening: 10500n, actualNet: 0n, forecastNet: 250n, closing: 10750n })
  expect(periods[2]).toMatchObject({ opening: 10750n, closing: 11750n, missingExpenses: true, missingRepayments: true, partial: true })
  expect(JSON.stringify(input)).toBe(before)
})

test('monthly recurrence and payday boundaries clamp independently in leap years and months with fewer days', () => {
  for (const year of [2024, 2025]) for (const day of [29, 30, 31]) {
    const input = data()
    input.settings.period_start_day = day
    input.incomes = [income('Month end', 'bank', 31)]
    const periods = monthlyOutlook(input, new Date(year, 0, day))
    expect(periods[0].toKey).toBe(`${year}-02-${year === 2024 ? '29' : '28'}`)
    expect(periods[1].fromKey).toBe(periods[0].toKey)
    expect(periods[1].toKey).toBe(`${year}-03-${day}`)
    for (let i = 1; i < periods.length; i++) expect(periods[i].fromKey).toBe(periods[i - 1].toKey)
    const dates: string[] = []
    for (let month = 0; month < 8; month++) {
      const occurrence = dateKey(new Date(year, month + 1, 0))
      if (occurrence > dateKey(new Date(year, 0, day)) && occurrence < periods[6].toKey) dates.push(occurrence)
    }
    expect(periods.reduce((total, period) => total + period.buckets.income.forecast, 0n)).toBe(BigInt(dates.length) * 1000n)
  }
})

test('exact net worth handles debt, overdrafts, credits and totals beyond i64 and JS number limits', () => {
  const accounts = [account('bank', 'bank', '9007199254740993'), account('overdraft', 'bank', '-100'), account('card', 'credit_card', '500'), account('credit', 'loan', '-50'), account('fund', 'investment', '9223372036854775807')]
  const result = netWorth(accounts)
  expect(result.assets).toBe(9007199254740993n + 50n + 9223372036854775807n)
  expect(result.liabilities).toBe(600n)
  expect(result.total).toBe(result.assets - result.liabilities)
  expect(netWorth([])).toEqual({ assets: 0n, liabilities: 0n, total: 0n })
})

test('past, inactive-account plans and paid dates are excluded; year rollover and final-day actuals remain correct', () => {
  const input = data()
  input.plans = [plan('expense', '2026-12-31', '999'), { ...plan('expense', '2027-01-02', '999'), account_id: 'archived' }]
  input.incomes = [income('Salary', 'bank', 31)]
  const periods = monthlyOutlook(input, new Date(2026, 11, 31))
  expect(periods[0]).toMatchObject({ remainingDays: false, forecastNet: 0n, partial: false, closing: 12000n })
  expect(periods[1].fromKey).toBe('2027-01-01')
  expect(periods[1].buckets.expenses.forecast).toBe(0n)
  expect(periods[1].buckets.income.forecast).toBe(1000n)
  expect(periods[6].fromKey).toBe('2027-06-01')
})

test('installments forecast once per month without changing actuals or treating elapsed payments as paid', () => {
  const input = data()
  input.installments = [{ id: 'laptop', name: 'Laptop', account_id: 'bank', account_name: 'bank', debt_account_id: 'card', debt_account_name: 'card', debt_account_type: 'credit_card', purchase_kind: 'existing_purchase', purchase_transaction_id: null, monthly_amount: '9007199254740993', interest_rate_bps: null, installment_count: 3, first_due_date: '2026-01-31' }]
  const before = JSON.stringify(input)
  const periods = monthlyOutlook(input, new Date(2026, 0, 15))
  expect(periods.slice(0, 3).map(period => period.buckets.repayments.forecast)).toEqual(Array(3).fill(9007199254740993n))
  expect(periods[3].buckets.repayments.forecast).toBe(0n)
  expect(periods[0].buckets.repayments.actual).toBe(0n)
  expect(JSON.stringify(input)).toBe(before)
  expect(monthlyOutlook(input, new Date(2026, 0, 31))[0].buckets.repayments.forecast).toBe(0n)
  input.accounts = input.accounts.filter(account => account.id !== 'card')
  expect(monthlyOutlook(input, new Date(2026, 0, 15))[0].buckets.repayments.forecast).toBe(0n)
})

test('account-based outlook forecasts net salary once after configured deductions', () => {
  const input = data()
  input.incomes = [{ ...income('salary', 'bank', 25, '5000000'), deductions_total: '275000' }]
  const periods = monthlyOutlook(input, new Date(2026, 0, 15))
  expect(periods[0].buckets.income.forecast).toBe(4725000n)
  expect(periods[1].buckets.income.forecast).toBe(4725000n)
})
