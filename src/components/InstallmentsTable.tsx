import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { AccountLabel } from './InstitutionLogo'
import { InstallmentCalendar } from './InstallmentCalendar'
import { useMemo, useState } from 'react'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { Pencil, Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { deleteInstallment, type FinancialData, type Installment } from '../lib/desktop'
import { dateKey, isCash, isDebt } from '../lib/financial'
import { installmentSchedule, interestRateText } from '../lib/installments'
import { formatAmount } from '../lib/money'
import { InstallmentDialog } from './InstallmentDialog'
import { DataTable } from './ui/data-table'
import { Button } from './ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

type DisplayInstallment = Installment & { last: string; next: string | null; status: string }
export function InstallmentsTable({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  useLanguage()

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
    return { ...plan, last, next: dates.find(payment => payment.date >= todayString)?.date ?? null, status: !available ? translate("Account unavailable · excluded") : last < todayString ? translate("Schedule ended") : plan.first_due_date > todayString ? translate("Scheduled") : translate("In progress") }
  }), [data.installments, data.accounts, todayString, getLanguage()])
  const columns = useMemo<ColumnDef<DisplayInstallment>[]>(() => {
    const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-4 align-top' }
    return [
      { id: 'name', header: translate("Installment plan"), meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-40 max-w-64 break-words font-medium` }, cell: ({ row }) => row.original.name },
      { id: 'debt', header: translate("Credit card"), meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-40 max-w-60 break-words` }, cell: ({ row }) => <><AccountLabel id={row.original.debt_account_id} name={row.original.debt_account_name} />{row.original.debt_account_type === 'loan' && <span className="mt-1 block text-xs text-muted">{translate("Existing loan schedule")}</span>}</> },
      { id: 'purchase', header: translate("Purchase"), meta, cell: ({ row }) => row.original.purchase_kind === 'new_purchase' ? row.original.purchase_transaction_id ? translate("Purchase recorded") : translate("Purchase deleted · review schedule") : translate("Already recorded · schedule only") },
      { id: 'monthly', header: translate("Monthly payment"), meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} whitespace-nowrap text-right font-semibold tabular-nums` }, cell: ({ row }) => formatAmount(row.original.monthly_amount, currency) },
      { id: 'interest', header: translate("Annual interest"), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.interest_rate_millis == null ? translate("Not set") : `${interestRateText(row.original.interest_rate_millis)}%` },
      { id: 'term', header: translate("Installments"), meta, cell: ({ row }) => <>{row.original.installment_count} {" "}{translate("monthly")}<span className="mt-1 block whitespace-nowrap text-xs text-muted">{translate("Total:")}{" "}{formatAmount((BigInt(row.original.monthly_amount) * BigInt(row.original.installment_count)).toString(), currency)}</span></> },
      { id: 'dates', header: translate("Schedule"), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap text-xs tabular-nums` }, cell: ({ row }) => <>{row.original.first_due_date}<span className="block">{translate("to")}{" "}{row.original.last}</span></> },
      { id: 'next', header: translate("Next scheduled date"), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.next ?? '—' },
      { id: 'source', header: translate("Pay from"), meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-32 max-w-60 break-words` }, cell: ({ row }) => <AccountLabel id={row.original.account_id} name={row.original.account_name} /> },
      { id: 'status', header: translate("Schedule status"), meta, cell: ({ row }) => row.original.status },
      { id: 'actions', header: translate("Actions"), meta, cell: ({ row }) => <TooltipProvider><div className="flex items-center gap-1">
        {row.original.debt_account_type !== 'loan' && <Tooltip><TooltipTrigger asChild><Button type="button" size="icon-sm" variant="ghost" disabled={loading} aria-label={translate("Edit installment {value0}", { value0: row.original.name })} onClick={() => setEditing({ plan: row.original })}><Pencil aria-hidden="true" /></Button></TooltipTrigger><TooltipContent>{translate("Edit")}</TooltipContent></Tooltip>}
        <Tooltip><TooltipTrigger asChild><Button type="button" size="icon-sm" variant="ghost" disabled={loading} aria-label={translate("Remove installment {value0}", { value0: row.original.name })} onClick={() => { setError(null); setDeleting(row.original) }}><Trash2 aria-hidden="true" /></Button></TooltipTrigger><TooltipContent>{translate("Remove")}</TooltipContent></Tooltip>
      </div></TooltipProvider> },
    ]
  }, [currency, loading, getLanguage()])
  const table = useReactTable({ data: rows, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try { await deleteInstallment(deleting.id); setDeleting(null); toast.success(translate("Installment removed. Account balances are unchanged.")) }
    catch (error) { setError(typeof error === 'string' ? error : "Could not remove the installment.") }
    finally { setSaving(false) }
  }
  const canAdd = data.accounts.some(isCash) && data.accounts.some(account => account.type === 'credit_card')
  return <section className="min-w-0" aria-labelledby="installments-title">
    <div className="mb-4 flex flex-wrap items-start justify-between gap-4"><div><h2 id="installments-title" className="text-2xl font-semibold">{translate("Installment plans")}</h2><p className="mt-2 text-sm">{translate("Your credit card purchases, with a fixed monthly payment.")}</p></div><div className="flex items-center gap-2"><InstallmentCalendar data={data} today={today} loading={loading} /><Button disabled={!canAdd || loading} onClick={() => setEditing({})}><Plus size={16} />{translate("Add installment")}</Button></div></div>
    {rows.some(row => row.debt_account_type === 'loan') && <p className="mb-4 rounded-xl bg-soft p-4 text-sm">{translate("Existing loan schedules are preserved for reference and remain in Outlook forecasts. They cannot be edited here; removing one only removes its forecast schedule.")}</p>}
    {!canAdd && <p className="mb-4 text-sm">{translate("Add an active credit card and a cash, bank, or digital wallet account to create an installment schedule.")}</p>}
    {rows.length ? <DataTable table={table} label={translate("Installment plans")} className="min-w-[1200px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" busy={loading} /> : <p className="rounded-2xl border border-line bg-card p-6 text-sm">{translate("No installment plans yet. Add a plan to see its monthly payment and schedule here.")}</p>}
    <p className="mt-3 text-xs">{translate("Schedule status is based on dates. Payments are not matched or marked as paid. Upcoming installments after today are included in Outlook; due and past dates are excluded. Edit or remove schedules that end early, and avoid duplicate payment plans.")}</p>
    {editing && <InstallmentDialog data={data} plan={editing.plan} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) { setDeleting(null); setError(null) } }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>{translate("Remove installment?")}</DialogTitle><DialogDescription>{translate("Remove “")}{deleting?.name}{translate("” and its future schedule from Outlook. Recorded transactions and account balances stay unchanged.")}</DialogDescription></DialogHeader>{error && <p role="alert">{translate(error)}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>{translate("Cancel")}</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? translate("Removing…") : translate("Remove installment")}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
