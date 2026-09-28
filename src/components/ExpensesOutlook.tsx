import { PayeeLabel } from './PayeeLogo'
import { AccountLabel } from './InstitutionLogo'
import { AccountSelect } from './AccountSelect'
import { CategoryIcon } from './CategoryIcon'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { getCoreRowModel, getExpandedRowModel, useReactTable, type ColumnDef, type ExpandedState } from '@tanstack/react-table'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { desktopAvailable } from '../lib/desktop'
import { expenseComparison, getExpenseReport, type ExpenseReport, type ExpenseMethod, type ExpenseRow } from '../lib/expenses'
import { addMonths, currentCycle, cycleLabel, monthLabel } from '../lib/cashflow'
import { dateKey } from '../lib/financial'
import { formatAmount } from '../lib/money'
import { useLocalDate } from '../lib/useLocalDate'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { DataTable } from './ui/data-table'
import { FormField as Field } from './FormField'

export function ExpensesOutlook() {
  const [data, setData] = useState<ExpenseReport | null>(null)
  const [error, setError] = useState(false), [loading, setLoading] = useState(true), [attempt, setAttempt] = useState(0)
  const [selected, setSelected] = useState(''), [accountId, setAccountId] = useState(''), [method, setMethod] = useState<ExpenseMethod>('')
  const today = useLocalDate()
  const request = useRef(0)
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    const load = async () => {
      const id = ++request.current
      setLoading(true)
      try { const result = await getExpenseReport(); if (active && id === request.current) { setData(result); setError(false) } }
      catch { if (active && id === request.current) setError(true) }
      finally { if (active && id === request.current) setLoading(false) }
    }
    void load()
    const events = ['transactions-changed', 'accounts-changed', 'transaction-options-changed', 'focus']
    events.forEach(event => window.addEventListener(event, load))
    return () => { active = false; request.current++; events.forEach(event => window.removeEventListener(event, load)) }
  }, [attempt])
  if (!desktopAvailable) return <p>Open the desktop app to view recorded expenses.</p>
  if (error) return <div role="alert">Could not load expenses. <Button variant="outline" onClick={() => setAttempt(n => n + 1)}>Retry expenses</Button></div>
  if (!data) return <p role="status">Loading expenses…</p>
  if (!data.settings.currency) return <p>Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to view expenses.</p>
  const month = selected || currentCycle(data.settings.period_start_day, today)
  const result = expenseComparison(data, month, dateKey(today), accountId, method)
  const money = (n: bigint) => formatAmount(n.toString(), data.settings.currency!)
  return <section className="min-w-0 space-y-5" aria-labelledby="expenses-title">
    <div><h2 id="expenses-title" className="text-2xl font-semibold">Expenses</h2><p className="mt-2 text-sm">Recorded spending by purchase date, including credit card purchases. Transfers, repayments, bills and forecasts are excluded. These totals are a separate view and are not added to Cashflow Planner.</p></div>
    <div className="flex flex-wrap items-end gap-3">
      <Field label="Expense cycle"><Input type="month" min="0001-01" max="9999-11" value={month} onChange={e => { if (/^\d{4}-(0[1-9]|1[0-2])$/.test(e.target.value) && e.target.value >= '0001-01' && e.target.value <= '9999-11') setSelected(e.target.value) }} /></Field>
      <Button variant="outline" disabled={month <= '0001-01'} onClick={() => setSelected(addMonths(month, -1))}>Previous cycle</Button>
      <Button variant="outline" disabled={month >= '9999-11'} onClick={() => setSelected(addMonths(month, 1))}>Next cycle</Button>
      <Button variant="outline" onClick={() => setSelected('')}>Current cycle</Button>
      <Field label="Expense account"><AccountSelect value={accountId} onChange={e => setAccountId(e.target.value)}><option value="">All accounts</option>{data.accounts.map(a => <option key={a.id} value={a.id}>{a.name}{a.is_archived ? ' (archived)' : ''}</option>)}</AccountSelect></Field>
      <Field label="Payment method"><NativeSelect value={method} onChange={e => setMethod(e.target.value as ExpenseMethod)}><option value="">All methods</option><option value="cash">Cash</option><option value="bank">Bank / debit</option><option value="wallet">Wallet</option><option value="credit_card">Credit card</option><option value="loan">Loan account</option><option value="investment">Investment account</option></NativeSelect></Field>
      {(accountId || method) && <Button variant="ghost" onClick={() => { setAccountId(''); setMethod('') }}>Clear filters</Button>}
    </div>
    <p className="text-sm">Selected cycle: {cycleLabel(month, data.settings.period_start_day)}. Based on recorded data through {dateKey(today)}; an incomplete cycle is not a full-period comparison. Zero means no matching recorded expenses.</p>
    {loading && <p role="status" className="text-sm">Refreshing expenses…</p>}
    <div className="grid gap-3 md:grid-cols-3">{[
      ['Selected cycle spending', money(result.current)],
      ['Previous cycle spending', result.previous === null ? 'Unavailable' : money(result.previous)],
      ['Change from previous cycle', result.difference === null ? 'Unavailable' : `${result.difference > 0n ? '+' : ''}${money(result.difference)}`],
    ].map(([label, value]) => <div key={label} className="rounded-2xl border border-line bg-card p-4"><p className="text-sm text-muted">{label}</p><p className="mt-2 break-words text-xl font-semibold tabular-nums">{value}</p></div>)}</div>
    <p className="text-sm">{result.transactionCount} recorded expense{result.transactionCount === 1 ? '' : 's'} in the selected cycle. The table compares this cycle with up to six earlier cycles. Expand a category for transactions across the displayed periods.</p>
    <ExpenseTable result={result} currency={data.settings.currency} day={data.settings.period_start_day} loading={loading} />
    {!result.rows.length && <p className="rounded-xl bg-soft p-4">No recorded expenses match these accounts, methods and periods.</p>}
    <p className="text-xs">Installment purchases appear once at their recorded purchase amount. Later installment or card payments do not count as new spending. Archived accounts and categories retain their history. Income entries, including credits recorded as income, do not reduce this gross expense total.</p>
  </section>
}

