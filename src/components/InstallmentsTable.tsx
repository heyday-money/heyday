import { InstallmentCalendar } from './InstallmentCalendar'
import { useMemo, useState } from 'react'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { toast } from 'sonner'
import { deleteInstallment, type FinancialData, type Installment } from '../lib/desktop'
import { dateKey, isCash, isDebt } from '../lib/financial'
import { installmentSchedule, interestRateText } from '../lib/installments'
import { formatAmount } from '../lib/money'
import { InstallmentDialog } from './InstallmentDialog'
import { DataTable } from './ui/data-table'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

type DisplayInstallment = Installment & { last: string; next: string | null; status: string }
export function InstallmentsTable({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  const [editing, setEditing] = useState<{ plan?: Installment } | null>(null)
  const [deleting, setDeleting] = useState<Installment | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const currency = data.settings.currency!
  const todayString = dateKey(today)
  const rows = useMemo<DisplayInstallment[]>(() => (data.installments ?? []).map(plan => {
    const dates = installmentSchedule(plan)
    const last = dates.at(-1)!.date
    const available = data.accounts.some(account => account.id === plan.account_id && isCash(account)) && data.accounts.some(account => account.id === plan.debt_account_id && isDebt(account))
    return { ...plan, last, next: dates.find(payment => payment.date >= todayString)?.date ?? null, status: !available ? 'Account unavailable · excluded' : last < todayString ? 'Schedule ended' : plan.first_due_date > todayString ? 'Scheduled' : 'In progress' }
  }), [data.installments, data.accounts, todayString])
  const columns = useMemo<ColumnDef<DisplayInstallment>[]>(() => {
    const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-4 align-top' }
    return [
      { id: 'name', header: 'Installment plan', meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-40 max-w-64 break-words font-medium` }, cell: ({ row }) => row.original.name },
      { id: 'debt', header: 'Credit card', meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-40 max-w-60 break-words` }, cell: ({ row }) => <>{row.original.debt_account_name}{row.original.debt_account_type === 'loan' && <span className="mt-1 block text-xs text-muted">Existing loan schedule</span>}</> },
      { id: 'purchase', header: 'Purchase', meta, cell: ({ row }) => row.original.purchase_kind === 'new_purchase' ? row.original.purchase_transaction_id ? 'Purchase recorded' : 'Purchase deleted · review schedule' : 'Already recorded · schedule only' },
      { id: 'monthly', header: 'Monthly payment', meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} whitespace-nowrap text-right font-semibold tabular-nums` }, cell: ({ row }) => formatAmount(row.original.monthly_amount, currency) },
      { id: 'interest', header: 'Annual interest', meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.interest_rate_bps == null ? 'Not set' : `${interestRateText(row.original.interest_rate_bps)}%` },
      { id: 'term', header: 'Installments', meta, cell: ({ row }) => <>{row.original.installment_count} monthly<span className="mt-1 block whitespace-nowrap text-xs text-muted">Total: {formatAmount((BigInt(row.original.monthly_amount) * BigInt(row.original.installment_count)).toString(), currency)}</span></> },
      { id: 'dates', header: 'Schedule', meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap text-xs tabular-nums` }, cell: ({ row }) => <>{row.original.first_due_date}<span className="block">to {row.original.last}</span></> },
      { id: 'next', header: 'Next scheduled date', meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.next ?? '—' },
      { id: 'source', header: 'Pay from', meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-32 max-w-60 break-words` }, cell: ({ row }) => row.original.account_name },
      { id: 'status', header: 'Schedule status', meta, cell: ({ row }) => row.original.status },
      { id: 'actions', header: 'Actions', meta, cell: ({ row }) => <div className="flex gap-2">{row.original.debt_account_type !== 'loan' && <Button size="sm" variant="outline" disabled={loading} aria-label={`Edit installment ${row.original.name}`} onClick={() => setEditing({ plan: row.original })}>Edit</Button>}<Button size="sm" variant="outline" disabled={loading} aria-label={`Remove installment ${row.original.name}`} onClick={() => { setError(null); setDeleting(row.original) }}>Remove</Button></div> },
    ]
  }, [currency, loading])
  const table = useReactTable({ data: rows, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try { await deleteInstallment(deleting.id); setDeleting(null); toast.success('Installment removed. Account balances are unchanged.') }
    catch (error) { setError(typeof error === 'string' ? error : 'Could not remove the installment.') }
    finally { setSaving(false) }
  }
  const canAdd = data.accounts.some(isCash) && data.accounts.some(account => account.type === 'credit_card')
  return <section className="min-w-0" aria-labelledby="installments-title">
    <div className="mb-4 flex flex-wrap items-start justify-between gap-4"><div><h2 id="installments-title" className="text-[25px] font-semibold">Installment plans</h2><p className="mt-2 text-sm">Your credit card purchases, with a fixed monthly payment.</p></div><div className="flex items-center gap-2"><InstallmentCalendar data={data} today={today} loading={loading} /><Button disabled={!canAdd || loading} onClick={() => setEditing({})}><Plus size={16} />Add installment</Button></div></div>
    {rows.some(row => row.debt_account_type === 'loan') && <p className="mb-4 rounded-xl bg-soft p-4 text-sm">Existing loan schedules are preserved for reference and remain in Outlook forecasts. They cannot be edited here; removing one only removes its forecast schedule.</p>}
    {!canAdd && <p className="mb-4 text-sm">Add an active credit card and a cash or bank account to create an installment schedule.</p>}
    {rows.length ? <DataTable table={table} label="Installment plans" className="min-w-[1200px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" busy={loading} /> : <p className="rounded-2xl border border-line bg-card p-6 text-sm">No installment plans yet. Add a plan to see its monthly payment and schedule here.</p>}
    <p className="mt-3 text-xs">Schedule status is based on dates. Payments are not matched or marked as paid. Upcoming installments after today are included in Outlook; due and past dates are excluded. Edit or remove schedules that end early, and avoid duplicate payment plans.</p>
    {editing && <InstallmentDialog data={data} plan={editing.plan} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) { setDeleting(null); setError(null) } }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>Remove installment?</DialogTitle><DialogDescription>Remove “{deleting?.name}” and its future schedule from Outlook. Recorded transactions and account balances stay unchanged.</DialogDescription></DialogHeader>{error && <p role="alert">{error}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>Cancel</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? 'Removing…' : 'Remove installment'}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
