import { useRef, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { saveSubscription, type FinancialData, type Subscription } from '../lib/desktop'
import { dateKey } from '../lib/financial'
import { subscriptionDates } from '../lib/subscriptions'
import { decimalToInteger, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function SubscriptionDialog({ data, subscription, onClose }: { data: FinancialData; subscription?: Subscription; onClose: () => void }) {
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  const currency = data.settings.currency!
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits)
  const initial = BigInt(subscription?.amount ?? '0')
  const [accountId, setAccountId] = useState(subscription?.account_id ?? '')
  const [categoryId, setCategoryId] = useState(subscription?.category_id ?? '')
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [error, setError] = useState<string | null>(null)
  function changeOpen(open: boolean) {
    if (open || saving) return
    if (dirty) setDiscard(true)
    else onClose()
  }
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (saving) return
    const form = new FormData(event.currentTarget)
    setSaving(true); setError(null)
    try {
      const input = { id: subscription?.id ?? null, name: String(form.get('name')).trim(), amount: decimalToInteger(String(form.get('amount')), digits), frequency: String(form.get('frequency')) as Subscription['frequency'], first_billing_date: String(form.get('first')), end_date: String(form.get('end')) || null, is_active: form.get('active') === 'true', account_id: accountId, category_id: categoryId || null, currency }
      if (BigInt(input.amount) <= 0n) throw new Error('Amount must be greater than zero.')
      subscriptionDates(input, input.first_billing_date, input.end_date ?? input.first_billing_date)
      const available = data.accounts.some(account => account.id === accountId && ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
      if (!available && (input.is_active || accountId !== subscription?.account_id)) throw new Error('Choose an active cash, bank, digital wallet, or credit card account.')
      await saveSubscription(input)
      onClose(); toast.success('Subscription saved. Account balances are unchanged.')
    } catch (error) { setError(typeof error === 'string' ? error : error instanceof Error ? error.message : 'Could not save the subscription.') } finally { setSaving(false) }
  }
  return <Dialog open onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!saving} onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
      <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>{subscription ? 'Edit subscription' : 'Add subscription'}</DialogTitle><DialogDescription>Plan a recurring charge. Record actual payments separately in Transactions.</DialogDescription></DialogHeader>
      <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 overflow-y-auto px-6 py-5">
          <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label="Subscription name"><Input className="mt-2" name="name" required maxLength={100} defaultValue={subscription?.name ?? ''} placeholder="e.g. Netflix or cloud storage" /></Field>
            <Field label={`Amount per charge (${currency})`}><Input className="mt-2" name="amount" inputMode="decimal" required defaultValue={subscription ? `${initial / scale}${digits ? '.' + (initial % scale).toString().padStart(digits, '0') : ''}` : ''} /></Field>
            <Field label="Billing frequency"><NativeSelect className="mt-2" name="frequency" defaultValue={subscription?.frequency ?? 'monthly'}><option value="monthly">Monthly</option><option value="yearly">Yearly</option></NativeSelect></Field>
            <Field label="Pay from"><NativeSelect className="mt-2" required value={accountId} onChange={event => setAccountId(event.target.value)}><option value="">Choose account</option>{subscription && !data.accounts.some(account => account.id === subscription.account_id) && <option value={subscription.account_id}>{subscription.account_name} (unavailable)</option>}{data.accounts.filter(account => ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type)).map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></Field>
            <Field label="First billing date"><Input className="mt-2" name="first" type="date" min="0001-01-01" max="9999-12-31" required defaultValue={subscription?.first_billing_date ?? dateKey(new Date())} /></Field>
            <Field label="End date (optional)"><Input className="mt-2" name="end" type="date" min="0001-01-01" max="9999-12-31" defaultValue={subscription?.end_date ?? ''} /></Field>
            <Field label="Category (optional)"><NativeSelect className="mt-2" value={categoryId} onChange={event => setCategoryId(event.target.value)}><option value="">Uncategorized</option>{data.categories.filter(category => !category.is_archived || category.id === subscription?.category_id).map(category => <option key={category.id} value={category.id}>{category.name}{category.is_archived ? ' (archived)' : ''}</option>)}</NativeSelect></Field>
            <Field label="Subscription status"><NativeSelect className="mt-2" name="active" defaultValue={String(subscription?.is_active ?? true)}><option value="true">Active</option><option value="false">Paused</option></NativeSelect></Field>
          </fieldset>
          <p className="mt-4 text-xs">Billing follows the original date, using month-end when needed. Leave the end date blank to continue indefinitely. Paused subscriptions are excluded from forecasts.</p>
          <p className="mt-3 text-xs">Cash, bank, and wallet charges appear in Outlook after today. Credit card charges do not reduce cash; plan card repayments separately. Saving never creates a transaction or changes balances. Avoid adding duplicate payment plans for these charges.</p>
          {error && <p className="mt-4 text-sm" role="alert">{error}</p>}
        </div>
        <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">Discard unsaved subscription changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={onClose}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save subscription'}</Button></DialogFooter>}</div>
      </form>
    </DialogContent>
  </Dialog>
}
