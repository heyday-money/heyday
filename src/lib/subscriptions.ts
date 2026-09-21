import type { Subscription } from './desktop'

export function validSubscriptionDate(value: string) {
  if (!/^(?!0000)\d{4}-\d{2}-\d{2}$/.test(value)) return false
  const [year, month, day] = value.split('-').map(Number)
  return month >= 1 && month <= 12 && day >= 1 && day <= monthDays(year, month)
}
function monthDays(year: number, month: number) {
  return [31, year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1]
}
// Inclusive range. Derive every occurrence from the original date to prevent clamping drift.
export function subscriptionDates(plan: Pick<Subscription, 'frequency' | 'first_billing_date' | 'end_date'>, from: string, through: string) {
  if (!validSubscriptionDate(plan.first_billing_date) || !validSubscriptionDate(from) || !validSubscriptionDate(through) || (plan.end_date !== null && (!validSubscriptionDate(plan.end_date) || plan.end_date < plan.first_billing_date))) throw new Error('Enter valid billing dates; the end date cannot be before the first billing date.')
  if (!['monthly', 'yearly'].includes(plan.frequency)) throw new Error('Choose monthly or yearly billing.')
  const start = from > plan.first_billing_date ? from : plan.first_billing_date
  const end = plan.end_date && plan.end_date < through ? plan.end_date : through
  if (start > end) return []
  const [, anchorMonth, day] = plan.first_billing_date.split('-').map(Number)
  const [startYear, startMonth] = start.split('-').map(Number)
  const [endYear, endMonth] = end.split('-').map(Number)
  const dates: string[] = []
  for (let index = startYear * 12 + startMonth - 1; index <= endYear * 12 + endMonth - 1; index++) {
    const year = Math.floor(index / 12), month = index % 12 + 1
    if (plan.frequency === 'yearly' && month !== anchorMonth) continue
    const date = `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-${String(Math.min(day, monthDays(year, month))).padStart(2, '0')}`
    if (date >= start && date <= end) dates.push(date)
  }
  return dates
}
export function nextSubscriptionDate(plan: Subscription, today: string) {
  if (!plan.is_active) return null
  const through = `${Math.min(9999, Number(today.slice(0, 4)) + 1)}-12-31`.padStart(10, '0')
  // For schedules starting later than the lookup horizon, the original first date is next.
  if (plan.first_billing_date > through) return plan.first_billing_date
  return subscriptionDates(plan, today, through)[0] ?? null
}
