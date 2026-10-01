import type { FinancialData } from './desktop'
import { dateKey, isCash, isDebt } from './financial'
import { currentPeriod } from './period'
import { statementTotals } from './card-billing'

function monthDate(year: number, month: number, day: number) {
  const last = new Date(0)
  last.setFullYear(year, month + 1, 0)
  const result = new Date(0)
  result.setFullYear(year, month, Math.min(day, last.getDate()))
  result.setHours(0, 0, 0, 0)
  return result
}

export function homeCycle(day: number, month: string, today: Date) {
  const current = currentPeriod(day, today)
  const key = month || dateKey(current.start).slice(0, 7)
  if (!/^(\d{4})-(0[1-9]|1[0-2])$/.test(key) || key < '0001-01' || key > '9999-11') throw new Error('Choose a valid cycle month.')
  const year = Number(key.slice(0, 4)), index = Number(key.slice(5)) - 1
  const start = monthDate(year, index, day), end = monthDate(year, index + 1, day)
  const last = new Date(end)
  last.setDate(last.getDate() - 1)
  return { key, start, end, last, startKey: dateKey(start), endKey: dateKey(end) }
}

export interface HomeAttention {
  id: string
  name: string
  title: string
  detail: string
  amount?: bigint
  accountId?: string
  kind: 'billing' | 'plan'
  date: string
}

// Home consumes one native snapshot. No estimates are added to ledger totals.
export function homeOverview(data: FinancialData, month: string, today: Date) {
  const cycle = homeCycle(data.settings.period_start_day, month, today)
  const todayKey = dateKey(today)
  const accounts = new Map(data.accounts.map(account => [account.id, account]))
  const cash = data.accounts.filter(isCash).reduce((sum, account) => sum + BigInt(account.current_balance), 0n)
  let incomeReceived = 0n, spending = 0n
  const recorded = data.transactions.filter(row => row.date <= todayKey)
  for (const row of recorded) {
    if (row.date < cycle.startKey || row.date >= cycle.endKey) continue
    if (row.type === 'income') incomeReceived += BigInt(row.amount)
    if (row.type === 'expense') spending += BigInt(row.amount)
  }
  const upcomingIncome = data.incomes.filter(source => source.is_active && accounts.has(source.destination_account_id)).map(source => {
    let date = monthDate(today.getFullYear(), today.getMonth(), source.recurrence_day_of_month)
    if (dateKey(date) < todayKey) date = monthDate(today.getFullYear(), today.getMonth() + 1, source.recurrence_day_of_month)
    return { source, date: dateKey(date), amount: BigInt(source.estimated_amount) - BigInt(source.deductions_total ?? '0') }
  }).sort((a, b) => a.date.localeCompare(b.date) || a.source.name.localeCompare(b.source.name) || a.source.id.localeCompare(b.source.id))

  const horizon = new Date(today)
  horizon.setDate(horizon.getDate() + 14)
  const horizonKey = dateKey(horizon)
  const attention: HomeAttention[] = []
  const billing = data.card_billing
  // Older statement balances are references, never additional debt to aggregate.
  const cards = new Set((billing?.statements ?? []).map(statement => statement.account_id))
  for (const accountId of cards) {
    const statements = billing!.statements.filter(s => s.account_id === accountId).sort((a, b) => b.end_date.localeCompare(a.end_date) || b.id.localeCompare(a.id))
    const latest = statements[0]
    const name = accounts.get(accountId)?.name ?? recorded.find(t => t.account_id === accountId)?.account_name ?? 'Credit card'
    const review = statements.find(statement => statement.needs_review)
    if (review) {
      attention.push({ id: `card:${accountId}`, name, title: 'Statement needs review', detail: 'Review statement activity and payment links.', accountId, kind: 'billing', date: review.due_date })
      continue
    }
    const totals = statementTotals(billing!, latest, todayKey)
    if (totals.remaining > 0n && latest.due_date <= horizonKey) {
      attention.push({ id: `card:${accountId}`, name, title: latest.due_date < todayKey ? 'Statement due date has passed' : latest.due_date === todayKey ? 'Statement due today' : 'Upcoming statement due date',
        detail: totals.minimum === 0n ? 'Minimum covered · balance remains' : 'Review or record your payment.', amount: totals.remaining, accountId, kind: 'billing', date: latest.due_date })
    } else if (statements.slice(1).some(statement => statementTotals(billing!, statement, todayKey).remaining > 0n)) {
      attention.push({ id: `card:${accountId}`, name, title: 'Review older statement balances', detail: 'Older unapplied balances are references, not additional debt.', accountId, kind: 'billing', date: latest.due_date })
    }
  }
  for (const plan of data.plans) {
    const source = accounts.get(plan.account_id), destination = plan.destination_account_id ? accounts.get(plan.destination_account_id) : undefined
    const available = source && isCash(source) && (plan.type === 'expense' || destination && isDebt(destination))
    if (available && plan.date > horizonKey) continue
    attention.push({ id: `plan:${plan.id}`, name: plan.name,
      title: !available ? 'Plan account unavailable' : plan.date < todayKey ? 'Past plan · needs review' : plan.date === todayKey ? 'Plan scheduled today' : 'Upcoming payment plan',
      detail: 'A saved plan, not a confirmed payment. Review it or record the actual transaction.', amount: BigInt(plan.amount), kind: 'plan', date: plan.date })
  }
  attention.sort((a, b) => a.date.localeCompare(b.date) || a.id.localeCompare(b.id))
  // Stable sorting preserves native newest-created-first ordering on the same date.
  const recent = [...recorded].sort((a, b) => b.date.localeCompare(a.date)).slice(0, 5)
  return { cycle, cash, incomeReceived, spending, upcomingIncome, attention, recent }
}
