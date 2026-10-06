import { exclusionFor, excludedOnDate, includedCycleCount, type SelectiveDefault } from './selective-defaults'
import { t as translate } from "./i18n"
import { getLocale } from "./i18n"
import { localToday, cardForecasts, installmentCovered, type CardBillingData } from './card-billing'
import { loanSchedule, type LoanContract, type LoanFacility } from './loans'
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
  paid_off_on?: string | null
  id: string; name: string; loan_type: string | null; current_balance: string; monthly_installment: string | null; notes: string | null; is_archived: boolean
}
export interface PlannerInstallment {
  id: string; name: string; debt_account_id: string; debt_account_name: string; debt_account_type: 'credit_card' | 'loan'
  account_name: string; monthly_amount: string; installment_count: number; first_due_date: string; accounts_available: boolean
}
export interface PlannerCreditCard { id: string; name: string; is_archived: boolean }
export interface PlannerCardTransaction {
  loan_account_id?: string | null
  loan_component?: string | null
  account_name?: string
  income_source_id?: string | null
  income_source_name?: string | null
  category_id?: string | null
  category_name?: string | null
  id: string; type: 'income' | 'expense' | 'transfer' | 'repayment'; account_id: string; account_type: string
  destination_account_id: string | null; destination_account_type: string | null; amount: string; date: string
}
export interface PlannerItem {
  actual_key?: string
  credit_card?: PlannerCreditCard
  debt_account?: PlannerDebtAccount
  installment?: PlannerInstallment
  income?: PlannerIncome
  deduction?: IncomeDeduction
  id: string; category_id: PlannerCategoryId; name: string; description: string; card_name: string
  transaction_category_id: string | null; schedule_amount: string | null; schedule_start: string | null; schedule_end: string | null
}
export interface PlannerData {
  selective_defaults?: SelectiveDefault[]
  ledger_transactions?: PlannerCardTransaction[]
  card_billing?: CardBillingData
  loan_contracts?: LoanContract[]
  loan_facilities?: LoanFacility[]
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
  expense_categories: { id: string; name: string; icon?: string | null; is_archived: boolean }[]
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
export function boundary(month: string, day: number) {
  const index = monthIndex(month), year = Math.floor(index / 12), m = index % 12
  const last = new Date(0); last.setFullYear(year, m + 1, 0); last.setHours(12, 0, 0, 0)
  const date = new Date(0); date.setFullYear(year, m, Math.min(day, last.getDate())); date.setHours(12, 0, 0, 0)
  return date
}
export function cycleLabel(month: string, day: number) {
  const start = boundary(month, day), end = boundary(addMonths(month, 1), day)
  end.setDate(end.getDate() - 1)
  const format = new Intl.DateTimeFormat(getLocale(), { day: 'numeric', month: 'short', year: 'numeric' })
  return `${format.format(start)} – ${format.format(end)}`
}
export const monthLabel = (month: string) => boundary(month, 1).toLocaleDateString(getLocale(), { month: 'short', year: 'numeric' })
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
  if (amount === null) return translate('Unavailable')
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
    description: translate("Income source · monthly on day {value0} · {value1}{value2}", { value0: income.recurrence_day_of_month, value1: income.destination_account_name, value2: (!income.is_active ? translate(" · Inactive") : (income.account_archived ? translate(" · Account unavailable") : '')) }),
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
        description: translate("{value0}. {value1}Deducted per salary payment, monthly on day {value2}. {value3} Gross salary remains in Gross Income; this amount is subtracted once here. Manage this deduction on Income.", { value0: deduction.description || deduction.name, value1: (deduction.debt_account_id ? translate("Linked debt: {value0}. Expected payroll repayment; actual debt balance is unchanged. ", { value0: deduction.debt_account_name ?? "Unavailable account" }) : ""), value2: salary.recurrence_day_of_month, value3: (!salary.is_active ? translate("Inactive salary; estimates excluded.") : (salary.account_archived ? translate("Destination unavailable; estimates excluded.") : '')) }) })
    }
    for (const card of data.credit_cards) {
      linked.push({ ...defaults, id: `card:${card.id}`, category_id: 'cards', name: card.name, credit_card: card,
        description: translate("Cash, bank, and wallet repayments/transfers recorded in Transactions, totaled for each payday cycle.{value0} Purchases, refunds and debt-to-debt transfers are excluded. Confirmed bills add remaining dated payment plans and suppress only explicitly included installment occurrences. Without statements, recorded payments retain the legacy installment replacement rule. No installment paid status is inferred.", { value0: (card.is_archived ? translate(" Archived account; historical payments retained.") : '') }) })
    }
    for (const account of data.debt_accounts) {
      const payroll = data.income_deductions.filter(deduction => deduction.debt_account_id === account.id && data.incomes.some(salary => salary.id === deduction.income_id && salary.is_active && !salary.account_archived))
      const legacy = data.installments.some(plan => plan.debt_account_type === 'loan' && plan.debt_account_id === account.id)
      linked.push({ ...defaults, id: `debt:${account.id}`, category_id: 'debt', name: (payroll.length ? translate("{value0} · Additional Payments", { value0: account.name }) : account.name), debt_account: account,
        description: translate("{value0}{value1} · Balance: {value2} THB (positive means owed, negative means credit). {value3} {value4}{value5} Exclude payments already deducted through payroll.", { value0: (payroll.length ? translate("Payroll repayments ({value0}) are entered on the salary and included under Income Deductions. This row is for additional payments outside payroll. Existing entered amounts and schedules are retained; review them for duplicates. ", { value0: payroll.map(deduction => deduction.name).join(', ') }) : ''), value1: account.loan_type?.replaceAll('_', ' ') ?? 'Loan', value2: plannerMoney(BigInt(account.current_balance)), value3: account.notes ?? '', value4: (account.is_archived ? translate("Archived account. ") : ''), value5: (data.loan_facilities?.some(f => f.account_id === account.id) ? translate("Contract schedules replace monthly and legacy schedules. Expand to view contract estimates; an entered account cycle amount replaces the full total. No payments are inferred.") : (account.monthly_installment != null ? translate("Monthly installment: {value0} THB per payday cycle. Replaces legacy loan schedules in this row. Current settings apply to past and future cycles; entered cycle amounts are preserved.", { value0: plannerMoney(BigInt(account.monthly_installment)) }) : (legacy ? translate("Legacy loan schedules are included once in this row. Entering a cycle amount replaces their combined payment.") : translate("Enter the payment for each cycle; the balance is not a payment.")))) }) })
    }
    for (const plan of data.installments.filter(plan => plan.debt_account_type === 'credit_card')) {
      linked.push({ ...defaults, id: `installment:${plan.id}`, category_id: 'installments', name: plan.name, card_name: plan.debt_account_name, installment: plan,
        description: translate("{value0} · Pay from {value1} · {value2} THB per installment · {value3} installments from {value4}.{value5} Schedule dates do not confirm payment. Exclude these installments from other card payments.", { value0: plan.debt_account_name, value1: plan.account_name, value2: plannerMoney(BigInt(plan.monthly_amount)), value3: plan.installment_count, value4: plan.first_due_date, value5: (plan.accounts_available ? '' : translate(" Account unavailable; schedule excluded.")) }) })
    }
  }
  return [...sources, ...linked, ...data.items, ...actualRows(data)]
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
    && (transaction.account_type === 'cash' || transaction.account_type === 'bank' || transaction.account_type === 'wallet')
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
function managedCard(data: PlannerData, id: string) { return !!data.card_billing?.statements.some(s => s.account_id === id) }
export function coveredInstallmentCycle(data: PlannerData, item: PlannerItem, month: string) {
  return !!item.installment && !!data.card_billing?.installments.some(c => c.installment_id === item.installment!.id && installmentSchedule(item.installment!).some(p => p.date === c.date) && c.date >= boundaryKey(month,data.period_start_day) && c.date < boundaryKey(addMonths(month,1),data.period_start_day))
}
function usesRecordedCardTotal(data: PlannerData, item: PlannerItem, month: string) {
  return !!item.installment && !managedCard(data,item.installment.debt_account_id) && recordedCardPayments(data, item.installment.debt_account_id, month, addMonths(month, 1)).count > 0
}
export function cardPlannedBetween(data: PlannerData, cardId: string, from: string, to: string) {
  const start=boundaryKey(from,data.period_start_day),end=boundaryKey(to,data.period_start_day)
  return cardForecasts(data.card_billing).filter(p=>p.card_id===cardId&&p.date>=start&&p.date<end&&!excludedOnDate(data.selective_defaults,cardId,p.date,data.period_start_day)).reduce((n,p)=>n+p.amount,0n)
}
export function loanContractAmount(data: PlannerData, contract: LoanContract, from: string, to = addMonths(from,1)) {
  const start=boundaryKey(from,data.period_start_day), end=boundaryKey(to,data.period_start_day)
  return loanSchedule(contract).filter(p => p.date >= start && p.date < end && !excludedOnDate(data.selective_defaults,contract.account_id,p.date,data.period_start_day)).reduce((n,p) => n+BigInt(p.amount),0n)
}
function linkedPaymentsBetween(data: PlannerData, item: PlannerItem, from: string, to: string): bigint | null {
  if (item.debt_account?.is_archived) return null
  if (item.debt_account && data.loan_facilities?.some(f => f.account_id === item.debt_account!.id)) {
    return (data.loan_contracts ?? []).filter(c => c.account_id === item.debt_account!.id).reduce((n,c) => n + loanContractAmount(data,c,from,to),0n)
  }
  if (item.debt_account?.monthly_installment != null) {
    return BigInt(includedCycleCount(data.selective_defaults, item.debt_account.id, from, to)) * BigInt(item.debt_account.monthly_installment)
  }
  const plans = item.installment ? [item.installment] : data.installments.filter(plan => plan.debt_account_type === 'loan' && plan.debt_account_id === item.debt_account?.id)
  const available = plans.filter(plan => plan.accounts_available)
  if (!available.length) return null
  const start = boundaryKey(from, data.period_start_day), end = boundaryKey(to, data.period_start_day)
  return available.reduce((total, plan) => total + installmentSchedule(plan)
    .filter(payment => payment.date >= start && payment.date < end && !excludedOnDate(data.selective_defaults,plan.debt_account_id,payment.date,data.period_start_day) && !installmentCovered(data.card_billing,plan.id,payment.date))
    .reduce((sum, payment) => sum + BigInt(payment.amount), 0n), 0n)
}
function generatedAmount(data: PlannerData, item: PlannerItem, month: string) {
  if (item.income) return expectedIncomeBetween(item.income, month, addMonths(month, 1), data.period_start_day)
  if (item.debt_account || item.installment) return linkedPaymentsBetween(data, item, month, addMonths(month, 1))
  return scheduledAmount(item, month)
}
function paidOffForCycle(data: PlannerData, item: PlannerItem, month: string) {
  const date = item.debt_account?.paid_off_on
  if (!date) return false
  const [year, m, day] = date.split('-').map(Number)
  return month >= currentCycle(data.period_start_day, new Date(year, m - 1, day, 12))
}
export function itemExclusion(data: PlannerData, item: PlannerItem, month: string) {
  return exclusionFor(data.selective_defaults, item.debt_account?.id ?? item.credit_card?.id ?? item.installment?.debt_account_id, month)
}
function archivedForecastAccount(data: PlannerData, item: PlannerItem) {
  // Paid-off loans retain their separate date-based historical forecast rules.
  if (item.debt_account) return item.debt_account.is_archived && !item.debt_account.paid_off_on
  if (item.credit_card) return item.credit_card.is_archived
  return !!item.installment && [...data.credit_cards, ...data.debt_accounts].some(account => account.id === item.installment!.debt_account_id && account.is_archived)
}
function recordedItemPayments(data: PlannerData, item: PlannerItem, from: string, to: string) {
  if (item.credit_card) return recordedCardPayments(data, item.credit_card.id, from, to).amount
  return item.debt_account ? actualBetween(data, from, to).values.get(`debt:${item.debt_account.id}`) ?? 0n : 0n
}
function forecastItemAmount(data: PlannerData, item: PlannerItem, month: string) {
  if (itemExclusion(data, item, month)) return { value: 0n, source: 'Selective Default' }
  if (archivedForecastAccount(data, item)) return { value: recordedItemPayments(data, item, month, addMonths(month, 1)), source: 'Recorded payments' }
  if (paidOffForCycle(data, item, month)) return { value: 0n, source: 'Loan paid off' }
  if (item.credit_card) {
    const start = boundaryKey(month,data.period_start_day), end = boundaryKey(addMonths(month,1),data.period_start_day)
    const review = data.card_billing?.statements.some(s => s.account_id === item.credit_card!.id && s.needs_review && data.card_billing!.plans.some(p => p.statement_id === s.id && p.date >= start && p.date < end))
    return { value: recordedCardPayments(data, item.credit_card.id, month, addMonths(month, 1)).amount + cardPlannedBetween(data,item.credit_card.id,month,addMonths(month,1)), source: review ? 'Statement needs review' : managedCard(data,item.credit_card.id) ? 'Recorded + remaining plan' : 'Recorded payments' }
  }
  if (coveredInstallmentCycle(data,item,month)) return {value: generatedAmount(data,item,month), source: 'Included in statement'}
  if (usesRecordedCardTotal(data, item, month)) return { value: 0n, source: 'Using recorded card total' }
  const entry = data.amounts.find(a => a.item_id === item.id && a.month === month)
  const generated = generatedAmount(data, item, month)
  return { value: entry ? BigInt(entry.amount) : generated, source: entry ? 'Entered' : generated !== null ? item.deduction ? 'Expected deduction' : item.income ? 'Expected income' : 'Scheduled' : 'Empty' }
}
function forecastCycleTotals(data: PlannerData, month: string) {
  const buckets: Record<PlannerCategoryId, bigint> = { income: 0n, deductions: 0n, debt: 0n, installments: 0n, cards: 0n, expenses: 0n }
  for (const item of plannerItems(data)) buckets[item.category_id] += forecastItemAmount(data, item, month).value ?? 0n
  const netIncome = buckets.income - buckets.deductions
  const expenses = buckets.debt + buckets.installments + buckets.cards + buckets.expenses
  return { buckets, netIncome, expenses, outflows: buckets.deductions + expenses, surplus: netIncome - expenses }
}
// Sum intervening cycles even outside the visible window. Interval arithmetic avoids
// iterating through decades of empty cycles. Explicit entries replace generated values.
function forecastOpeningForCycle(data: PlannerData, month: string): bigint | null {
  if (!data.opening || month < data.opening.month) return null
  let cash = BigInt(data.opening.amount)
  const from = monthIndex(data.opening.month), to = monthIndex(month)
  for (const item of plannerItems(data)) {
    const sign = item.category_id === 'income' ? 1n : -1n
    if (archivedForecastAccount(data, item)) {
      cash -= recordedItemPayments(data, item, data.opening.month, month)
      const accountId = item.debt_account?.id ?? item.credit_card?.id
      for (const p of data.selective_defaults ?? []) if (p.account_id === accountId) {
        const begin = p.start_month > data.opening.month ? p.start_month : data.opening.month
        const end = p.end_month && p.end_month < month ? p.end_month : month
        if (begin < end) cash += recordedItemPayments(data,item,begin,end)
      }
      continue
    }
    if (item.credit_card) {
      cash -= recordedCardPayments(data, item.credit_card.id, data.opening.month, month).amount + cardPlannedBetween(data,item.credit_card.id,data.opening.month,month)
      for (const p of data.selective_defaults ?? []) if (p.account_id === item.credit_card.id) {
        const begin = p.start_month > data.opening.month ? p.start_month : data.opening.month
        const end = p.end_month && p.end_month < month ? p.end_month : month
        if (begin < end) cash += recordedCardPayments(data,item.credit_card.id,begin,end).amount
      }
    } else if (item.income) {
      cash += sign * (expectedIncomeBetween(item.income, data.opening.month, month, data.period_start_day) ?? 0n)
    } else if (item.debt_account || item.installment) {
      cash -= linkedPaymentsBetween(data, item, data.opening.month, month) ?? 0n
      // A recorded full bill payment replaces this card's forecast for the cycle.
      // This is a cashflow display choice, not a claim that an installment was paid.
      if (item.installment && !managedCard(data,item.installment.debt_account_id)) for (const cycle of recordedPaymentCycles(data, item.installment.debt_account_id, data.opening.month, month)) {
        cash += generatedAmount(data, item, cycle) ?? 0n
      }
    } else if (item.schedule_start && item.schedule_end && item.schedule_amount !== null) {
      const count = Math.max(0, Math.min(to, monthIndex(item.schedule_end) + 1) - Math.max(from, monthIndex(item.schedule_start)))
      cash += sign * BigInt(count) * BigInt(item.schedule_amount)
    }
    for (const entry of data.amounts) if (entry.item_id === item.id && entry.month >= data.opening.month && entry.month < month) {
      if (itemExclusion(data, item, entry.month) || paidOffForCycle(data, item, entry.month) || usesRecordedCardTotal(data, item, entry.month) || coveredInstallmentCycle(data,item,entry.month)) continue
      cash += sign * (BigInt(entry.amount) - (generatedAmount(data, item, entry.month) ?? 0n))
    }
  }
  return cash
}
export function plannerCycles(data: PlannerData, start: string) {
  return Array.from({ length: 7 }, (_, i) => {
    const month = addMonths(start, i), totals = cycleTotals(data, month), opening = openingForCycle(data, month)
    return { month, ...totals, opening, closing: opening === null || !totals.available ? null : opening + totals.surplus }
  })
}

