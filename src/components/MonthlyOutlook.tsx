import { OutlookTable } from './OutlookTable'
import { useCallback, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { toast } from 'sonner'
import { desktopAvailable, deletePaymentPlan, type PaymentPlan } from '../lib/desktop'
import { dateKey, isCash, monthlyOutlook } from '../lib/financial'
import { formatAmount } from '../lib/money'
import { useFinancialData } from '../lib/useFinancialData'
import { Button } from './ui/button'
import { PaymentPlanDialog } from './PaymentPlanDialog'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function MonthlyOutlook() {
  const { data, loading, error, today, reload } = useFinancialData()
  const [editing, setEditing] = useState<{ date: string; plan?: PaymentPlan } | null>(null)
  const [deleting, setDeleting] = useState<PaymentPlan | null>(null)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const tomorrow = dateKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1))
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setSaveError(null)
    try { await deletePaymentPlan(deleting.id); setDeleting(null); toast.success('Plan removed.') }
    catch (error) { setSaveError(typeof error === 'string' ? error : 'Could not remove the plan.') }
    finally { setSaving(false) }
  }
  const currency = data?.settings.currency
  const periods = useMemo(() => data ? monthlyOutlook(data, today) : [], [data, today])
  const openPlan = useCallback((date: string) => setEditing({ date }), [])
  return <section className="min-w-0" aria-labelledby="monthly-outlook-title">
    <div className="mb-5 flex flex-wrap items-start justify-between gap-4"><div><h2 id="monthly-outlook-title" className="text-2xl font-semibold">Monthly outlook</h2><p className="mt-2 text-sm">Compare this payday cycle and the next six. Cash and bank accounts only.</p></div><Button disabled={!desktopAvailable || !data || !currency || error || loading || !data.accounts.some(isCash)} onClick={() => setEditing({ date: tomorrow })}><Plus size={16} />Add payment plan</Button></div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">Open the desktop app to see your local monthly outlook.</p>
      : error ? <div role="alert">Could not load your monthly outlook. <Button variant="outline" onClick={reload}>Retry outlook</Button></div>
      : !data ? <p role="status">Loading monthly outlook…</p>
      : !currency ? <p className="rounded-xl bg-soft p-5">Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to start planning.</p>
      : !data.accounts.some(isCash) ? <p className="rounded-xl bg-soft p-5">Add a cash or bank account in <Link to="/accounts" className="text-brand">Accounts</Link> to build your monthly outlook. Investments and debt are included in <Link to="/net-worth" className="text-brand">Net Worth</Link>.</p>
      : <>
        <OutlookTable periods={periods} currency={currency} tomorrow={tomorrow} onPlan={openPlan} loading={loading} />
        <p className="mt-3 text-xs">Actuals run through today ({dateKey(today)}). Forecasts start tomorrow. Opening cash is reconstructed from current balances minus recorded cash movements in this cycle, including manually entered starting balances.</p>
        <p className="mt-2 text-xs">Forecasts include active monthly income, dated payment plans, installment schedules, and cash/bank subscriptions. Missing plans are not assumed to be zero spending. Credit-card purchases affect debt, not cash; plan repayments separately. Transfers between cash/bank accounts cancel out.</p>
        <p className="mt-2 text-xs">Income schedules and plans are not matched to early payments. Update or remove a plan if you pay early. Due or past plans are excluded from forecasts; record their actual payments in Transactions.</p>
        <details className="mt-5 rounded-2xl border border-line bg-card p-5">
          <summary className="cursor-pointer font-semibold">Payment plans ({data.plans.length})</summary>
          {!data.plans.length ? <p className="mt-3 text-sm">No payments planned yet. Add dated expenses and debt repayments to build your outlook.</p> : <ul className="mt-3 divide-y divide-line" aria-label="Payment plans">{data.plans.map(plan => <li key={plan.id} className="flex flex-wrap items-center justify-between gap-3 py-4">
            <div className="min-w-0 flex-1"><h3 className="break-words font-semibold">{plan.name}</h3><p className="mt-1 break-words text-xs">{plan.date} · {plan.account_name}{plan.destination_account_name ? ` → ${plan.destination_account_name}` : ` · ${plan.category_name ?? 'Uncategorized'}`} · {formatAmount(plan.amount, currency)}</p><p className="mt-1 text-xs">{plan.date <= dateKey(today) ? 'Due or past · excluded from forecast; record actual payment separately' : !data.accounts.some(account => account.id === plan.account_id) || (plan.destination_account_id && !data.accounts.some(account => account.id === plan.destination_account_id)) ? 'Account unavailable · excluded from forecast' : 'Planned · balances unchanged'}</p></div>
            <Button variant="outline" aria-label={`Edit plan ${plan.name}`} onClick={() => setEditing({ plan, date: plan.date })}>Edit</Button><Button variant="outline" aria-label={`Remove plan ${plan.name}`} onClick={() => { setDeleting(plan); setSaveError(null) }}>Remove</Button>
          </li>)}</ul>}
        </details>
      </>}
    {editing && data && currency && <PaymentPlanDialog data={data} date={editing.date} plan={editing.plan} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) setDeleting(null) }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>Remove payment plan?</DialogTitle><DialogDescription>Remove “{deleting?.name}” from your forecast. Account balances and actual transactions stay unchanged.</DialogDescription></DialogHeader>{saveError && <p role="alert">{saveError}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>Cancel</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? 'Removing…' : 'Remove plan'}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
