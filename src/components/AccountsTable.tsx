import { t as translate, useLanguage } from "../lib/i18n"
import { AccountActions } from './AccountActions'
import { Link, useNavigate } from '@tanstack/react-router'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { getCoreRowModel, getSortedRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { type Account, loanTypes } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { accountTypes } from './AccountFormDialog'
import { AccountLabel } from './InstitutionLogo'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'

export function AccountsTable({ accounts, currency, onEdit, showLoanDetails = false, sortBalance = false }: {
  showLoanDetails?: boolean
  sortBalance?: boolean
  accounts: Account[]
  currency: string
  onEdit: (account: Account) => void
}) {
  useLanguage()
  const navigate = useNavigate()

  const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-3 align-top' }
  const columns: ColumnDef<Account>[] = [
    { id: 'name', header: translate("Account"), meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-44 max-w-64 break-words font-medium` }, cell: ({ row: { original: account } }) => <><Link className="text-brand hover:underline" to="/accounts/$accountId/details" params={{ accountId: account.id }}><AccountLabel id={account.id} name={account.name} /></Link>{account.paid_off_on ? <span className="mt-1 block text-xs text-muted">{translate("Paid off")} {account.paid_off_on}</span> : account.is_archived && <span className="mt-1 block text-xs text-muted">{translate("Archived")}</span>}{account.last_four && <span className="mt-1 block text-xs text-muted">•••• {account.last_four}</span>}</> },
    { id: 'type', header: translate("Type"), meta, cell: ({ row: { original: account } }) => <span className="block min-w-24">{account.type === 'loan' ? loanTypes.find(type => type.value === account.loan_type)?.label ?? translate("Loan (unclassified)") : accountTypes.find(type => type.value === account.type)?.label}</span> },
    { id: 'balance',
      accessorFn: account => BigInt(account.current_balance ?? account.opening_balance),
      enableSorting: sortBalance,
      sortDescFirst: false,
      sortingFn: (a, b, id) => {
        const left = a.getValue<bigint>(id), right = b.getValue<bigint>(id)
        return left < right ? -1 : left > right ? 1 : 0
      },
      header: ({ column }) => {
        if (!sortBalance) return translate("Balance")
        const direction = column.getIsSorted()
        const Icon = direction === 'asc' ? ArrowUp : direction === 'desc' ? ArrowDown : ArrowUpDown
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={translate("Sort balance {value0} first", { value0: direction === 'asc' ? translate('highest') : translate('lowest') })} onClick={() => column.toggleSorting(direction === 'asc')}>{translate("Balance")}<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} whitespace-nowrap text-right tabular-nums` }, cell: ({ row: { original: account } }) => {
      const debt = ['credit_card', 'loan'].includes(account.type)
      const balance = BigInt(account.current_balance ?? account.opening_balance)
      const net = debt ? -balance : balance
      return <><span className={`font-semibold ${net < 0n ? 'text-red-700 dark:text-red-400' : net > 0n ? 'text-green-700 dark:text-green-400' : 'text-ink'}`}>{formatAmount(balance.toString(), currency)}</span><span className="mt-1 block text-xs text-muted">{debt ? balance < 0n ? translate("Credit / overpayment") : translate("Outstanding balance") : balance < 0n ? translate("Negative balance") : translate("Current balance")}</span></>
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
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={translate("Sort annual interest rate {value0} first", { value0: direction === 'asc' ? translate('highest') : translate('lowest') })} onClick={() => column.toggleSorting(direction === 'asc')}>{translate("Annual interest rate")}<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums` },
      cell: ({ row: { original: account } }) => account.interest_rate_ten_thousandths == null ? <span className="text-muted">{translate("Not set")}</span> : `${interestRateText(String(account.interest_rate_ten_thousandths), 4)}%`,
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
        return <Button type="button" variant="ghost" size="sm" className="-mr-2" aria-label={translate("Sort monthly installment {value0} first", { value0: direction === 'asc' ? translate('highest') : translate('lowest') })} onClick={() => column.toggleSorting(direction === 'asc')}>{translate("Monthly installment")}<Icon size={14} aria-hidden="true" /></Button>
      },
      meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums` },
      cell: ({ row: { original: account } }) => account.monthly_installment == null ? <span className="text-muted">{translate("Not set")}</span> : formatAmount(account.monthly_installment, currency),
    } satisfies ColumnDef<Account>] : []),
    { id: 'actions', header: translate("Actions"), meta: { ...meta, cellClassName: `${meta.cellClassName} w-24` }, cell: ({ row: { original: account } }) => <AccountActions account={account} onEdit={onEdit} /> },
  ]
  const table = useReactTable({ data: accounts, columns, getRowId: account => account.id, getCoreRowModel: getCoreRowModel(), getSortedRowModel: getSortedRowModel(), defaultColumn: { enableSorting: false } })
  return <DataTable table={table} label={translate("Accounts")} className={showLoanDetails ? "min-w-[850px]" : "min-w-[600px]"} headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line"
    rowClassName="cursor-pointer transition-colors hover:bg-soft/60 focus-within:bg-soft/60"
    onRowClick={(account, event) => {
      if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return
      if (!(event.target instanceof Element) || !event.currentTarget.contains(event.target) || event.target.closest('a, button, input, select, textarea, details, summary, [role="button"], [contenteditable="true"]')) return
      if (window.getSelection()?.toString()) return
      void navigate({ to: '/accounts/$accountId/details', params: { accountId: account.id } })
    }} />
}
