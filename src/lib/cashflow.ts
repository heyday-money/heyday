import { invoke } from '@tauri-apps/api/core'
import { decimalToInteger } from './money'
import { currentPeriod } from './period'
import { installmentSchedule } from './installments'
import type { IncomeDeduction } from './desktop'

export type PlannerCategoryId = 'income' | 'deductions' | 'debt' | 'installments' | 'cards' | 'expenses'
export type CycleStatus = 'tracking' | 'forecast' | 'complete'
export interface PlannerIncome {
  id: string; name: string; estimated_amount: string; recurrence_day_of_month: number
  is_active: boolean; destination_account_name: string; account_archived: boolean
}
export interface PlannerDebtAccount {
  id: string; name: string; loan_type: string | null; current_balance: string; notes: string | null; is_archived: boolean
}
export interface PlannerInstallment {
  id: string; name: string; debt_account_id: string; debt_account_name: string; debt_account_type: 'credit_card' | 'loan'
  account_name: string; monthly_amount: string; installment_count: number; first_due_date: string; accounts_available: boolean
}
export interface PlannerCreditCard { id: string; name: string; is_archived: boolean }
export interface PlannerCardTransaction {
  id: string; type: 'income' | 'expense' | 'transfer' | 'repayment'; account_id: string; account_type: string
  destination_account_id: string | null; destination_account_type: string | null; amount: string; date: string
}
export interface PlannerItem {
  credit_card?: PlannerCreditCard
  debt_account?: PlannerDebtAccount
  installment?: PlannerInstallment
  income?: PlannerIncome
  deduction?: IncomeDeduction
  id: string; category_id: PlannerCategoryId; name: string; description: string; card_name: string
  transaction_category_id: string | null; schedule_amount: string | null; schedule_start: string | null; schedule_end: string | null
}
export interface PlannerData {
  incomes: PlannerIncome[]
  income_deductions: IncomeDeduction[]
  source_currency: string | null
  debt_accounts: PlannerDebtAccount[]
  installments: PlannerInstallment[]
  credit_cards: PlannerCreditCard[]
  card_transactions: PlannerCardTransaction[]
  categories: { id: PlannerCategoryId; name: string; subtotal: string }[]
  items: PlannerItem[]
  amounts: { item_id: string; month: string; amount: string }[]
  months: { month: string; status: CycleStatus }[]
  opening: { month: string; amount: string } | null
  period_start_day: number
  expense_categories: { id: string; name: string; is_archived: boolean }[]
}
export type PlannerChange =
  | ({ kind: 'item' } & Omit<PlannerItem, 'id' | 'income' | 'debt_account' | 'installment' | 'credit_card' | 'deduction'> & { id: string | null })
  | { kind: 'delete'; id: string }
  | { kind: 'amount'; item_id: string; month: string; amount: string | null }
  | { kind: 'status'; month: string; status: CycleStatus }
  | { kind: 'opening'; month: string; amount: string | null }
