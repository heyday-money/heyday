import { invoke } from '@tauri-apps/api/core'
import type { AccountType, Settings, Transaction } from './desktop'
import { addMonths, boundary, monthIndex } from './cashflow'
import { dateKey } from './financial'

export interface ExpenseAccount { id: string; name: string; type: AccountType; is_archived: boolean }
export interface ExpenseReport { settings: Settings; accounts: ExpenseAccount[]; transactions: Transaction[] }
export const getExpenseReport = () => invoke<ExpenseReport>('get_expense_report')
export type ExpenseMethod = '' | AccountType
export interface ExpenseRow {
  id: string
  name: string
  icon?: string | null
  amounts: bigint[]
  transaction?: Transaction
  children?: ExpenseRow[]
}

// Recorded spending by purchase date only. Plans, transfers and bill payments never enter these totals.
export function expenseComparison(data: ExpenseReport, selected: string, today: string, accountId = '', method: ExpenseMethod = '') {
  const count = Math.min(7, monthIndex(selected) - monthIndex('0001-01') + 1)
  const cycles = Array.from({ length: count }, (_, i) => {
    const month = addMonths(selected, i - count + 1)
    return { month, start: dateKey(boundary(month, data.settings.period_start_day)), end: dateKey(boundary(addMonths(month, 1), data.settings.period_start_day)) }
  })
  const accounts = new Map(data.accounts.map(a => [a.id, a]))
  const totals = cycles.map(() => 0n)
  const groups = new Map<string, ExpenseRow>()
  let transactionCount = 0
  for (const t of data.transactions) {
    if (t.type !== 'expense' || t.date > today || (accountId && t.account_id !== accountId) || (method && accounts.get(t.account_id)?.type !== method)) continue
    const index = cycles.findIndex(c => t.date >= c.start && t.date < c.end)
    if (index < 0) continue
    const amount = BigInt(t.amount)
    totals[index] += amount
    if (index === cycles.length - 1) transactionCount++
    const key = t.category_id ? `category:${t.category_id}` : 'uncategorized'
    const group = groups.get(key) ?? { id: key, name: t.category_name ?? 'Uncategorized', icon: t.category_icon, amounts: cycles.map(() => 0n), children: [] }
    group.amounts[index] += amount
    group.children!.push({ id: `transaction:${t.id}`, name: t.description || 'Expense', transaction: t, amounts: cycles.map((_, i) => i === index ? amount : 0n) })
    groups.set(key, group)
  }
  const rows = [...groups.values()].sort((a, b) => a.name.localeCompare(b.name))
  for (const row of rows) row.children!.sort((a, b) => b.transaction!.date.localeCompare(a.transaction!.date) || a.id.localeCompare(b.id))
  const current = totals.at(-1) ?? 0n, previous = totals.at(-2) ?? null
  return { cycles, rows, totals, current, previous, difference: previous === null ? null : current - previous, transactionCount }
}