export function cycleStatus(data: PlannerData, month: string): CycleStatus {
  return data.months.find(m => m.month === month)?.status ?? (month > currentCycle(data.period_start_day) ? 'forecast' : 'tracking')
}
export function isActualCycle(data: PlannerData, month: string) {
  return data.ledger_transactions !== undefined && cycleStatus(data, month) !== 'forecast'
}
const cashType = (type: string | null) => type !== null && ['cash', 'bank', 'wallet'].includes(type)
const actualIncomeKey = (t: PlannerCardTransaction) => t.income_source_id ? `actual:income:source:${t.income_source_id}` : `actual:income:${t.account_id}`
function linkedLoanCharge(t: PlannerCardTransaction) {
  return t.type === 'expense' && !!t.loan_account_id && (t.loan_component === 'interest' || t.loan_component === 'fee')
}
function actualRows(data: PlannerData): PlannerItem[] {
  if (data.source_currency !== 'THB') return []
  const rows = new Map<string, PlannerItem>()
  for (const t of data.ledger_transactions ?? []) {
    if (!cashType(t.account_type) || !['income', 'expense'].includes(t.type) || linkedLoanCharge(t)) continue
    const key = t.type === 'income' ? actualIncomeKey(t) : `actual:expense:${t.category_id ?? 'uncategorized'}`
    rows.set(key, { id: key, actual_key: key, category_id: t.type === 'income' ? 'income' : 'expenses', name: (t.type === 'income' ? translate("Received · {value0}", { value0: t.income_source_name ?? `Unassigned · ${t.account_name ?? t.account_id}` }) : (t.category_name ?? 'Uncategorized')), description: translate("Actual cash, bank and wallet transactions. Read-only; edit the underlying transactions. Forecast amounts are kept separately."), card_name: '', transaction_category_id: t.type === 'expense' ? t.category_id ?? null : null, schedule_amount: null, schedule_start: null, schedule_end: null })
  }
  return [...rows.values()]
}
function actualBetween(data: PlannerData, from: string, to: string) {
  const values = new Map<string, bigint>()
  const buckets: Record<PlannerCategoryId, bigint> = { income: 0n, deductions: 0n, debt: 0n, installments: 0n, cards: 0n, expenses: 0n }
  const loanSplits = new Map<string, { principal: bigint; interest: bigint; fee: bigint; other: bigint }>()
  let otherIn = 0n, otherOut = 0n
  const start = boundaryKey(from, data.period_start_day), end = boundaryKey(to, data.period_start_day), today = localToday()
  const add = (key: string, bucket: PlannerCategoryId, amount: bigint) => { values.set(key, (values.get(key) ?? 0n) + amount); buckets[bucket] += amount }
  if (data.source_currency === 'THB') for (const t of data.ledger_transactions ?? []) {
    if (t.date < start || t.date >= end || t.date > today) continue
    const source = cashType(t.account_type), destination = cashType(t.destination_account_type), amount = BigInt(t.amount)
    if (source && (linkedLoanCharge(t) || ((t.type === 'repayment' || t.type === 'transfer') && t.destination_account_type === 'loan'))) {
      const loanId = linkedLoanCharge(t) ? t.loan_account_id! : t.destination_account_id!
      const split = loanSplits.get(loanId) ?? { principal: 0n, interest: 0n, fee: 0n, other: 0n }
      if (t.loan_component === 'principal') split.principal += amount
      else if (linkedLoanCharge(t) && t.loan_component === 'interest') split.interest += amount
      else if (linkedLoanCharge(t)) split.fee += amount
      else split.other += amount
      loanSplits.set(loanId, split)
    }
    if (source && linkedLoanCharge(t)) add(`debt:${t.loan_account_id}`, 'debt', amount)
    else if (t.type === 'income' && source) add(actualIncomeKey(t), 'income', amount)
    else if (t.type === 'expense' && source) add(`actual:expense:${t.category_id ?? 'uncategorized'}`, 'expenses', amount)
    else if (t.type === 'transfer' || t.type === 'repayment') {
      if (source && destination) continue
      if (source && t.destination_account_type === 'credit_card') add(`card:${t.destination_account_id}`, 'cards', amount)
      else if (source && t.destination_account_type === 'loan') add(`debt:${t.destination_account_id}`, 'debt', amount)
      else if (source) otherOut += amount
      else if (destination) otherIn += amount
    }
  }
  const expenses = buckets.expenses + buckets.cards + buckets.debt
  return { values, buckets, loanSplits, netIncome: buckets.income, expenses, otherIn, otherOut, outflows: expenses + otherOut, surplus: buckets.income + otherIn - expenses - otherOut }
}
export function loanRepaymentSplit(data: PlannerData, month: string, loanId: string) {
  return actualBetween(data, month, addMonths(month, 1)).loanSplits.get(loanId)
}
export type PlannerView = 'forecast' | 'actual'
export function itemAmount(data: PlannerData, item: PlannerItem, month: string, view?: PlannerView) {
  if (!(view ? view === 'actual' : isActualCycle(data, month))) return item.actual_key ? { value: null, source: 'Actual only' } : forecastItemAmount(data, item, month)
  if (data.source_currency !== 'THB' || data.ledger_transactions === undefined) return { value: null, source: 'Actual data unavailable' }
  const key = item.actual_key ?? (item.credit_card ? `card:${item.credit_card.id}` : item.debt_account ? `debt:${item.debt_account.id}` : null)
  return { value: key ? actualBetween(data, month, addMonths(month, 1)).values.get(key) ?? 0n : null, source: key ? 'Recorded actual' : 'Forecast only' }
}
export function cycleTotals(data: PlannerData, month: string, view?: PlannerView) {
  return (view ? view === 'actual' : isActualCycle(data, month)) ? { ...actualBetween(data, month, addMonths(month, 1)), available: data.source_currency === 'THB' && data.ledger_transactions !== undefined } : { ...forecastCycleTotals(data, month), otherIn: 0n, otherOut: 0n, available: true }
}
export function openingForCycle(data: PlannerData, month: string): bigint | null {
  if (!data.opening || month < data.opening.month) return null
  if (data.ledger_transactions === undefined) return forecastOpeningForCycle(data, month)
  // Partition at explicit mode changes and the current-cycle boundary. This keeps
  // long histories exact without iterating through every calendar month.
  const start = data.opening.month, points = new Set([start, month])
  const split = (m: string) => { if (m > start && m < month) points.add(m) }
  split(addMonths(currentCycle(data.period_start_day), 1))
  for (const status of data.months) { split(status.month); if (status.month < '9999-12') split(addMonths(status.month, 1)) }
  const boundaries = [...points].sort()
  let cash = BigInt(data.opening.amount)
  for (let i = 0; i < boundaries.length - 1; i++) {
    const from = boundaries[i], to = boundaries[i + 1]
    if (isActualCycle(data, from)) {
      if (data.source_currency !== 'THB') return null
      cash += actualBetween(data, from, to).surplus
    } else cash += forecastOpeningForCycle(data, to)! - forecastOpeningForCycle(data, from)!
  }
  return cash
}


