import { AddTransactionDialog } from './AddTransactionDialog'
import { SalaryPaymentHistory } from './SalaryPaymentDialog'
import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { TransactionPayee } from './TransactionPayee'
import { transactionPayeeKey } from '../lib/transaction-payee'
import { AccountLabel } from './InstitutionLogo'
import { AccountSelect } from './AccountSelect'
import { CategoryIcon } from './CategoryIcon'
import { getCoreRowModel, getPaginationRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { DataTable } from './ui/data-table'
import { useEffect, useMemo, useState } from 'react'
import { Link, useSearch } from '@tanstack/react-router'
import { ArrowDownLeft, ArrowUpRight, ArrowLeftRight, ChevronRight } from 'lucide-react'
import { toast } from 'sonner'
import { deleteTransaction, desktopAvailable, getSettings, listAccounts, listTransactions, type Account, type Transaction, type TransactionType } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const labels = { get income() { return translate("Income") }, get expense() { return translate("Expense") }, get transfer() { return translate("Transfer") }, get repayment() { return translate("Repayment") } }
const control = 'mt-2 w-full'
function moneyFlow(row: Transaction, accountFilter: string): 'in' | 'out' | 'transfer' {
  if (row.salary_payment_role === 'principal') return 'transfer'
  if (row.type === 'income') return 'in'
  if (row.type === 'expense') return 'out'
  return accountFilter ? row.destination_account_id === accountFilter ? 'in' : 'out' : 'transfer'
}
function message(error: unknown) { return error instanceof Error ? error.message : typeof error === 'string' ? error : "Could not save changes. Please try again." }

export function TransactionsPage() {
  const { account } = useSearch({ from: '/transactions' })
  return <TransactionHistory accountId={account} />
}

/** Account details and the full history share filtering and financial correction safeguards. */
export function TransactionHistory({ accountId: historyAccount, embedded = false }: { accountId?: string; embedded?: boolean }) {
  useLanguage()

  const [payrollHistory, setPayrollHistory] = useState(false)
  const [salaryPayment, setSalaryPayment] = useState<string | null>(null)
  const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null)
  const [records, setRecords] = useState<Transaction[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [currency, setCurrency] = useState<string | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirmReconciled, setConfirmReconciled] = useState(false)
  const [needsConfirmation, setNeedsConfirmation] = useState(false)
  const [deleting, setDeleting] = useState<Transaction | null>(null)
  const [selectedAccount, setFilter] = useState(historyAccount ?? '')
  const filter = embedded ? historyAccount ?? '' : selectedAccount
  useEffect(() => { setFilter(historyAccount ?? '') }, [historyAccount])
  const [typeFilter, setTypeFilter] = useState<TransactionType | ''>('')
  const [payeeFilter, setPayeeFilter] = useState('')
  const [categoryFilter, setCategoryFilter] = useState('')
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(true, true), listTransactions()]).then(([settings, accounts, rows]) => {
      if (active) { setCurrency(settings.currency); setAccounts(accounts); setRecords(rows) }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  useEffect(() => {
    const refresh = () => setAttempt(value => value + 1)
    const events = ['transactions-changed', 'accounts-changed', 'verification-changed', 'transaction-options-changed', 'focus']
    events.forEach(event => window.addEventListener(event, refresh))
    return () => events.forEach(event => window.removeEventListener(event, refresh))
  }, [])
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try {
      await deleteTransaction(deleting.id, confirmReconciled)
      setRecords(await listTransactions()); setDeleting(null)
      toast.success(translate("Transaction deleted. Balances updated."))
    } catch (error) { setError(message(error)); if (message(error).includes('reconciliation history')) setNeedsConfirmation(true) } finally { setSaving(false) }
  }
  const accountRecords = useMemo(() => records.filter(row =>
    !filter || row.account_id === filter || row.destination_account_id === filter || row.loan_account_id === filter), [records, filter])
  const visible = useMemo(() => accountRecords.filter(row =>
    (!typeFilter || (row.salary_payment_role === 'principal' ? 'repayment' : row.type) === typeFilter) &&
    (!payeeFilter || (payeeFilter === 'unassigned' ? row.type === 'expense' && !transactionPayeeKey(row) : transactionPayeeKey(row) === payeeFilter)) &&
    (!categoryFilter || (row.type === 'expense' && (categoryFilter === 'unassigned' ? !row.category_id : row.category_id === categoryFilter)))), [accountRecords, typeFilter, payeeFilter, categoryFilter, getLanguage()])
  const spending = visible.filter(row => row.type === 'expense').reduce((total, row) => total + BigInt(row.amount), 0n)
  const payeeOptions = new Map<string, string>()
  const categoryOptions = new Map<string, string>()
  for (const row of embedded ? accountRecords : records) {
    const recipient = transactionPayeeKey(row)
    if (recipient) payeeOptions.set(recipient, row.payee_id ? row.payee_name! : row.loan_account_name!)
    if (row.category_id) categoryOptions.set(row.category_id, row.category_name!)
  }
  const accountOptions = new Map(accounts.map(account => [account.id, account.name]))
  for (const row of records) {
    accountOptions.set(row.account_id, row.account_name)
    if (row.loan_account_id) accountOptions.set(row.loan_account_id, row.loan_account_name!)
    if (row.destination_account_id) accountOptions.set(row.destination_account_id, row.destination_account_name!)
  }
  const columns = useMemo<ColumnDef<Transaction>[]>(() => [
    { id: 'date', header: translate("Date"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'whitespace-nowrap px-3 py-1.5 align-middle tabular-nums' }, cell: ({ row: { original: row } }) => <><time dateTime={row.date}>{row.date}</time></> },
    { id: 'description', header: translate("Description"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'min-w-40 max-w-72 break-words px-3 py-1.5 align-middle font-medium text-ink', rowHeader: true }, cell: ({ row: { original: row } }) => <>{row.description || labels[row.type]}{row.salary_payment_id && <Button size="xs" variant="link" onClick={() => setSalaryPayment(row.salary_payment_id!)}>{translate("Payslip breakdown")}</Button>}{row.type === 'income' && row.income_source_name && <span className="block text-xs font-normal text-muted">{row.income_source_name}</span>}</> },
    { id: 'type', header: translate("Type"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'px-3 py-1.5 align-middle' }, cell: ({ row: { original: row } }) => <><TransactionFlow row={row} accountFilter={filter} /></> },
    { id: 'account', header: translate("Account"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'min-w-36 max-w-60 break-words px-3 py-1.5 align-middle' }, cell: ({ row: { original: row } }) => <><AccountLabel id={row.account_id} name={row.account_name} />{row.destination_account_name && <><span aria-hidden="true"> → </span><span className="sr-only"> {" "}{translate("to")}{" "}</span><AccountLabel id={row.destination_account_id} name={row.destination_account_name} /></>}</> },
    { id: 'payee', header: translate("Payee"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'min-w-28 max-w-48 break-words px-3 py-1.5 align-middle' }, cell: ({ row: { original: row } }) => <TransactionPayee transaction={row} /> },
    { id: 'category', header: translate("Category"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap', cellClassName: 'min-w-28 max-w-48 break-words px-3 py-1.5 align-middle' }, cell: ({ row: { original: row } }) => <>{row.type === 'expense' ? <span className="inline-flex items-center gap-2"><CategoryIcon name={row.category_icon} />{row.category_name || translate("Uncategorized")}</span> : '—'}</> },
    { id: 'amount', header: translate("Amount"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap text-right', cellClassName: 'whitespace-nowrap px-3 py-1.5 text-right align-middle font-semibold tabular-nums text-ink' }, cell: ({ row: { original: row } }) => {
      const flow = moneyFlow(row, filter)
      return <span className={flow === 'in' ? 'text-green-700 dark:text-green-400' : flow === 'out' ? 'text-red-700 dark:text-red-400' : 'text-ink'}>{row.salary_payment_role === 'principal' ? '−' : row.type === 'income' ? '+' : row.type === 'expense' ? '−' : ''}{currency ? formatAmount(row.amount, currency) : '—'}</span>
    } },
    { id: 'actions', header: translate("Actions"), meta: { headerClassName: 'px-3 py-2 font-semibold whitespace-nowrap text-right', cellClassName: 'px-3 py-1.5 text-right align-middle' }, cell: ({ row: { original: row } }) => <><Button size="xs" variant="outline" className="mr-2" title={row.has_reconciliation_history ? translate("Transactions with reconciliation history cannot be edited.") : row.salary_payment_id || row.loan_account_id || row.imported_linked_payment ? translate("Linked salary, loan, and installment entries cannot be edited here. Open their source record to review them.") : translate("Edit transaction")} aria-label={translate("Edit {value0}", { value0: row.description || labels[row.type] })} disabled={!!row.has_reconciliation_history || !!row.salary_payment_id || !!row.loan_account_id || !!row.imported_linked_payment} onClick={() => setEditingTransaction(row)}>{translate("Edit")}</Button><Button size="xs" variant="outline" aria-label={translate("Delete {value0}", { value0: row.description || labels[row.type] })} onClick={() => { setError(null); setConfirmReconciled(false); setNeedsConfirmation(!!row.has_reconciliation_history); setDeleting(row) }}>{translate("Delete")}</Button></> },
  ], [currency, filter, getLanguage()])
  const table = useReactTable({ data: visible, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: embedded ? getPaginationRowModel() : undefined, initialState: { pagination: { pageSize: 10, pageIndex: 0 } } })
  const clearFilters = () => { if (!embedded) setFilter(''); setTypeFilter(''); setPayeeFilter(''); setCategoryFilter('') }
  const historyAccountDetails = accounts.find(account => account.id === historyAccount)
  return <>
    {editingTransaction && <AddTransactionDialog editing={editingTransaction} onClose={() => setEditingTransaction(null)} />}
      {!embedded && historyAccount && <nav aria-label={translate("Breadcrumb")} className="mb-4 text-sm">
        <ol className="flex flex-wrap items-center gap-2 text-muted">
          <li><Link to="/accounts" className="text-brand hover:underline">{translate("Accounts")}</Link></li>
          <li aria-hidden="true"><ChevronRight size={14} /></li>
          <li className="min-w-0 break-words"><Link to="/accounts/$accountId/details" params={{ accountId: historyAccount }} className="text-brand hover:underline">
            {accountOptions.get(historyAccount) || translate("Account details")}{historyAccountDetails?.last_four && ` · •••• ${historyAccountDetails.last_four}`}
          </Link></li>
          <li aria-hidden="true"><ChevronRight size={14} /></li>
          <li aria-current="page">{translate("Transaction history")}</li>
        </ol>
      </nav>}
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        {embedded ? <>
          <div><h3 className="text-lg font-semibold">{translate("Transaction history")}</h3><p className="mt-1 text-sm text-muted">{translate("All recorded activity for this account, newest first.")}</p></div>
          <Button asChild variant="outline" size="sm"><Link to="/transactions" search={{ account: historyAccount }}>{translate("View all transactions")}</Link></Button>
        </> : <div><h2 className="text-2xl font-semibold">{translate("Your money in motion.")}</h2><p className="mt-2 text-sm">{translate("Record actual activity. Saved transactions update your account balances.")}</p></div>}
      </div>
      {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage your local transactions.")}</p>
        : loading ? <p role="status">{translate("Loading transactions…")}</p>
        : loadError ? <div role="alert">{translate("Could not load transactions.")}{" "}<Button variant="outline" onClick={() => setAttempt(value => value + 1)}>{translate("Try again")}</Button></div>
        : !currency ? <p>{translate("Choose your currency first in")}{" "}<Link className="text-brand" to="/settings">{translate("Settings")}</Link>.</p>
        : <>
          {!embedded && !accounts.some(account => !account.is_archived) && <p className="mb-5">{translate("Add an active account in")}{" "}<Link className="text-brand" to="/accounts">{translate("Accounts")}</Link> {" "}{translate("to record transactions.")}</p>}
          <div className="mb-5 flex flex-wrap items-end gap-4">
            {!embedded && <div className="w-full sm:w-60"><Field label={translate("Filter by account")}><AccountSelect className={control} value={filter} onChange={event => setFilter(event.target.value)}><option value="">{translate("All accounts")}</option>{Array.from(accountOptions).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</AccountSelect></Field></div>}
            <div className="w-full sm:w-60"><Field label={translate("Filter by transaction type")}><NativeSelect className={control} value={typeFilter} onChange={event => setTypeFilter(event.target.value as TransactionType | '')}><option value="">{translate("All types")}</option><option value="income">{translate("Income · In")}</option><option value="expense">{translate("Expense · Out")}</option><option value="transfer">{translate("Transfer")}</option><option value="repayment">{translate("Repayment")}</option></NativeSelect></Field></div>
            <div className="w-full sm:w-60"><Field label={translate("Filter by payee")}><NativeSelect className={control} value={payeeFilter} onChange={event => setPayeeFilter(event.target.value)}><option value="">{translate("All payees")}</option><option value="unassigned">{translate("No payee")}</option>{Array.from(payeeOptions).sort((a, b) => a[1].localeCompare(b[1])).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field></div>
            <div className="w-full sm:w-60"><Field label={translate("Filter by category")}><NativeSelect className={control} value={categoryFilter} onChange={event => setCategoryFilter(event.target.value)}><option value="">{translate("All categories")}</option><option value="unassigned">{translate("Uncategorized")}</option>{Array.from(categoryOptions).sort((a, b) => a[1].localeCompare(b[1])).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field></div>
            {((!embedded && filter) || typeFilter || payeeFilter || categoryFilter) && <Button variant="outline" onClick={clearFilters}>{translate("Clear filters")}</Button>}
          </div>
          {!embedded && <div className="mb-4 flex flex-wrap items-center gap-3">{accounts.some(account => account.id === filter && !account.is_archived && ['bank', 'wallet', 'credit_card', 'loan'].includes(account.type))
            ? <><Button asChild><Link to="/accounts/$accountId" params={{ accountId: filter }} search={{ reconcile: true }}>{translate("Reconcile account")}</Link></Button><p className="text-xs">{translate("Reconcile all transactions for this account, regardless of the other filters.")}</p></>
            : <><Button disabled>{translate("Reconcile account")}</Button><p className="text-xs">{translate("Select a bank, digital wallet, credit card, or loan account to reconcile.")}</p></>}
          </div>}
          <p className="mb-5 text-xs">{translate("Income is money in; expenses are money out. Transfers and repayments move money between your accounts.")}{filter && translate(" Transfer in/out shows the direction for the selected account.")}</p>
          {!embedded && <p className="mb-5 rounded-xl bg-soft p-4 text-sm" aria-label={translate("Filtered spending")}>{translate("Spending in these results:")}{" "}<strong>{formatAmount(spending.toString(), currency)}</strong><span className="mt-1 block text-xs">{translate("All recorded dates, matching the selected filters. Expenses only; transfers and repayments are excluded.")}</span></p>}
          {!visible.length ? <div className="rounded-2xl border border-line bg-card p-10 text-center"><h3 className="font-semibold">{(embedded ? accountRecords : records).length ? translate("No matching transactions") : translate("No transactions yet")}</h3><p className="mt-2 text-sm">{(embedded ? accountRecords : records).length ? translate("Try different filters, or clear them to see all transactions.") : translate("Record a payment, purchase, transfer, or repayment. Expected income stays separate.")}</p></div>
            : <DataTable table={table} label={translate("Transaction history")} className="min-w-[960px] text-[13px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" />}
          {embedded && visible.length > 0 && <nav aria-label={translate("Transaction history pages")} className="mt-4 flex flex-wrap items-center justify-between gap-3">
            <p className="text-xs text-muted" aria-live="polite">{translate("Showing {value0}–{value1} of {value2} transactions", { value0: table.getState().pagination.pageIndex * 10 + 1, value1: Math.min((table.getState().pagination.pageIndex + 1) * 10, visible.length), value2: visible.length })}</p>
            {table.getPageCount() > 1 && <div className="flex gap-2">
              <Button variant="outline" size="sm" disabled={!table.getCanPreviousPage()} onClick={() => table.previousPage()}>{translate("Previous page")}</Button>
              <Button variant="outline" size="sm" disabled={!table.getCanNextPage()} onClick={() => table.nextPage()}>{translate("Next page")}</Button>
            </div>}
          </nav>}
        </>}
    {filter && currency && accounts.some(a => a.id === filter && a.type === 'loan') && <Button variant="outline" className="my-3" onClick={() => setPayrollHistory(true)}>{translate("Salary payment history")}</Button>}
    {payrollHistory && currency && <SalaryPaymentHistory accountId={filter} currency={currency} onClose={() => setPayrollHistory(false)} />}
    {salaryPayment && currency && <SalaryPaymentHistory paymentId={salaryPayment} currency={currency} onClose={() => setSalaryPayment(null)} />}
    <Dialog open={!!deleting} onOpenChange={next => { if (!next && !saving) { setDeleting(null); setError(null) } }}>
      <DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>{translate("Delete transaction?")}</DialogTitle><DialogDescription>{translate("Delete “")}{deleting && (deleting.description || labels[deleting.type])}{translate("” and reverse its effect on account balances. For a linked loan payment, all principal, interest and fee components are reversed together. This cannot be undone.")}{deleting?.salary_payment_id && translate("Deleting this entry reverses the entire salary payment, including its bank deposit and all linked loan repayments.")}{needsConfirmation && translate(" This transaction has reconciliation history. Affected reconciliations in either account will be marked as needing review.")}</DialogDescription></DialogHeader>{deleting?.imported_linked_payment && <p role="status">{translate("Delete this linked payment on the device with its payroll, loan, or installment details, then sync again.")}</p>}{needsConfirmation && <label className="flex items-start gap-2 text-sm"><Input type="checkbox" className="size-4 shrink-0 p-0" checked={confirmReconciled} onChange={event => setConfirmReconciled(event.target.checked)} disabled={saving} />{translate("I confirm deleting this reconciled entry and marking affected history as needing review.")}</label>}{error && <p role="alert">{translate(error)}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>{translate("Cancel")}</Button><Button variant="destructive" disabled={saving || deleting?.imported_linked_payment || (needsConfirmation && !confirmReconciled)} onClick={remove}>{saving ? translate("Deleting…") : translate("Delete transaction")}</Button></DialogFooter></DialogContent>
    </Dialog>
  </>
}

function TransactionFlow({ row, accountFilter }: { row: Transaction; accountFilter: string }) {
  useLanguage()

  if (row.salary_payment_role === 'principal') return <span>{translate("Payroll repayment")}</span>
  const transfer = row.type === 'transfer' || row.type === 'repayment'
  const flow = moneyFlow(row, accountFilter)
  const direction = transfer ? flow === 'transfer' ? translate("Transfer") : translate(flow === 'in' ? 'Transfer in' : 'Transfer out') : flow === 'in' ? translate('In') : translate("Out")
  const Icon = transfer ? ArrowLeftRight : row.type === 'income' ? ArrowDownLeft : ArrowUpRight
  const tone = transfer ? 'bg-soft text-brand' : row.type === 'income' ? 'bg-green-500/10 text-green-700 dark:text-green-400' : 'bg-red-500/10 text-red-700 dark:text-red-400'
  return <div className="inline-flex items-center gap-1.5 whitespace-nowrap"><div>{translate(labels[row.type])}</div><span className={`inline-flex items-center gap-1 whitespace-nowrap rounded-full px-1.5 py-0.5 text-xs font-medium ${tone}`}><Icon size={13} aria-hidden="true" />{direction}</span></div>
}
