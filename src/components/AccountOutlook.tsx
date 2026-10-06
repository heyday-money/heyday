import { SelectiveDefaultWarning } from './SelectiveDefault'
import { exclusionFor, cycleForDate } from '../lib/selective-defaults'
import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { AccountLabel } from './InstitutionLogo'
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

export function AccountOutlook() {
  useLanguage()

  const { data, loading, error, today, reload } = useFinancialData()
  const [editing, setEditing] = useState<{ date: string; plan?: PaymentPlan } | null>(null)
  const [deleting, setDeleting] = useState<PaymentPlan | null>(null)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const tomorrow = dateKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1))
  async function remove() {
    if (!deleting || saving) return
    setSaving(true); setSaveError(null)
    try { await deletePaymentPlan(deleting.id); setDeleting(null); toast.success(translate("Plan removed.")) }
    catch (error) { setSaveError(typeof error === 'string' ? error : "Could not remove the plan.") }
    finally { setSaving(false) }
  }
  const currency = data?.settings.currency
  const periods = useMemo(() => data ? monthlyOutlook(data, today) : [], [data, today, getLanguage()])
  const visiblePlans = data?.plans.filter(plan => data.accounts.some(account => account.id === plan.account_id) && (!plan.destination_account_id || data.accounts.some(account => account.id === plan.destination_account_id))) ?? []
  const openPlan = useCallback((date: string) => setEditing({ date }), [])
  return <section className="min-w-0" aria-labelledby="monthly-outlook-title">
    <div className="mb-5 flex flex-wrap items-start justify-between gap-4"><div><h2 id="monthly-outlook-title" className="text-2xl font-semibold">{translate("Monthly Outlook")}</h2></div><Button disabled={!desktopAvailable || !data || !currency || error || loading || !data.accounts.some(isCash)} onClick={() => setEditing({ date: tomorrow })}><Plus size={16} />{translate("Add payment plan")}</Button></div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to see your local monthly outlook.")}</p>
      : error ? <div role="alert">{translate("Could not load your monthly outlook.")}{" "}<Button variant="outline" onClick={reload}>{translate("Retry outlook")}</Button></div>
      : !data ? <p role="status">{translate("Loading monthly outlook…")}</p>
      : !currency ? <p className="rounded-xl bg-soft p-5">{translate("Choose your currency in")}{" "}<Link to="/settings" className="text-brand">{translate("Settings")}</Link> {" "}{translate("to start planning.")}</p>
      : !data.accounts.some(isCash) ? <p className="rounded-xl bg-soft p-5">{translate("Add a cash, bank, or digital wallet account in")}{" "}<Link to="/accounts" className="text-brand">{translate("Accounts")}</Link> {" "}{translate("to build your monthly outlook. Investments and debt are included in")}{" "}<Link to="/net-worth" className="text-brand">{translate("Net Worth")}</Link>.</p>
      : <>
        <OutlookTable periods={periods} currency={currency} tomorrow={tomorrow} onPlan={openPlan} loading={loading} />
        <p className="mt-3 text-xs">{translate("Actuals run through today (")}{dateKey(today)}{translate("). Scheduled forecasts start tomorrow; explicit card payment plans may include today. Opening cash is reconstructed from current balances minus recorded cash movements in this cycle, including manually entered starting balances.")}</p>
        <p className="mt-2 text-xs">{translate("Forecasts include active monthly income, dated payment plans, installment schedules, and cash/bank/wallet subscriptions. Missing plans are not assumed to be zero spending. Credit-card purchases affect debt, not cash. Card Billing plans add only remaining explicitly linked payments and replace included installment occurrences. Transfers between cash/bank/wallet accounts cancel out.")}</p>
        <p className="mt-2 text-xs">{translate("Income schedules and general payment plans are not matched to early payments. Card Billing plans use explicit payment links. Update or remove a plan if you pay early. Due or past plans are excluded from forecasts; record their actual payments in Transactions.")}</p>
        <details className="mt-5 rounded-2xl border border-line bg-card p-5">
          <summary className="cursor-pointer font-semibold">{translate("Payment plans (")}{visiblePlans.length})</summary>
          {!visiblePlans.length ? <p className="mt-3 text-sm">{translate("No payments planned yet. Add dated expenses and debt repayments to build your outlook.")}</p> : <ul className="mt-3 divide-y divide-line" aria-label={translate("Payment plans")}>{visiblePlans.map(plan => <li key={plan.id} className="flex flex-wrap items-center justify-between gap-3 py-4">
            <div className="min-w-0 flex-1"><h3 className="break-words font-semibold">{plan.name} <SelectiveDefaultWarning period={exclusionFor(data.selective_defaults,plan.destination_account_id,cycleForDate(plan.date,data.settings.period_start_day))} /></h3><p className="mt-1 break-words text-xs">{plan.date} · <Link to="/accounts/$accountId/details" params={{ accountId: plan.account_id }} className="text-brand hover:underline"><AccountLabel id={plan.account_id} name={plan.account_name} /></Link>{plan.destination_account_name && plan.destination_account_id ? <> → <Link to="/accounts/$accountId/details" params={{ accountId: plan.destination_account_id }} className="text-brand hover:underline"><AccountLabel id={plan.destination_account_id} name={plan.destination_account_name} /></Link></> : translate(" · {value0}", { value0: plan.category_name ?? translate("Uncategorized") })} · {formatAmount(plan.amount, currency)}</p><p className="mt-1 text-xs">{exclusionFor(data.selective_defaults,plan.destination_account_id,cycleForDate(plan.date,data.settings.period_start_day)) ? translate('Excluded by Selective Default') : plan.date <= dateKey(today) ? translate("Due or past · excluded from forecast; record actual payment separately") : !data.accounts.some(account => account.id === plan.account_id) || (plan.destination_account_id && !data.accounts.some(account => account.id === plan.destination_account_id)) ? translate("Account unavailable · excluded from forecast") : translate("Planned · balances unchanged")}</p></div>
            <Button variant="outline" aria-label={translate("Edit plan {value0}", { value0: plan.name })} onClick={() => setEditing({ plan, date: plan.date })}>{translate("Edit")}</Button><Button variant="outline" aria-label={translate("Remove plan {value0}", { value0: plan.name })} onClick={() => { setDeleting(plan); setSaveError(null) }}>{translate("Remove")}</Button>
          </li>)}</ul>}
        </details>
      </>}
    {editing && data && currency && <PaymentPlanDialog data={data} date={editing.date} plan={editing.plan} onClose={() => setEditing(null)} />}
    <Dialog open={!!deleting} onOpenChange={open => { if (!open && !saving) setDeleting(null) }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()}><DialogHeader><DialogTitle>{translate("Remove payment plan?")}</DialogTitle><DialogDescription>{translate("Remove “")}{deleting?.name}{translate("” from your forecast. Account balances and actual transactions stay unchanged.")}</DialogDescription></DialogHeader>{saveError && <p role="alert">{translate(saveError)}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setDeleting(null)}>{translate("Cancel")}</Button><Button variant="destructive" disabled={saving} onClick={remove}>{saving ? translate("Removing…") : translate("Remove plan")}</Button></DialogFooter></DialogContent></Dialog>
  </section>
}
