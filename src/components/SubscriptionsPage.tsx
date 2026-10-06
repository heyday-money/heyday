import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { AccountLabel } from './InstitutionLogo'
import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { ExternalLink, Pencil, Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { deleteSubscription, openSubscriptionManagement, desktopAvailable, type FinancialData, type Subscription } from '../lib/desktop'
import { useFinancialData } from '../lib/useFinancialData'
import { dateKey } from '../lib/financial'
import { nextSubscriptionDate } from '../lib/subscriptions'
import { formatAmount } from '../lib/money'
import { SubscriptionLogo } from './SubscriptionLogo'
import { subscriptionPlatforms } from '../lib/subscription-management'
import { SubscriptionDialog } from './SubscriptionDialog'
import { Button } from './ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'
import { DataTable } from './ui/data-table'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

export function SubscriptionsPage() {
  useLanguage()

  const { data, loading, error, today, reload } = useFinancialData()
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage your local subscriptions.")}</p>
  if (error) return <div role="alert">{translate("Could not load subscriptions.")}{" "}<Button variant="outline" onClick={reload}>{translate("Retry subscriptions")}</Button></div>
  if (!data) return <p role="status">{translate("Loading subscriptions…")}</p>
  if (!data.settings.currency) return <p className="rounded-xl bg-soft p-5">{translate("Choose your currency in")}{" "}<Link to="/settings" className="text-brand">{translate("Settings")}</Link> {" "}{translate("to manage subscriptions.")}</p>
  return <SubscriptionsTable data={data} today={today} loading={loading} />
}
function SubscriptionsTable({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  useLanguage()

  const [editing, setEditing] = useState<{ subscription?: Subscription } | null>(null)
  const [deleting, setDeleting] = useState<Subscription | null>(null)
  const [saving, setSaving] = useState(false)
  const [openingId, setOpeningId] = useState<string | null>(null)
  async function manage(subscription: Subscription) {
    setOpeningId(subscription.id)
    try { await openSubscriptionManagement(subscription.id) }
    catch (error) { toast.error(translate(typeof error === 'string' ? error : 'Could not open the management link. Please try again.')) }
    finally { setOpeningId(null) }
  }
  const [error, setError] = useState<string | null>(null)
  const currency = data.settings.currency!
  const todayString = dateKey(today)
  const rows = useMemo(() => (data.subscriptions ?? []).map(subscription => {
    const next = nextSubscriptionDate(subscription, todayString)
    const available = data.accounts.some(account => account.id === subscription.account_id && ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
    return { ...subscription, next: available ? next : null, status: !subscription.is_active ? translate("Paused") : !available ? translate("Account unavailable") : !next ? translate("Schedule ended") : subscription.first_billing_date > todayString ? translate("Scheduled") : translate("Active") }
  }), [data.subscriptions, data.accounts, todayString, getLanguage()])
  const columns = useMemo<ColumnDef<typeof rows[number]>[]>(() => {
    const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-3 align-top' }
    return [
      { id: 'name', header: translate("Subscription"), meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-40 max-w-60 break-words font-medium` }, cell: ({ row }) => <div className="flex items-center gap-3"><SubscriptionLogo subscription={row.original} /><span>{row.original.name}</span></div> },
      { id: 'amount', header: translate("Amount per charge"), meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums font-semibold` }, cell: ({ row }) => formatAmount(row.original.amount, currency) },
      { id: 'frequency', header: translate("Frequency"), meta, cell: ({ row }) => row.original.frequency === 'monthly' ? translate("Monthly") : translate("Yearly") },
      { id: 'account', header: translate("Pay from"), meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-32 max-w-52 break-words` }, cell: ({ row }) => <><AccountLabel id={row.original.account_id} name={row.original.account_name} />{row.original.account_type === 'credit_card' && <span className="mt-1 block text-xs text-muted">{translate("Credit card · excluded from cash forecast")}</span>}</> },
      { id: 'next', header: translate("Next scheduled charge"), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.next ?? '—' },
      { id: 'dates', header: translate("Schedule"), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap text-xs tabular-nums` }, cell: ({ row }) => <>{row.original.first_billing_date}<span className="block">{row.original.end_date ? translate("Through {value0}", { value0: row.original.end_date }) : translate("No end date")}</span></> },
      { id: 'category', header: translate("Category"), meta, cell: ({ row }) => row.original.category_name ?? translate("Uncategorized") },
      { id: 'management', header: translate("Managed through"), meta, cell: ({ row }) => subscriptionPlatforms.find(platform => platform.value === row.original.managed_via)?.label ?? translate("Not specified") },
      { id: 'status', header: translate("Status"), meta, cell: ({ row }) => row.original.status },
      { id: 'actions', header: translate("Actions"), meta, cell: ({ row }) => <TooltipProvider><div className="flex items-center gap-1">
        <Tooltip><TooltipTrigger asChild><Button type="button" size="icon-sm" variant="ghost" disabled={loading} aria-label={translate("Edit subscription {value0}", { value0: row.original.name })} onClick={() => setEditing({ subscription: row.original })}><Pencil aria-hidden="true" /></Button></TooltipTrigger><TooltipContent>{translate("Edit")}</TooltipContent></Tooltip>
        <Tooltip><TooltipTrigger asChild><Button type="button" size="icon-sm" variant="ghost" disabled={loading} aria-label={translate("Remove subscription {value0}", { value0: row.original.name })} onClick={() => { setError(null); setDeleting(row.original) }}><Trash2 aria-hidden="true" /></Button></TooltipTrigger><TooltipContent>{translate("Remove")}</TooltipContent></Tooltip>
        {row.original.management_url && <Tooltip><TooltipTrigger asChild><Button type="button" size="icon-sm" variant="ghost" disabled={loading || openingId !== null} aria-label={translate("Manage subscription {value0}", { value0: row.original.name })} onClick={() => void manage(row.original)}><ExternalLink aria-hidden="true" /></Button></TooltipTrigger><TooltipContent><p>{translate("Open management link")}</p><p className="break-all text-muted">{row.original.management_url}</p></TooltipContent></Tooltip>}
      </div></TooltipProvider> },
    ]
  }, [currency, loading, openingId, getLanguage()])
  const table = useReactTable({ data: rows, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try { await deleteSubscription(deleting.id); setDeleting(null); toast.success(translate("Subscription removed. Account balances are unchanged.")) }
    catch (error) { setError(typeof error === 'string' ? error : "Could not remove the subscription.") }
    finally { setSaving(false) }
  }
  const canAdd = data.accounts.some(account => ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
  return <section className="min-w-0" aria-labelledby="subscriptions-title">
    <div className="mb-5 flex flex-wrap items-start justify-between gap-4"><div><h2 id="subscriptions-title" className="text-2xl font-semibold">{translate("Subscription plans")}</h2><p className="mt-2 text-sm">{translate("Keep track of recurring services and their next scheduled charge.")}</p></div><Button disabled={!canAdd || loading} onClick={() => setEditing({})}><Plus size={16} />{translate("Add subscription")}</Button></div>
    {!canAdd && <p className="mb-4 text-sm">{translate("Add a cash, bank, digital wallet, or credit card account in")}{" "}<Link to="/accounts" className="text-brand">{translate("Accounts")}</Link> {" "}{translate("to create a subscription.")}</p>}
    {rows.length ? <DataTable table={table} label={translate("Subscription plans")} className="min-w-[1100px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" busy={loading} /> : <p className="rounded-2xl border border-line bg-card p-6">{translate("No subscriptions yet. Add your first recurring service.")}</p>}
    <p className="mt-3 text-xs">{translate("These are schedules, not confirmed payments. Record charges separately in Transactions. Pausing or removing a plan here does not cancel the service with its provider.")}</p>
    {editing && <SubscriptionDialog data={data} subscription={editing.subscription} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) { setDeleting(null); setError(null) } }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>{translate("Remove subscription?")}</DialogTitle><DialogDescription>{translate("Remove “")}{deleting?.name}{translate("” and its future forecasts. This does not cancel the service, delete transactions, or change account balances.")}</DialogDescription></DialogHeader>{error && <p role="alert">{translate(error)}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>{translate("Cancel")}</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? translate("Removing…") : translate("Remove subscription")}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
