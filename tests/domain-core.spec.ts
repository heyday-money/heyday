import { expect, test } from '@playwright/test'
import vectors from '../crates/heyday-core/tests/fixtures/domain.json' with { type: 'json' }
import { decimalToInteger, fractionDigits } from '../src/lib/money'
import { currentPeriod } from '../src/lib/period'
import { accountValue, dateKey } from '../src/lib/financial'
import type { Account } from '../src/lib/desktop'

test('shared core decimal and currency precision vectors match frontend input conversion', () => {
  for (const c of vectors.decimal) {
    if (c.result === null) expect(() => decimalToInteger(c.value, c.digits), JSON.stringify(c)).toThrow()
    else expect(decimalToInteger(c.value, c.digits)).toBe(c.result)
  }
  for (const c of vectors.precision) expect(fractionDigits(c.currency)).toBe(c.digits)
})

test('shared core balances preserve debt signs, overdrafts and exact large integers', () => {
  for (const c of vectors.balances) {
    const account = { type: c.kind, current_balance: c.balance } as Account
    expect(accountValue(account).toString()).toBe(c.value)
  }
})

test('shared core payday cycles match local calendar boundaries', () => {
  for (const c of vectors.periods) {
    const [year, month, day] = c.today.split('-').map(Number)
    const period = currentPeriod(c.day, new Date(year, month - 1, day))
    expect(dateKey(period.start), JSON.stringify(c)).toBe(c.start)
    expect(dateKey(period.end), JSON.stringify(c)).toBe(c.end)
  }
})