export const getPlanner = () => invoke<PlannerData>('get_cashflow_planner')
export const savePlanner = (input: PlannerChange) => invoke<PlannerData>('save_cashflow_planner', { input })
export function monthIndex(month: string) {
  if (!/^\d{4}-(0[1-9]|1[0-2])$/.test(month) || month < '0001-01') throw new Error('Choose a valid cycle month.')
  return Number(month.slice(0, 4)) * 12 + Number(month.slice(5)) - 1
}
export function addMonths(month: string, count: number) {
  const index = monthIndex(month) + count
  if (!Number.isInteger(count) || index < 12 || index > 119999) throw new Error('Cycle is outside years 0001–9999.')
  return `${Math.floor(index / 12).toString().padStart(4, '0')}-${(index % 12 + 1).toString().padStart(2, '0')}`
}
export function currentCycle(day: number, today = new Date()) {
  const { start } = currentPeriod(day, today)
  return `${start.getFullYear().toString().padStart(4, '0')}-${(start.getMonth() + 1).toString().padStart(2, '0')}`
}
function boundary(month: string, day: number) {
  const index = monthIndex(month), year = Math.floor(index / 12), m = index % 12
  const last = new Date(0); last.setFullYear(year, m + 1, 0); last.setHours(12, 0, 0, 0)
  const date = new Date(0); date.setFullYear(year, m, Math.min(day, last.getDate())); date.setHours(12, 0, 0, 0)
  return date
}
export function cycleLabel(month: string, day: number) {
  const start = boundary(month, day), end = boundary(addMonths(month, 1), day)
  end.setDate(end.getDate() - 1)
  const format = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
  return `${format.format(start)} – ${format.format(end)}`
}
export const monthLabel = (month: string) => boundary(month, 1).toLocaleDateString('en-GB', { month: 'short', year: 'numeric' })
export function parsePlannerAmount(value: string, signed = false) {
  const amount = decimalToInteger(value, 2)
  if (!signed && BigInt(amount) < 0n) throw new Error('Amount cannot be negative.')
  return amount
}
export function editableSatang(value: string | null) {
  if (value === null) return ''
  const amount = BigInt(value), abs = amount < 0n ? -amount : amount
  return `${amount < 0n ? '-' : ''}${abs / 100n}.${(abs % 100n).toString().padStart(2, '0')}`
}
export function plannerMoney(amount: bigint | null) {
  if (amount === null) return 'Unavailable'
  const abs = amount < 0n ? -amount : amount
  return `${amount < 0n ? '−' : ''}${(abs / 100n).toLocaleString('en-US')}.${(abs % 100n).toString().padStart(2, '0')}`
}
export function scheduledAmount(item: PlannerItem, month: string): bigint | null {
  return item.schedule_amount !== null && item.schedule_start && item.schedule_end && month >= item.schedule_start && month <= item.schedule_end ? BigInt(item.schedule_amount) : null
}
// Source rows are derived from the same SQLite snapshot, never copied into planner_items.
export function plannerItems(data: PlannerData): PlannerItem[] {
  const sources: PlannerItem[] = data.source_currency === 'THB' ? data.incomes.map(income => ({
    id: `income:${income.id}`, category_id: 'income', name: income.name,
    description: `Income source · monthly on day ${income.recurrence_day_of_month} · ${income.destination_account_name}${!income.is_active ? ' · Inactive' : income.account_archived ? ' · Account unavailable' : ''}`,
    card_name: '', transaction_category_id: null, schedule_amount: null, schedule_start: null, schedule_end: null, income,
  })) : []
  const linked: PlannerItem[] = []
  if (data.source_currency === 'THB') {
    const defaults = { card_name: '', transaction_category_id: null, schedule_amount: null, schedule_start: null, schedule_end: null }
    for (const deduction of data.income_deductions) {
      const salary = data.incomes.find(income => income.id === deduction.income_id)
      if (!salary) continue
      linked.push({ ...defaults, id: `deduction:${deduction.id}`, category_id: 'deductions', name: `${salary.name} · ${deduction.name}`, deduction,
        income: { ...salary, estimated_amount: deduction.amount },
        description: `${deduction.description || deduction.name}. Deducted per salary payment, monthly on day ${salary.recurrence_day_of_month}. ${!salary.is_active ? 'Inactive salary; estimates excluded.' : salary.account_archived ? 'Destination unavailable; estimates excluded.' : ''} Gross salary remains in Gross Income; this amount is subtracted once here. Manage this deduction on Income.` })
    }
    for (const card of data.credit_cards) {
      linked.push({ ...defaults, id: `card:${card.id}`, category_id: 'cards', name: card.name, credit_card: card,
        description: `Cash and bank repayments/transfers recorded in Transactions, totaled for each payday cycle.${card.is_archived ? ' Archived account; historical payments retained.' : ''} Purchases, refunds and debt-to-debt transfers are excluded. Where payments exist, this recorded total replaces this card's linked installment forecasts for that cycle; no installment split or paid status is inferred.` })
    }
    for (const account of data.debt_accounts) {
      const legacy = data.installments.some(plan => plan.debt_account_type === 'loan' && plan.debt_account_id === account.id)
      linked.push({ ...defaults, id: `debt:${account.id}`, category_id: 'debt', name: account.name, debt_account: account,
        description: `${account.loan_type?.replaceAll('_', ' ') ?? 'Loan'} · Balance: ${plannerMoney(BigInt(account.current_balance))} THB (positive means owed, negative means credit). ${account.notes ?? ''} ${account.is_archived ? 'Archived account. ' : ''}${legacy ? 'Legacy loan schedules are included once in this row. Entering a cycle amount replaces their combined payment.' : 'Enter the payment for each cycle; the balance is not a payment.'} Exclude payments already deducted through payroll.` })
    }
    for (const plan of data.installments.filter(plan => plan.debt_account_type === 'credit_card')) {
      linked.push({ ...defaults, id: `installment:${plan.id}`, category_id: 'installments', name: plan.name, card_name: plan.debt_account_name, installment: plan,
        description: `${plan.debt_account_name} · Pay from ${plan.account_name} · ${plannerMoney(BigInt(plan.monthly_amount))} THB per installment · ${plan.installment_count} installments from ${plan.first_due_date}.${plan.accounts_available ? '' : ' Account unavailable; schedule excluded.'} Schedule dates do not confirm payment. Exclude these installments from other card payments.` })
    }
  }
  return [...sources, ...linked, ...data.items]
}
function expectedIncomeBetween(income: PlannerIncome, from: string, to: string, payday: number): bigint | null {
  if (!income.is_active || income.account_archived) return null
  const day = income.recurrence_day_of_month
  // Count monthly occurrences in [start, end), clamping each calendar month independently.
  const count = monthIndex(to) - monthIndex(from)
    - (boundary(from, day) < boundary(from, payday) ? 1 : 0)
    + (boundary(to, day) < boundary(to, payday) ? 1 : 0)
  return BigInt(count) * BigInt(income.estimated_amount)
}
function boundaryKey(month: string, day: number) {
  const date = boundary(month, day)
  return `${month}-${String(date.getDate()).padStart(2, '0')}`
}
function isCardPayment(transaction: PlannerCardTransaction, cardId: string) {
  return (transaction.type === 'repayment' || transaction.type === 'transfer')
    && (transaction.account_type === 'cash' || transaction.account_type === 'bank')
    && transaction.destination_account_id === cardId && transaction.destination_account_type === 'credit_card'
}
export function recordedCardPayments(data: PlannerData, cardId: string, from: string, to: string) {
  const start = boundaryKey(from, data.period_start_day), end = boundaryKey(to, data.period_start_day)
  const payments = data.card_transactions.filter(transaction => isCardPayment(transaction, cardId) && transaction.date >= start && transaction.date < end)
  return { amount: payments.reduce((sum, transaction) => sum + BigInt(transaction.amount), 0n), count: payments.length }
}
function recordedPaymentCycles(data: PlannerData, cardId: string, from: string, to: string) {
  const start = boundaryKey(from, data.period_start_day), end = boundaryKey(to, data.period_start_day)
  return new Set(data.card_transactions.filter(transaction => isCardPayment(transaction, cardId) && transaction.date >= start && transaction.date < end).map(transaction => {
    const month = transaction.date.slice(0, 7)
    return transaction.date >= boundaryKey(month, data.period_start_day) ? month : addMonths(month, -1)
  }))
}
function usesRecordedCardTotal(data: PlannerData, item: PlannerItem, month: string) {
  return !!item.installment && recordedCardPayments(data, item.installment.debt_account_id, month, addMonths(month, 1)).count > 0
}
function linkedPaymentsBetween(data: PlannerData, item: PlannerItem, from: string, to: string): bigint | null {
  if (item.debt_account?.is_archived) return null
  const plans = item.installment ? [item.installment] : data.installments.filter(plan => plan.debt_account_type === 'loan' && plan.debt_account_id === item.debt_account?.id)
  const available = plans.filter(plan => plan.accounts_available)
  if (!available.length) return null
  const start = boundaryKey(from, data.period_start_day), end = boundaryKey(to, data.period_start_day)
  return available.reduce((total, plan) => total + installmentSchedule(plan)
    .filter(payment => payment.date >= start && payment.date < end)
    .reduce((sum, payment) => sum + BigInt(payment.amount), 0n), 0n)
}
function generatedAmount(data: PlannerData, item: PlannerItem, month: string) {
  if (item.income) return expectedIncomeBetween(item.income, month, addMonths(month, 1), data.period_start_day)
  if (item.debt_account || item.installment) return linkedPaymentsBetween(data, item, month, addMonths(month, 1))
  return scheduledAmount(item, month)
}
export function itemAmount(data: PlannerData, item: PlannerItem, month: string) {
  if (item.credit_card) return { value: recordedCardPayments(data, item.credit_card.id, month, addMonths(month, 1)).amount, source: 'Recorded payments' }
  if (usesRecordedCardTotal(data, item, month)) return { value: 0n, source: 'Using recorded card total' }
  const entry = data.amounts.find(a => a.item_id === item.id && a.month === month)
  const generated = generatedAmount(data, item, month)
  return { value: entry ? BigInt(entry.amount) : generated, source: entry ? 'Entered' : generated !== null ? item.deduction ? 'Expected deduction' : item.income ? 'Expected income' : 'Scheduled' : 'Empty' }
}
export function cycleTotals(data: PlannerData, month: string) {
  const buckets: Record<PlannerCategoryId, bigint> = { income: 0n, deductions: 0n, debt: 0n, installments: 0n, cards: 0n, expenses: 0n }
  for (const item of plannerItems(data)) buckets[item.category_id] += itemAmount(data, item, month).value ?? 0n
  const netIncome = buckets.income - buckets.deductions
  const expenses = buckets.debt + buckets.installments + buckets.cards + buckets.expenses
  return { buckets, netIncome, expenses, outflows: buckets.deductions + expenses, surplus: netIncome - expenses }
}
// Sum intervening cycles even outside the visible window. Interval arithmetic avoids
// iterating through decades of empty cycles. Explicit entries replace generated values.
export function openingForCycle(data: PlannerData, month: string): bigint | null {
  if (!data.opening || month < data.opening.month) return null
  let cash = BigInt(data.opening.amount)
  const from = monthIndex(data.opening.month), to = monthIndex(month)
  for (const item of plannerItems(data)) {
    const sign = item.category_id === 'income' ? 1n : -1n
    if (item.credit_card) {
      cash -= recordedCardPayments(data, item.credit_card.id, data.opening.month, month).amount
    } else if (item.income) {
      cash += sign * (expectedIncomeBetween(item.income, data.opening.month, month, data.period_start_day) ?? 0n)
    } else if (item.debt_account || item.installment) {
      cash -= linkedPaymentsBetween(data, item, data.opening.month, month) ?? 0n
      // A recorded full bill payment replaces this card's forecast for the cycle.
      // This is a cashflow display choice, not a claim that an installment was paid.
      if (item.installment) for (const cycle of recordedPaymentCycles(data, item.installment.debt_account_id, data.opening.month, month)) {
        cash += generatedAmount(data, item, cycle) ?? 0n
      }
    } else if (item.schedule_start && item.schedule_end && item.schedule_amount !== null) {
      const count = Math.max(0, Math.min(to, monthIndex(item.schedule_end) + 1) - Math.max(from, monthIndex(item.schedule_start)))
      cash += sign * BigInt(count) * BigInt(item.schedule_amount)
    }
    for (const entry of data.amounts) if (entry.item_id === item.id && entry.month >= data.opening.month && entry.month < month) {
      if (usesRecordedCardTotal(data, item, entry.month)) continue
      cash += sign * (BigInt(entry.amount) - (generatedAmount(data, item, entry.month) ?? 0n))
    }
  }
  return cash
}
export function plannerCycles(data: PlannerData, start: string) {
  return Array.from({ length: 7 }, (_, i) => {
    const month = addMonths(start, i), totals = cycleTotals(data, month), opening = openingForCycle(data, month)
    return { month, ...totals, opening, closing: opening === null ? null : opening + totals.surplus }
  })
}
