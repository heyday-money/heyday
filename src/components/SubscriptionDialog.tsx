import { t as translate, useLanguage } from "../lib/i18n"
import { LogoPicker } from './LogoPicker'
import { useLogoAsset } from './LogoImage'
import type { LogoChange } from '../lib/logos'
import { subscriptionPlatforms, validateManagementUrl } from '../lib/subscription-management'
import { AccountSelect } from './AccountSelect'
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
  useLanguage()

  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  const currency = data.settings.currency!
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits)
  const initial = BigInt(subscription?.amount ?? '0')
  const [name, setName] = useState(subscription?.name ?? '')
  const [logoChange, setLogoChange] = useState<LogoChange>()
  const savedLogo = useLogoAsset(subscription?.logo_asset_id)
  const [logoBusy, setLogoBusy] = useState(false)
  const logoBusyRef = useRef(false)
  const saveLock = useRef(false)
  const [accountId, setAccountId] = useState(subscription?.account_id ?? '')
  const [categoryId, setCategoryId] = useState(subscription?.category_id ?? '')
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [error, setError] = useState<string | null>(null)
  function changeOpen(open: boolean) {
    if (open || saveLock.current || logoBusyRef.current) return
    if (dirty) setDiscard(true)
    else onClose()
  }
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (saveLock.current || logoBusyRef.current) return
    saveLock.current = true
    const form = new FormData(event.currentTarget)
    setSaving(true); setError(null)
    try {
      const input = { logo_change: logoChange, managed_via: (String(form.get('managed_via')) || null) as Subscription['managed_via'], management_url: validateManagementUrl(String(form.get('management_url') ?? '')), id: subscription?.id ?? null, name: String(form.get('name')).trim(), amount: decimalToInteger(String(form.get('amount')), digits), frequency: String(form.get('frequency')) as Subscription['frequency'], first_billing_date: String(form.get('first')), end_date: String(form.get('end')) || null, is_active: form.get('active') === 'true', account_id: accountId, category_id: categoryId || null, currency }
      if (BigInt(input.amount) <= 0n) throw new Error("Amount must be greater than zero.")
      subscriptionDates(input, input.first_billing_date, input.end_date ?? input.first_billing_date)
      const available = data.accounts.some(account => account.id === accountId && ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type))
      if (!available && (input.is_active || accountId !== subscription?.account_id)) throw new Error("Choose an active cash, bank, digital wallet, or credit card account.")
      await saveSubscription(input)
      onClose(); toast.success(translate("Subscription saved. Account balances are unchanged."))
    } catch (error) { setError(typeof error === 'string' ? error : error instanceof Error ? error.message : "Could not save the subscription.") } finally { saveLock.current = false; setSaving(false) }
  }
  return <Dialog open onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!saving && !logoBusy} onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saveLock.current || logoBusyRef.current) event.preventDefault() }}>
      <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>{subscription ? translate("Edit subscription") : translate("Add subscription")}</DialogTitle><DialogDescription>{translate("Plan a recurring charge. Record actual payments separately in Transactions.")}</DialogDescription></DialogHeader>
      <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 overflow-y-auto px-6 py-5">
          <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label={translate("Subscription name")}><Input className="mt-2" name="name" required maxLength={100} value={name} onChange={event => setName(event.target.value)} placeholder={translate("e.g. Netflix or cloud storage")} /></Field>
            <Field label={translate("Amount per charge ({value0})", { value0: currency })}><Input className="mt-2" name="amount" inputMode="decimal" required defaultValue={subscription ? `${initial / scale}${digits ? '.' + (initial % scale).toString().padStart(digits, '0') : ''}` : ''} /></Field>
            <div className="sm:col-span-2"><LogoPicker name={name} value={logoChange} savedSource={savedLogo} hasSavedLogo={!!subscription?.logo_asset_id} disabled={saving} onChange={change => { setLogoChange(change); setDirty(true) }} onBusyChange={busy => { logoBusyRef.current = busy; setLogoBusy(busy) }} /></div>
            <Field label={translate("Managed through")}><NativeSelect className="mt-2" name="managed_via" defaultValue={subscription?.managed_via ?? ''}><option value="">{translate("Not specified")}</option>{subscriptionPlatforms.map(platform => <option key={platform.value} value={platform.value}>{platform.label}</option>)}</NativeSelect></Field>
            <Field label={translate("Management link (optional)")}><Input className="mt-2" name="management_url" type="url" maxLength={2048} defaultValue={subscription?.management_url ?? ''} placeholder="https://…" /></Field>
            <p className="text-xs sm:col-span-2">{translate("Choose where you subscribed. Add an HTTPS link to open its subscription settings in your browser.")}</p>
            <Field label={translate("Billing frequency")}><NativeSelect className="mt-2" name="frequency" defaultValue={subscription?.frequency ?? 'monthly'}><option value="monthly">{translate("Monthly")}</option><option value="yearly">{translate("Yearly")}</option></NativeSelect></Field>
            <Field label={translate("Pay from")}><AccountSelect className="mt-2" required value={accountId} onChange={event => setAccountId(event.target.value)}><option value="">{translate("Choose account")}</option>{subscription && !data.accounts.some(account => account.id === subscription.account_id) && <option value={subscription.account_id}>{subscription.account_name} {" "}{translate("(unavailable)")}</option>}{data.accounts.filter(account => ['cash', 'bank', 'wallet', 'credit_card'].includes(account.type)).map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</AccountSelect></Field>
            <Field label={translate("First billing date")}><Input className="mt-2" name="first" type="date" min="0001-01-01" max="9999-12-31" required defaultValue={subscription?.first_billing_date ?? dateKey(new Date())} /></Field>
            <Field label={translate("End date (optional)")}><Input className="mt-2" name="end" type="date" min="0001-01-01" max="9999-12-31" defaultValue={subscription?.end_date ?? ''} /></Field>
            <Field label={translate("Category (optional)")}><NativeSelect className="mt-2" value={categoryId} onChange={event => setCategoryId(event.target.value)}><option value="">{translate("Uncategorized")}</option>{data.categories.filter(category => !category.is_archived || category.id === subscription?.category_id).map(category => <option key={category.id} value={category.id}>{category.name}{category.is_archived ? translate(" (archived)") : ''}</option>)}</NativeSelect></Field>
            <Field label={translate("Subscription status")}><NativeSelect className="mt-2" name="active" defaultValue={String(subscription?.is_active ?? true)}><option value="true">{translate("Active")}</option><option value="false">{translate("Paused")}</option></NativeSelect></Field>
          </fieldset>
          <p className="mt-4 text-xs">{translate("Billing follows the original date, using month-end when needed. Leave the end date blank to continue indefinitely. Paused subscriptions are excluded from forecasts.")}</p>
          <p className="mt-3 text-xs">{translate("Cash, bank, and wallet charges appear in Outlook after today. Credit card charges do not reduce cash; plan card repayments separately. Saving never creates a transaction or changes balances. Avoid adding duplicate payment plans for these charges.")}</p>
          {error && <p className="mt-4 text-sm" role="alert">{translate(error)}</p>}
        </div>
        <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">{translate("Discard unsaved subscription changes?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={onClose}>{translate("Discard changes")}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving || logoBusy} onClick={() => changeOpen(false)}>{translate("Cancel")}</Button><Button type="submit" disabled={saving || logoBusy}>{saving ? translate("Saving…") : translate("Save subscription")}</Button></DialogFooter>}</div>
      </form>
    </DialogContent>
  </Dialog>
}
