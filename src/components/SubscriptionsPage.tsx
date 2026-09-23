import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { toast } from 'sonner'
import { deleteSubscription, desktopAvailable, type FinancialData, type Subscription } from '../lib/desktop'
import { useFinancialData } from '../lib/useFinancialData'
import { dateKey } from '../lib/financial'
import { nextSubscriptionDate } from '../lib/subscriptions'
import { formatAmount } from '../lib/money'
import { SubscriptionDialog } from './SubscriptionDialog'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

export function SubscriptionsPage() {
  const { data, loading, error, today, reload } = useFinancialData()
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your local subscriptions.</p>
  if (error) return <div role="alert">Could not load subscriptions. <Button variant="outline" onClick={reload}>Retry subscriptions</Button></div>
  if (!data) return <p role="status">Loading subscriptions…</p>
  if (!data.settings.currency) return <p className="rounded-xl bg-soft p-5">Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to manage subscriptions.</p>
  return <SubscriptionsTable data={data} today={today} loading={loading} />
}
function SubscriptionsTable({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  const [editing, setEditing] = useState<{ subscription?: Subscription } | null>(null)
  const [deleting, setDeleting] = useState<Subscription | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const currency = data.settings.currency!
  const todayString = dateKey(today)
  const rows = useMemo(() => (data.subscriptions ?? []).map(subscription => {
    const next = nextSubscriptionDate(subscription, todayString)
    const available = data.accounts.some(account => account.id === subscription.account_id && ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
    return { ...subscription, next: available ? next : null, status: !subscription.is_active ? 'Paused' : !available ? 'Account unavailable' : !next ? 'Schedule ended' : subscription.first_billing_date > todayString ? 'Scheduled' : 'Active' }
  }), [data.subscriptions, data.accounts, todayString])
  const columns = useMemo<ColumnDef<typeof rows[number]>[]>(() => {
    const meta = { headerClassName: 'px-4 py-3 font-semibold whitespace-nowrap', cellClassName: 'px-4 py-4 align-top' }
    return [
      { id: 'name', header: 'Subscription', meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-40 max-w-60 break-words font-medium` }, cell: ({ row }) => row.original.name },
      { id: 'amount', header: 'Amount per charge', meta: { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums font-semibold` }, cell: ({ row }) => formatAmount(row.original.amount, currency) },
      { id: 'frequency', header: 'Frequency', meta, cell: ({ row }) => row.original.frequency === 'monthly' ? 'Monthly' : 'Yearly' },
      { id: 'account', header: 'Pay from', meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-32 max-w-52 break-words` }, cell: ({ row }) => <>{row.original.account_name}{row.original.account_type === 'credit_card' && <span className="mt-1 block text-xs text-muted">Credit card · excluded from cash forecast</span>}</> },
      { id: 'next', header: 'Next scheduled charge', meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap tabular-nums` }, cell: ({ row }) => row.original.next ?? '—' },
      { id: 'dates', header: 'Schedule', meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap text-xs tabular-nums` }, cell: ({ row }) => <>{row.original.first_billing_date}<span className="block">{row.original.end_date ? `Through ${row.original.end_date}` : 'No end date'}</span></> },
      { id: 'category', header: 'Category', meta, cell: ({ row }) => row.original.category_name ?? 'Uncategorized' },
      { id: 'status', header: 'Status', meta, cell: ({ row }) => row.original.status },
      { id: 'actions', header: 'Actions', meta, cell: ({ row }) => <div className="flex gap-2"><Button size="sm" variant="outline" disabled={loading} aria-label={`Edit subscription ${row.original.name}`} onClick={() => setEditing({ subscription: row.original })}>Edit</Button><Button size="sm" variant="outline" disabled={loading} aria-label={`Remove subscription ${row.original.name}`} onClick={() => { setError(null); setDeleting(row.original) }}>Remove</Button></div> },
    ]
  }, [currency, loading])
  const table = useReactTable({ data: rows, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setError(null)
    try { await deleteSubscription(deleting.id); setDeleting(null); toast.success('Subscription removed. Account balances are unchanged.') }
    catch (error) { setError(typeof error === 'string' ? error : 'Could not remove the subscription.') }
    finally { setSaving(false) }
  }
  const canAdd = data.accounts.some(account => ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
  return <section className="min-w-0" aria-labelledby="subscriptions-title">
    <div className="mb-5 flex flex-wrap items-start justify-between gap-4"><div><h2 id="subscriptions-title" className="text-[25px] font-semibold">Subscription plans</h2><p className="mt-2 text-sm">Keep track of recurring services and their next scheduled charge.</p></div><Button disabled={!canAdd || loading} onClick={() => setEditing({})}><Plus size={16} />Add subscription</Button></div>
    {!canAdd && <p className="mb-4 text-sm">Add a cash, bank, digital wallet, or credit card account in <Link to="/accounts" className="text-brand">Accounts</Link> to create a subscription.</p>}
    {rows.length ? <DataTable table={table} label="Subscription plans" className="min-w-[1100px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" busy={loading} /> : <p className="rounded-2xl border border-line bg-card p-6">No subscriptions yet. Add your first recurring service.</p>}
    <p className="mt-3 text-xs">These are schedules, not confirmed payments. Record charges separately in Transactions. Pausing or removing a plan here does not cancel the service with its provider.</p>
    {editing && <SubscriptionDialog data={data} subscription={editing.subscription} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) { setDeleting(null); setError(null) } }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>Remove subscription?</DialogTitle><DialogDescription>Remove “{deleting?.name}” and its future forecasts. This does not cancel the service, delete transactions, or change account balances.</DialogDescription></DialogHeader>{error && <p role="alert">{error}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>Cancel</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? 'Removing…' : 'Remove subscription'}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
