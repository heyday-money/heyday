import { invoke } from '@tauri-apps/api/core'
import type { Account, Transaction } from './desktop'
import { cardChange, draftBillingDates, statementTotals, type CardBillingData } from './card-billing'

export interface CardOverviewData { selective_defaults?: import('./selective-defaults').SelectiveDefault[]; period_start_day?: number; accounts: Account[]; archived_ids: string[]; currency: string | null; billing: CardBillingData; transactions: Transaction[] }
export const getCardOverview = () => invoke<CardOverviewData>('get_card_overview')
const positive = (n: bigint) => n > 0n ? n : 0n
function nextDay(date: string) {
  const d = new Date(`${date}T12:00:00Z`); d.setUTCDate(d.getUTCDate() + 1)
  return d.toISOString().slice(0, 10)
}
function monthDate(date: string, offset: number, day: number) {
  const [year, month] = date.split('-').map(Number)
  const end = new Date(0); end.setUTCFullYear(year, month - 1 + offset + 1, 0)
  const d = new Date(0); d.setUTCFullYear(year, month - 1 + offset, Math.min(day, end.getUTCDate()))
  return d.toISOString().slice(0, 10)
}
function nextClose(today: string, day: number) {
  const close = monthDate(today, 0, day)
  return close > today ? close : monthDate(today, 1, day)
}
function dueFor(end: string, day: number | null) {
  if (day == null) return null
  const due = monthDate(end, 0, day)
  return due > end ? due : monthDate(end, 1, day)
}

export function cardOverview(data: CardOverviewData, today: string) {
  const rows = data.accounts.filter(account => !account.is_archived && !data.archived_ids.includes(account.id)).map(account => {
    const transactions = data.transactions.filter(t => t.date <= today && (t.account_id === account.id || t.destination_account_id === account.id))
    const statements = data.billing.statements.filter(s => s.account_id === account.id && s.end_date <= today).sort((a,b) => b.end_date.localeCompare(a.end_date))
    const scheduled = account.statement_day == null ? null : draftBillingDates(account.statement_day, account.payment_due_day, today)
    // A reviewed bank statement supplies authoritative dates for the latest cycle.
    const statement = statements.find(s => !scheduled || s.end_date >= scheduled.start_date)
    const start = statement?.start_date ?? scheduled?.start_date ?? null
    const end = statement?.end_date ?? scheduled?.end_date ?? null
    const nextEnd = account.statement_day == null ? null : nextClose(today, account.statement_day)
    const nextStart = end ? nextDay(end) : null
    const purchases = (from: string | null, to: string | null) => from && to ? transactions.filter(t => t.type === 'expense' && t.account_id === account.id && t.date >= from && t.date <= to) : []
    const latestPurchases = purchases(start, end)
    const nextPurchases = purchases(nextStart, nextEnd ?? today)
    const sum = (items: Transaction[]) => items.reduce((n,t) => n + BigInt(t.amount), 0n)
    const balance = BigInt(account.current_balance)
    const changesAfterClose = end ? transactions.filter(t => t.date > end).reduce((n,t) => n + cardChange(t,account.id), 0n) : 0n
    const estimatedClosing = end ? balance - changesAfterClose : null
    const totals = statement ? statementTotals(data.billing, statement, today) : null
    const older = statements.filter(s => s.id !== statement?.id && (!end || s.end_date < (start ?? end))).map(s => ({ statement:s, totals:statementTotals(data.billing,s,today) })).filter(s => s.totals.remaining > 0n)
    const linkedPayments = new Set(data.billing.allocations.map(a => a.transaction_id))
    const unassignedPayments = transactions.filter(t => t.destination_account_id === account.id && ['repayment','transfer'].includes(t.type) && !linkedPayments.has(t.id))
    return { account, archived:data.archived_ids.includes(account.id), balance, owed:positive(balance), credit:positive(-balance), start, end, due:statement?.due_date ?? (end ? dueFor(end,account.payment_due_day) : null), nextStart, nextEnd, nextDue:nextEnd ? dueFor(nextEnd,account.payment_due_day) : null,
      statement, totals, estimatedClosing, latestPurchases, nextPurchases, latestSpending:start && end ? sum(latestPurchases) : null, nextSpending:nextStart ? sum(nextPurchases) : null, older, unassignedPayments,
      difference:statement && estimatedClosing !== null ? estimatedClosing - BigInt(statement.amount) : null }
  })
  return { rows, owed:rows.reduce((n,r) => n+r.owed,0n), credit:rows.reduce((n,r) => n+r.credit,0n), latestSpending:rows.reduce((n,r) => n+(r.latestSpending??0n),0n), nextSpending:rows.reduce((n,r) => n+(r.nextSpending??0n),0n), unconfigured:rows.filter(r => r.account.statement_day == null).length }
}
export type CardOverviewRow = ReturnType<typeof cardOverview>['rows'][number]
