import { test, expect } from '@playwright/test'
import { nextSubscriptionDate, subscriptionDates } from '../src/lib/subscriptions'
import { monthlyOutlook } from '../src/lib/financial'
import type { Account, FinancialData, Subscription } from '../src/lib/desktop'

function plan(overrides: Partial<Subscription> = {}): Subscription {
  return { id: 'service', name: 'Service', account_id: 'bank', account_name: 'Bank', account_type: 'bank', category_id: null, category_name: null, amount: '9007199254740993', frequency: 'monthly', first_billing_date: '2024-01-31', end_date: null, is_active: true, ...overrides }
}
test('subscription recurrence anchors month-end, yearly leap days and inclusive end dates', () => {
  expect(subscriptionDates(plan(), '2024-01-01', '2024-03-31')).toEqual(['2024-01-31', '2024-02-29', '2024-03-31'])
  expect(subscriptionDates(plan({ frequency: 'yearly', first_billing_date: '2024-02-29' }), '2025-01-01', '2028-03-01')).toEqual(['2025-02-28', '2026-02-28', '2027-02-28', '2028-02-29'])
  expect(subscriptionDates(plan({ end_date: '2024-02-29' }), '2024-02-29', '2024-04-01')).toEqual(['2024-02-29'])
  expect(subscriptionDates(plan(), '2023-01-01', '2023-12-31')).toEqual([])
  expect(nextSubscriptionDate(plan(), '2024-02-29')).toBe('2024-02-29')
  expect(nextSubscriptionDate(plan({ is_active: false }), '2024-01-01')).toBeNull()
  expect(nextSubscriptionDate(plan({ end_date: '2024-02-29' }), '2024-03-01')).toBeNull()
  expect(nextSubscriptionDate(plan({ first_billing_date: '2999-01-01' }), '2024-01-01')).toBe('2999-01-01')
  expect(nextSubscriptionDate(plan({ first_billing_date: '9999-12-31' }), '9999-12-31')).toBe('9999-12-31')
  expect(() => subscriptionDates(plan({ first_billing_date: '2025-02-29' }), '2024-01-01', '2026-01-01')).toThrow()
})
test('Outlook includes each future cash subscription once and excludes paused, ended, card and unavailable charges', () => {
  const base: Account = { id: 'bank', name: 'Bank', type: 'bank', opening_balance: '100000', current_balance: '100000', loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_bps: null }
  const data: FinancialData = { settings: { currency: 'THB', period_start_day: 25 }, accounts: [base, { ...base, id: 'card', type: 'credit_card' }], transactions: [], incomes: [], plans: [], installments: [], categories: [], subscriptions: [plan(), plan({ id: 'paused', is_active: false }), plan({ id: 'card', account_id: 'card', account_type: 'credit_card' }), plan({ id: 'ended', end_date: '2024-01-31' }), plan({ id: 'missing', account_id: 'gone' })] }
  const before = JSON.stringify(data)
  const periods = monthlyOutlook(data, new Date(2024, 1, 15))
  expect(periods[0].buckets.expenses.forecast).toBe(0n)
  expect(periods[1].buckets.expenses.forecast).toBe(9007199254740993n)
  expect(periods[2].buckets.expenses.forecast).toBe(9007199254740993n)
  expect(periods[1].buckets.expenses.plannedCount).toBe(1)
  expect(JSON.stringify(data)).toBe(before)
  expect(monthlyOutlook(data, new Date(2024, 1, 29))[0].buckets.expenses.forecast).toBe(0n)
})