// Comparison views are independent of legacy saved cycle statuses. Both views
// begin with the same recorded opening; future projections carry the current
// cycle's full forecast once, never actual + full forecast together.
export function comparisonOpening(data: PlannerData, month: string, active: string): bigint | null {
  if (!data.opening || month < data.opening.month) return null
  const anchor = data.opening.month
  const actualEnd = month < active ? month : active
  let cash = BigInt(data.opening.amount)
  if (actualEnd > anchor) {
    if (data.source_currency !== 'THB' || data.ledger_transactions === undefined) return null
    cash += actualBetween(data, anchor, actualEnd).surplus
  }
  const forecastStart = actualEnd > anchor ? actualEnd : anchor
  if (month > forecastStart) cash += forecastOpeningForCycle(data, month)! - forecastOpeningForCycle(data, forecastStart)!
  return cash
}
export function plannerComparison(data: PlannerData, start: string, active = currentCycle(data.period_start_day)) {
  return Array.from({ length: 7 }, (_, i) => {
    const month = addMonths(start, i), opening = comparisonOpening(data, month, active)
    const values = (view: PlannerView) => {
      const totals = cycleTotals(data, month, view)
      return { ...totals, opening, closing: opening === null || !totals.available ? null : opening + totals.surplus }
    }
    return { month, forecast: values('forecast'), actual: values('actual') }
  })
}
export function comparisonViews(month: string, active: string, comparePast: boolean): PlannerView[] {
  return month === active || (month < active && comparePast) ? ['forecast', 'actual'] : month < active ? ['actual'] : ['forecast']
}
