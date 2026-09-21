import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { DataTable } from './ui/data-table'
import { useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { ArrowDownLeft, ArrowUpRight, ArrowLeftRight } from 'lucide-react'
import { toast } from 'sonner'
import { deleteTransaction, desktopAvailable, getSettings, listAccounts, listTransactions, type Account, type Transaction, type TransactionType } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { Button } from './ui/button'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const labels = { income: 'Income', expense: 'Expense', transfer: 'Transfer', repayment: 'Repayment' }
const control = 'mt-2 w-full'
function message(error: unknown) { return error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not save changes. Please try again.' }

export function TransactionsPage() {
  const [records, setRecords] = useState<Transaction[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [currency, setCurrency] = useState<string | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<Transaction | null>(null)
  const [filter, setFilter] = useState('')
  const [typeFilter, setTypeFilter] = useState<TransactionType | ''>('')
  const [payeeFilter, setPayeeFilter] = useState('')
  const [categoryFilter, setCategoryFilter] = useState('')
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(), listTransactions()]).then(([settings, accounts, rows]) => {
      if (active) { setCurrency(settings.currency); setAccounts(accounts); setRecords(rows) }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  useEffect(() => {
    const refresh = () => setAttempt(value => value + 1)
    window.addEventListener('transactions-changed', refresh)
    window.addEventListener('transaction-options-changed', refresh)
    return () => { window.removeEventListener('transactions-changed', refresh); window.removeEventListener('transaction-options-changed', refresh) }
  }, [])
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try {
      await deleteTransaction(deleting.id)
      setRecords(rows => rows.filter(row => row.id !== deleting.id)); setDeleting(null)
      toast.success('Transaction deleted. Balances updated.')
    } catch (error) { setError(message(error)) } finally { setSaving(false) }
  }
  const visible = useMemo(() => records.filter(row =>
    (!filter || row.account_id === filter || row.destination_account_id === filter) && (!typeFilter || row.type === typeFilter) &&
    (!payeeFilter || (row.type === 'expense' && (payeeFilter === 'unassigned' ? !row.payee_id : row.payee_id === payeeFilter))) &&
    (!categoryFilter || (row.type === 'expense' && (categoryFilter === 'unassigned' ? !row.category_id : row.category_id === categoryFilter)))), [records, filter, typeFilter, payeeFilter, categoryFilter])
  const spending = visible.filter(row => row.type === 'expense').reduce((total, row) => total + BigInt(row.amount), 0n)
  const payeeOptions = new Map<string, string>()
  const categoryOptions = new Map<string, string>()
  for (const row of records) {
    if (row.payee_id) payeeOptions.set(row.payee_id, row.payee_name!)
    if (row.category_id) categoryOptions.set(row.category_id, row.category_name!)
  }
  const accountOptions = new Map(accounts.map(account => [account.id, account.name]))
  for (const row of records) {
    accountOptions.set(row.account_id, row.account_name)
    if (row.destination_account_id) accountOptions.set(row.destination_account_id, row.destination_account_name!)
  }
  const columns = useMemo<ColumnDef<Transaction>[]>(() => [
    { id: 'date', header: 'Date', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'whitespace-nowrap px-4 py-4 align-top tabular-nums' }, cell: ({ row: { original: row } }) => <><time dateTime={row.date}>{row.date}</time></> },
    { id: 'description', header: 'Description', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'min-w-48 max-w-72 break-words px-4 py-4 align-top font-medium text-ink', rowHeader: true }, cell: ({ row: { original: row } }) => <>{row.description || labels[row.type]}</> },
    { id: 'type', header: 'Type', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-4 align-top' }, cell: ({ row: { original: row } }) => <><TransactionFlow row={row} accountFilter={filter} /></> },
    { id: 'account', header: 'Account', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'min-w-40 max-w-60 break-words px-4 py-4 align-top' }, cell: ({ row: { original: row } }) => <>{row.account_name}{row.destination_account_name && <><span aria-hidden="true"> → </span><span className="sr-only"> to </span>{row.destination_account_name}</>}</> },
    { id: 'payee', header: 'Payee', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'min-w-32 max-w-48 break-words px-4 py-4 align-top' }, cell: ({ row: { original: row } }) => <>{row.type === 'expense' ? row.payee_name || 'No payee' : '—'}</> },
    { id: 'category', header: 'Category', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'min-w-32 max-w-48 break-words px-4 py-4 align-top' }, cell: ({ row: { original: row } }) => <>{row.type === 'expense' ? row.category_name || 'Uncategorized' : '—'}</> },
    { id: 'amount', header: 'Amount', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap text-right', cellClassName: 'whitespace-nowrap px-4 py-4 text-right align-top font-semibold tabular-nums text-ink' }, cell: ({ row: { original: row } }) => <>{row.type === 'income' ? '+' : row.type === 'expense' ? '−' : ''}{currency ? formatAmount(row.amount, currency) : '—'}</> },
    { id: 'actions', header: 'Actions', meta: { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap text-right', cellClassName: 'px-4 py-3 text-right align-top' }, cell: ({ row: { original: row } }) => <><Button size="sm" variant="outline" aria-label={`Delete ${row.description || labels[row.type]}`} onClick={() => { setError(null); setDeleting(row) }}>Delete</Button></> },
  ], [currency, filter])
  const table = useReactTable({ data: visible, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  const clearFilters = () => { setFilter(''); setTypeFilter(''); setPayeeFilter(''); setCategoryFilter('') }
  return <>
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div><h2 className="text-[25px] font-semibold">Your money in motion.</h2><p className="mt-2 text-sm">Record actual activity. Saved transactions update your account balances.</p></div>
      </div>
      {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your local transactions.</p>
        : loading ? <p role="status">Loading transactions…</p>
        : loadError ? <div role="alert">Could not load transactions. <Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Try again</Button></div>
        : !currency ? <p>Choose your currency first in <Link className="text-brand" to="/settings">Settings</Link>.</p>
        : <>
          {!accounts.length && <p className="mb-5">Add an active account in <Link className="text-brand" to="/accounts">Accounts</Link> to record transactions.</p>}
          <div className="mb-5 flex flex-wrap items-end gap-4">
            <div className="w-full sm:w-60"><Field label="Filter by account"><NativeSelect className={control} value={filter} onChange={event => setFilter(event.target.value)}><option value="">All accounts</option>{Array.from(accountOptions).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field></div>
            <div className="w-full sm:w-60"><Field label="Filter by transaction type"><NativeSelect className={control} value={typeFilter} onChange={event => setTypeFilter(event.target.value as TransactionType | '')}><option value="">All types</option><option value="income">Income · In</option><option value="expense">Expense · Out</option><option value="transfer">Transfer</option><option value="repayment">Repayment</option></NativeSelect></Field></div>
            <div className="w-full sm:w-60"><Field label="Filter by payee"><NativeSelect className={control} value={payeeFilter} onChange={event => setPayeeFilter(event.target.value)}><option value="">All payees</option><option value="unassigned">No payee</option>{Array.from(payeeOptions).sort((a, b) => a[1].localeCompare(b[1])).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field></div>
            <div className="w-full sm:w-60"><Field label="Filter by category"><NativeSelect className={control} value={categoryFilter} onChange={event => setCategoryFilter(event.target.value)}><option value="">All categories</option><option value="unassigned">Uncategorized</option>{Array.from(categoryOptions).sort((a, b) => a[1].localeCompare(b[1])).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field></div>
            {(filter || typeFilter || payeeFilter || categoryFilter) && <Button variant="outline" onClick={clearFilters}>Clear filters</Button>}
          </div>
          <p className="mb-5 text-xs">Income is money in; expenses are money out. Transfers and repayments move money between your accounts.{filter && ' Transfer in/out shows the direction for the selected account.'}</p>
          <p className="mb-5 rounded-xl bg-soft p-4 text-sm" aria-label="Filtered spending">Spending in these results: <strong>{formatAmount(spending.toString(), currency)}</strong><span className="mt-1 block text-xs">All recorded dates, matching the selected filters. Expenses only; transfers and repayments are excluded.</span></p>
          {!visible.length ? <div className="rounded-[22px] border border-line bg-card p-10 text-center"><h3 className="font-semibold">{records.length ? 'No matching transactions' : 'No transactions yet'}</h3><p className="mt-2 text-sm">{records.length ? 'Try different filters, or clear them to see all transactions.' : 'Record a payment, purchase, transfer, or repayment. Expected income stays separate.'}</p></div>
            : <DataTable table={table} label="Transaction history" className="min-w-[1100px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" />}
        </>}
    <Dialog open={!!deleting} onOpenChange={next => { if (!next && !saving) { setDeleting(null); setError(null) } }}>
      <DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>Delete transaction?</DialogTitle><DialogDescription>Delete “{deleting && (deleting.description || labels[deleting.type])}” and reverse its effect on account balances. This cannot be undone.</DialogDescription></DialogHeader>{error && <p role="alert">{error}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>Cancel</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? 'Deleting…' : 'Delete transaction'}</Button></DialogFooter></DialogContent>
    </Dialog>
  </>
}

function TransactionFlow({ row, accountFilter }: { row: Transaction; accountFilter: string }) {
  const transfer = row.type === 'transfer' || row.type === 'repayment'
  const direction = transfer ? (accountFilter ? (row.destination_account_id === accountFilter ? 'Transfer in' : 'Transfer out') : 'Transfer') : row.type === 'income' ? 'In' : 'Out'
  const Icon = transfer ? ArrowLeftRight : row.type === 'income' ? ArrowDownLeft : ArrowUpRight
  const tone = transfer ? 'bg-soft text-brand' : row.type === 'income' ? 'bg-green-500/10 text-green-700 dark:text-green-400' : 'bg-red-500/10 text-red-700 dark:text-red-400'
  return <><div className="mb-2">{labels[row.type]}</div><span className={`inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2 py-1 text-xs font-medium ${tone}`}><Icon size={13} aria-hidden="true" />{direction}</span></>
}