function ExpenseTable({ result, currency, day, loading }: { result: ReturnType<typeof expenseComparison>; currency: string; day: number; loading: boolean }) {
  const [expanded, setExpanded] = useState<ExpandedState>({})
  const rows = useMemo<ExpenseRow[]>(() => [...result.rows, { id: 'total', name: 'Total recorded expenses', amounts: result.totals }], [result])
  const columns: ColumnDef<ExpenseRow>[] = [
    { id: 'category', header: 'Category / transaction', meta: { rowHeader: true, headerClassName: 'sticky left-0 z-20 min-w-52 bg-card px-3 py-2', cellClassName: 'sticky left-0 z-10 min-w-52 max-w-72 border-t border-line bg-card px-3 py-1.5' }, cell: ({ row }) => {
      const t = row.original.transaction
      if (t) return <div className="pl-5 text-xs font-normal"><p className="break-words font-medium">{row.original.name}</p><p className="mt-1 text-muted">{t.date} · <AccountLabel id={t.account_id} name={t.account_name} /></p><p className="text-muted">{t.payee_id ? <PayeeLabel id={t.payee_id} name={t.payee_name ?? 'Payee'} /> : 'No payee'}</p></div>
      return row.getCanExpand() ? <Button variant="ghost" size="xs" className="h-auto min-h-6 w-full justify-start whitespace-normal px-1 py-0.5 text-left text-xs" aria-expanded={row.getIsExpanded()} aria-label={`${row.getIsExpanded() ? 'Collapse' : 'Expand'} ${row.original.name}`} onClick={row.getToggleExpandedHandler()}>{row.getIsExpanded() ? <ChevronDown size={14} /> : <ChevronRight size={14} />}<CategoryIcon name={row.original.icon} />{row.original.name}</Button> : <span className="font-semibold">{row.original.name}</span>
    } },
    ...result.cycles.map((cycle, index) => ({ cycle, index })).reverse().map(({ cycle, index }): ColumnDef<ExpenseRow> => ({ id: cycle.month, header: () => <><span className="block">{monthLabel(cycle.month)}{index === result.cycles.length - 1 ? ' · Selected' : ''}</span><span className="mt-1 block text-[10px] font-normal">{cycleLabel(cycle.month, day)}</span></>, meta: { headerClassName: 'min-w-40 px-3 py-2 text-right', cellClassName: `border-t border-line px-3 py-1.5 text-right tabular-nums whitespace-nowrap ${index === result.cycles.length - 1 ? 'bg-soft' : ''}` }, cell: ({ row }) => row.original.transaction && row.original.amounts[index] === 0n ? '—' : formatAmount(row.original.amounts[index].toString(), currency) })),
  ]
  const table = useReactTable({ data: rows, columns, state: { expanded }, onExpandedChange: setExpanded, getRowId: row => row.id, getSubRows: row => row.children, getCoreRowModel: getCoreRowModel(), getExpandedRowModel: getExpandedRowModel() })
  return <DataTable table={table} label="Expenses by category and payday cycle" className="text-xs" busy={loading} rowClassName={row => row.id === 'total' ? 'font-semibold' : ''} />
}
