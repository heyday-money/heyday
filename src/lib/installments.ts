import type { Installment } from './desktop'

function daysInMonth(year: number, month: number) {
  return [31, year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1]
}
export function installmentSchedule(plan: Pick<Installment, 'first_due_date' | 'installment_count' | 'monthly_amount'>) {
  const { first_due_date: first, installment_count: count, monthly_amount: amount } = plan
  if (!Number.isInteger(count) || count < 1 || count > 600) throw new Error('Choose between 1 and 600 monthly installments.')
  if (!/^\d+$/.test(amount) || BigInt(amount) <= 0n || BigInt(amount) * BigInt(count) > 9223372036854775807n) throw new Error('Monthly payment must be positive and the total must fit the supported range.')
  if (!/^\d{4}-\d{2}-\d{2}$/.test(first)) throw new Error('Enter a valid first due date.')
  const [year, month, day] = first.split('-').map(Number)
  if (year < 1 || month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) throw new Error('Enter a valid first due date.')
  if (year * 12 + month - 1 + count - 1 >= 10000 * 12) throw new Error('The last installment must be no later than December 9999.')
  return Array.from({ length: count }, (_, index) => {
    const absoluteMonth = year * 12 + month - 1 + index
    const dueYear = Math.floor(absoluteMonth / 12), dueMonth = absoluteMonth % 12 + 1
    return { date: `${String(dueYear).padStart(4, '0')}-${String(dueMonth).padStart(2, '0')}-${String(Math.min(day, daysInMonth(dueYear, dueMonth))).padStart(2, '0')}`, amount }
  })
}

export function interestRateText(units: string, digits = 3) {
  const value = BigInt(units)
  const scale = 10n ** BigInt(digits)
  const fraction = (value % scale).toString().padStart(digits, '0').replace(/0+$/, '')
  return `${value / scale}${fraction ? '.' + fraction : ''}`
}

export function installmentYearSummary(plans: Installment[], startMonth: string) {
  if (!/^(?!0000)\d{4}-(0[1-9]|1[0-2])$/.test(startMonth) || startMonth > '9999-01') throw new Error('Choose a start month no later than January 9999.')
  const [year, month] = startMonth.split('-').map(Number)
  const months = Array.from({ length: 12 }, (_, index) => {
    const value = year * 12 + month - 1 + index
    return `${String(Math.floor(value / 12)).padStart(4, '0')}-${String(value % 12 + 1).padStart(2, '0')}`
  })
  const totals = months.map(() => 0n)
  const rows = plans.flatMap(plan => {
    const schedule = installmentSchedule(plan).filter(payment => payment.date.slice(0, 7) >= startMonth)
    if (!schedule.length) return []
    const amounts = months.map(monthKey => schedule.filter(payment => payment.date.slice(0, 7) === monthKey).reduce((total, payment) => total + BigInt(payment.amount), 0n))
    amounts.forEach((amount, index) => { totals[index] += amount })
    return [{ id: plan.id, name: plan.name, accountName: plan.debt_account_name, amounts, remaining: schedule.reduce((total, payment) => total + BigInt(payment.amount), 0n), total: false }]
  })
  return { months, rows, total: { id: 'summary:total', name: 'Total', accountName: '', amounts: totals, remaining: rows.reduce((total, row) => total + row.remaining, 0n), total: true } }
}
