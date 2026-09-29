import { Link } from '@tanstack/react-router'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { getCoreRowModel, getSortedRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { type Account, loanTypes } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { type useCardLimits } from '../lib/card-limits'
import { accountTypes } from './AccountFormDialog'
import { AccountLabel } from './InstitutionLogo'
import { CardCredit } from './SharedCreditLimits'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'

export function AccountsTable({ accounts, currency, limits, onEdit, showLoanDetails = false, sortBalance = false }: {
  showLoanDetails?: boolean
  sortBalance?: boolean
  accounts: Account[]
  currency: string
  limits: ReturnType<typeof useCardLimits>
  onEdit: (account: Account) => void
}) {
  const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-3 align-top' }
  const columns: ColumnDef<Account>[] = [
    { id: 'name', header: 'Account', meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-44 max-w-64 break-words font-medium` }, cell: ({ row: { original: account } }) => <><AccountLabel id={account.id} name={account.name} />{account.last_four && <span className="mt-1 block text-xs text-muted">•••• {account.last_four}</span>}</> },
    { id: 'type', header: 'Type', meta, cell: ({ row: { original: account } }) => <span className="block min-w-24">{account.type === 'loan' ? loanTypes.find(type => type.value === account.loan_type)?.label ?? 'Loan (unclassified)' : accountTypes.find(type => type.value === account.type)?.label}</span> },
    { id: 'balance',
      accessorFn: account => BigInt(account.current_balance ?? account.opening_balance),
      enableSorting: sortBalance,
      sortDescFirst: false,
      sortingFn: (a, b, id) => {
        const left = a.getValue<bigint>(id), right = b.getValue<bigint>(id)
        return left < right ? -1 : left > right ? 1 : 0
      },
      header: ({ column }) => {
        if (!sortBalance) return 'Balance'
        const direction = column.getIsSorted()
        const Icon = direction === 'asc' ? ArrowUp : direction === 'desc' ? ArrowDown : ArrowUpDown
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={`Sort balance ${direction === 'asc' ? 'highest' : 'lowest'} first`} onClick={() => column.toggleSorting(direction === 'asc')}>Balance<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} whitespace-nowrap text-right tabular-nums` }, cell: ({ row: { original: account } }) => {
      const debt = ['credit_card', 'loan'].includes(account.type)
      const balance = BigInt(account.current_balance ?? account.opening_balance)
      const net = debt ? -balance : balance
      return <><span className={`font-semibold ${net < 0n ? 'text-red-700 dark:text-red-400' : net > 0n ? 'text-green-700 dark:text-green-400' : 'text-ink'}`}>{formatAmount(balance.toString(), currency)}</span><span className="mt-1 block text-xs text-muted">{debt ? balance < 0n ? 'Credit / overpayment' : 'Outstanding balance' : balance < 0n ? 'Negative balance' : 'Current balance'}</span></>
    } },
    ...(showLoanDetails ? [{
      id: 'interest',
      accessorFn: (account: Account) => account.interest_rate_ten_thousandths == null ? undefined : BigInt(account.interest_rate_ten_thousandths),
      enableSorting: true,
      sortUndefined: 'last' as const,
      sortDescFirst: false,
      sortingFn: (a, b, id) => {
        const left = a.getValue<bigint>(id), right = b.getValue<bigint>(id)
        return left < right ? -1 : left > right ? 1 : 0
      },
      header: ({ column }) => {
        const direction = column.getIsSorted()
        const Icon = direction === 'asc' ? ArrowUp : direction === 'desc' ? ArrowDown : ArrowUpDown
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={`Sort annual interest rate ${direction === 'asc' ? 'highest' : 'lowest'} first`} onClick={() => column.toggleSorting(direction === 'asc')}>Annual interest rate<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums` },
      cell: ({ row: { original: account } }) => account.interest_rate_ten_thousandths == null ? <span className="text-muted">Not set</span> : `${interestRateText(String(account.interest_rate_ten_thousandths), 4)}%`,
    } satisfies ColumnDef<Account>, {
      id: 'monthlyInstallment',
      accessorFn: (account: Account) => account.monthly_installment == null ? undefined : BigInt(account.monthly_installment),
      enableSorting: true,
      sortUndefined: 'last' as const,
      sortDescFirst: false,
      sortingFn: (a, b, id) => {
        const left = a.getValue<bigint>(id), right = b.getValue<bigint>(id)
        return left < right ? -1 : left > right ? 1 : 0
      },
      header: ({ column }) => {
        const direction = column.getIsSorted()
        const Icon = direction === 'asc' ? ArrowUp : direction === 'desc' ? ArrowDown : ArrowUpDown
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={`Sort monthly installment ${direction === 'asc' ? 'highest' : 'lowest'} first`} onClick={() => column.toggleSorting(direction === 'asc')}>Monthly installment<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums` },
      cell: ({ row: { original: account } }) => account.monthly_installment == null ? <span className="text-muted">Not set</span> : formatAmount(account.monthly_installment, currency),
    } satisfies ColumnDef<Account>] : []),
    { id: 'details', header: 'Details', meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-48 max-w-72` }, cell: ({ row: { original: account } }) => {
      const hasDetails = account.type === 'loan' || account.type === 'credit_card' || account.statement_day != null || account.payment_due_day != null || account.interest_rate_ten_thousandths != null || account.notes
      return hasDetails ? <details className="text-xs"><summary className="cursor-pointer rounded text-sm text-brand focus-visible:outline-2 focus-visible:outline-brand">Account details</summary>
        {account.type === 'credit_card' && <CardCredit id={account.id} individualLimit={account.credit_limit} balance={account.current_balance ?? account.opening_balance} currency={currency} limits={limits} />}
        {account.type === 'loan' && <><p className="mt-2">Initial Loan Amount: {account.initial_loan_amount != null ? formatAmount(account.initial_loan_amount, currency) : 'Not set'}</p><p className="mt-2">Monthly installment: {account.monthly_installment != null ? formatAmount(account.monthly_installment, currency) : 'Not set'}</p></>}
        {account.statement_day != null && <p className="mt-2">Statement day: {account.statement_day}</p>}
        {account.payment_due_day != null && <p className="mt-2">Payment due day: {account.payment_due_day}</p>}
        {account.interest_rate_ten_thousandths != null && <p className="mt-2">Annual interest rate: {interestRateText(String(account.interest_rate_ten_thousandths), 4)}%</p>}
        {account.notes && <p className="mt-2 break-words whitespace-pre-wrap">{account.notes}</p>}
      </details> : <span className="text-muted">—</span>
    } },
    { id: 'actions', header: 'Actions', meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-48` }, cell: ({ row: { original: account } }) => <div className="flex flex-col items-start gap-2">
      {account.paid_off_on ? <span className="text-xs text-muted">Paid off {account.paid_off_on}</span> : <Button type="button" variant="outline" size="sm" aria-label={`Edit account ${account.name}`} onClick={() => onEdit(account)}>Edit</Button>}
      {account.type === 'loan' && <Link className="text-xs text-brand" to="/accounts/$accountId/loans" params={{ accountId: account.id }}>Overview, contracts & transactions</Link>}
      {account.type === 'credit_card' && <Link className="text-xs text-brand" to="/accounts/$accountId/billing" params={{ accountId: account.id }}>Billing & payments</Link>}
      {['bank', 'wallet', 'credit_card'].includes(account.type) && <Link className="text-xs text-brand" to="/accounts/$accountId" params={{ accountId: account.id }}>Transactions & reconciliation</Link>}
    </div> },
  ]
  const table = useReactTable({ data: accounts, columns, getRowId: account => account.id, getCoreRowModel: getCoreRowModel(), getSortedRowModel: getSortedRowModel(), defaultColumn: { enableSorting: false } })
  return <DataTable table={table} label="Accounts" className="min-w-[850px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" />
}
