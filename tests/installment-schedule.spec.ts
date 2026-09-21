import { test, expect } from '@playwright/test'
import { installmentSchedule } from '../src/lib/installments'

test('monthly installments clamp independently and preserve exact amounts', () => {
  const schedule = installmentSchedule({ monthly_amount: '9007199254740993', installment_count: 4, first_due_date: '2024-01-31' })
  expect(schedule.map(item => item.date)).toEqual(['2024-01-31', '2024-02-29', '2024-03-31', '2024-04-30'])
  expect(schedule.reduce((total, item) => total + BigInt(item.amount), 0n)).toBe(36028797018963972n)
  expect(installmentSchedule({ monthly_amount: '1', installment_count: 3, first_due_date: '2025-12-30' }).map(item => item.date)).toEqual(['2025-12-30', '2026-01-30', '2026-02-28'])
  expect(installmentSchedule({ monthly_amount: '1', installment_count: 2, first_due_date: '0099-12-31' })[1].date).toBe('0100-01-31')
})

test('installments reject invalid dates, terms and overflowing totals', () => {
  const valid = { monthly_amount: '100', installment_count: 12, first_due_date: '2024-01-31' }
  for (const first_due_date of ['0000-01-01', '2023-02-29', '2024-13-01', 'bad', '9999-12-31']) expect(() => installmentSchedule({ ...valid, first_due_date })).toThrow()
  for (const installment_count of [0, -1, 1.5, 601, NaN]) expect(() => installmentSchedule({ ...valid, installment_count })).toThrow()
  for (const monthly_amount of ['0', '-1', '1.5', '9223372036854775808', '9223372036854775807']) expect(() => installmentSchedule({ ...valid, monthly_amount })).toThrow()
  expect(installmentSchedule({ monthly_amount: '9223372036854775807', installment_count: 1, first_due_date: '9999-12-31' })).toHaveLength(1)
})

test('12-month matrix totals each item and includes payments beyond the visible year', async () => {
  const { installmentYearSummary } = await import('../src/lib/installments')
  const base = { id: 'a', name: 'Item', account_id: 'bank', account_name: 'Bank', debt_account_id: 'card', debt_account_name: 'Card', debt_account_type: 'credit_card' as const, monthly_amount: '20000', installment_count: 14, first_due_date: '2026-01-31', interest_rate_bps: null, purchase_kind: 'existing_purchase' as const, purchase_transaction_id: null }
  const summary = installmentYearSummary([base, { ...base, id: 'b', name: 'Item 2', monthly_amount: '10000', installment_count: 1 }], '2026-01')
  expect(summary.months).toHaveLength(12)
  expect(summary.months[11]).toBe('2026-12')
  expect(summary.rows[0].amounts).toEqual(Array(12).fill(20000n))
  expect(summary.rows[1].amounts).toEqual([10000n, ...Array(11).fill(0n)])
  expect(summary.total.amounts).toEqual([30000n, ...Array(11).fill(20000n)])
  expect(summary.total.remaining).toBe(290000n)
  expect(installmentYearSummary([base], '2026-02').rows[0].remaining).toBe(260000n)
  expect(installmentYearSummary([base], '2027-03').rows).toHaveLength(0)
  const future = installmentYearSummary([{ ...base, first_due_date: '2027-01-31' }], '2026-01')
  expect(future.rows[0].amounts).toEqual(Array(12).fill(0n))
  expect(future.rows[0].remaining).toBe(280000n)
  const exact = installmentYearSummary([{ ...base, monthly_amount: '9007199254740993' }], '2026-01')
  expect(exact.total.remaining).toBe(126100789566373902n)
  expect(installmentYearSummary([], '9999-01').months[11]).toBe('9999-12')
  expect(() => installmentYearSummary([], '9999-02')).toThrow()
})
