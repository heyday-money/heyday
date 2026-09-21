import { subscriptionDates } from './subscriptions'
import { installmentSchedule } from './installments'
import type { Account, FinancialData } from './desktop'
import { currentPeriod } from './period'

export function dateKey(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}
export function monthlyDate(year: number, month: number, day: number) {
  return new Date(year, month, Math.min(day, new Date(year, month + 1, 0).getDate()))
}
export function isCash(account: Account) { return account.type === 'cash' || account.type === 'bank' }
export function isDebt(account: Account) { return account.type === 'credit_card' || account.type === 'loan' }
export function accountValue(account: Account) {
  const balance = BigInt(account.current_balance ?? account.opening_balance)
  return isDebt(account) ? -balance : balance
}
export function netWorth(accounts: Account[]) {
  let assets = 0n, liabilities = 0n
  for (const account of accounts) {
    const value = accountValue(account)
    if (value >= 0n) assets += value
    else liabilities -= value
  }
  return { assets, liabilities, total: assets - liabilities }
}
export type FlowKey = 'income' | 'expenses' | 'repayments' | 'other'
export interface FlowDetail { key: string; label: string; actual: bigint; forecast: bigint }
export interface Flow { actual: bigint; forecast: bigint; plannedCount: number; details: Map<string, FlowDetail> }
function flow(): Flow { return { actual: 0n, forecast: 0n, plannedCount: 0, details: new Map() } }
function add(target: Flow, key: string, label: string, mode: 'actual' | 'forecast', amount: bigint) {
  target[mode] += amount
  const detail = target.details.get(key) ?? { key, label, actual: 0n, forecast: 0n }
  detail[mode] += amount
  target.details.set(key, detail)
  if (mode === 'forecast') target.plannedCount++
}
export function monthlyOutlook(data: FinancialData, today = new Date()) {
  const { start } = currentPeriod(data.settings.period_start_day, today)
  const todayString = dateKey(today)
  const cash = new Set(data.accounts.filter(isCash).map(account => account.id))
  const debt = new Set(data.accounts.filter(isDebt).map(account => account.id))
  const hasDebt = data.accounts.some(account => isDebt(account) && accountValue(account) < 0n)
  const currentCash = data.accounts.filter(isCash).reduce((total, account) => total + accountValue(account), 0n)
  const scheduledRepayments = (data.installments ?? []).filter(plan => cash.has(plan.account_id) && debt.has(plan.debt_account_id))
    .flatMap(plan => installmentSchedule(plan).map(payment => ({ ...payment, debtId: plan.debt_account_id, debtName: plan.debt_account_name })))
  let previousClosing = currentCash
  let partial = false
  return Array.from({ length: 7 }, (_, index) => {
    const from = monthlyDate(start.getFullYear(), start.getMonth() + index, data.settings.period_start_day)
    const to = monthlyDate(start.getFullYear(), start.getMonth() + index + 1, data.settings.period_start_day)
    const fromKey = dateKey(from), toKey = dateKey(to)
    const remainingDays = dateKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1)) < toKey
    const inside = (date: string) => date >= fromKey && date < toKey
    const buckets = { income: flow(), expenses: flow(), repayments: flow(), other: flow() }
    if (index === 0) for (const row of data.transactions) {
      if (!inside(row.date) || row.date > todayString) continue
      const amount = BigInt(row.amount)
      const sourceCash = cash.has(row.account_id)
      const destinationCash = row.destination_account_id !== null && cash.has(row.destination_account_id)
      if (row.type === 'income' && sourceCash) add(buckets.income, `actual:${row.account_id}`, `Recorded income · ${row.account_name}`, 'actual', amount)
      else if (row.type === 'expense' && sourceCash) add(buckets.expenses, row.category_id ?? 'uncategorized', row.category_name ?? 'Uncategorized', 'actual', amount)
      else if (row.type === 'transfer' || row.type === 'repayment') {
        if (sourceCash && row.destination_account_id && debt.has(row.destination_account_id)) {
          add(buckets.repayments, row.destination_account_id, row.destination_account_name ?? 'Debt account', 'actual', amount)
        } else {
          const change = (destinationCash ? amount : 0n) - (sourceCash ? amount : 0n)
          if (change !== 0n) add(buckets.other, `${row.account_id}:${row.destination_account_id}`, `${row.account_name} → ${row.destination_account_name}`, 'actual', change)
        }
      }
    }
    for (const income of data.incomes) {
      if (!income.is_active || !cash.has(income.destination_account_id)) continue
      // Boundaries and income occurrences clamp independently in each calendar month.
      for (let offset = 0; offset <= 1; offset++) {
        const date = dateKey(monthlyDate(from.getFullYear(), from.getMonth() + offset, income.recurrence_day_of_month))
        if (inside(date) && date > todayString) add(buckets.income, `expected:${income.id}`, `${income.name} · Expected`, 'forecast', BigInt(income.estimated_amount) - BigInt(income.deductions_total ?? '0'))
      }
    }
    for (const plan of data.plans) {
      if (!inside(plan.date) || plan.date <= todayString || !cash.has(plan.account_id)) continue
      if (plan.type === 'expense') add(buckets.expenses, plan.category_id ?? 'uncategorized', plan.category_name ?? 'Uncategorized', 'forecast', BigInt(plan.amount))
      else if (plan.destination_account_id && debt.has(plan.destination_account_id)) add(buckets.repayments, plan.destination_account_id, plan.destination_account_name ?? 'Debt account', 'forecast', BigInt(plan.amount))
    }
    for (const payment of scheduledRepayments) {
      if (inside(payment.date) && payment.date > todayString) add(buckets.repayments, payment.debtId, payment.debtName, 'forecast', BigInt(payment.amount))
    }
    for (const subscription of data.subscriptions ?? []) {
      if (!subscription.is_active || !cash.has(subscription.account_id)) continue
      for (const date of subscriptionDates(subscription, fromKey, toKey)) {
        if (inside(date) && date > todayString) add(buckets.expenses, subscription.category_id ?? 'uncategorized', subscription.category_name ?? 'Uncategorized', 'forecast', BigInt(subscription.amount))
      }
    }
    const actualNet = buckets.income.actual - buckets.expenses.actual - buckets.repayments.actual + buckets.other.actual
    const forecastNet = buckets.income.forecast - buckets.expenses.forecast - buckets.repayments.forecast + buckets.other.forecast
    const opening = index === 0 ? currentCash - actualNet : previousClosing
    const closing = opening + actualNet + forecastNet
    const missingExpenses = remainingDays && buckets.expenses.plannedCount === 0
    const missingRepayments = remainingDays && hasDebt && buckets.repayments.plannedCount === 0
    partial ||= missingExpenses || missingRepayments
    previousClosing = closing
    return { from, to, fromKey, toKey, remainingDays, buckets, opening, closing, actualNet, forecastNet, partial, missingExpenses, missingRepayments }
  })
}
