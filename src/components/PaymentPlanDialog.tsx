import { useRef, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { savePaymentPlan, type FinancialData, type PaymentPlan } from '../lib/desktop'
import { dateKey, isCash, isDebt } from '../lib/financial'
import { decimalToInteger, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

function editableAmount(amount: string, currency: string) {
  const digits = fractionDigits(currency), value = BigInt(amount), scale = 10n ** BigInt(digits)
  return `${value / scale}${digits ? '.' + (value % scale).toString().padStart(digits, '0') : ''}`
}
export function PaymentPlanDialog({ data, date, plan, onClose }: { data: FinancialData; date: string; plan?: PaymentPlan; onClose: () => void }) {
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  const currency = data.settings.currency!
  const [kind, setKind] = useState<'expense' | 'repayment'>(plan?.type ?? 'expense')
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const now = new Date()
  const tomorrow = dateKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1))
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
      const amount = decimalToInteger(String(form.get('amount')), fractionDigits(currency))
      if (BigInt(amount) <= 0n) throw new Error('Amount must be greater than zero.')
      await savePaymentPlan({ id: plan?.id ?? null, type: kind, name: String(form.get('name')).trim(), date: String(form.get('date')), amount, currency,
        account_id: String(form.get('account')), destination_account_id: kind === 'repayment' ? String(form.get('destination')) : null,
        category_id: kind === 'expense' ? String(form.get('category') ?? '') || null : null })
      onClose(); toast.success('Plan saved. Account balances are unchanged.')
    } catch (error) { setError(typeof error === 'string' ? error : error instanceof Error ? error.message : 'Could not save the plan.') } finally { setSaving(false) }
  }
  return <Dialog open onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!saving} onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
      <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>{plan ? 'Edit payment plan' : 'Add payment plan'}</DialogTitle><DialogDescription>A one-time forecast for a future date. Record the actual payment separately in Transactions.</DialogDescription></DialogHeader>
      <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 overflow-y-auto px-6 py-5">
          <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <Field label={`Planned amount (${currency})`}><Input className="mt-2" name="amount" inputMode="decimal" required defaultValue={plan ? editableAmount(plan.amount, currency) : ''} /></Field>
            <Field label="Plan name"><Input className="mt-2" name="name" maxLength={100} required defaultValue={plan?.name ?? ''} placeholder="e.g. Rent" /></Field>
            <Field label="Plan type"><NativeSelect className="mt-2" value={kind} onChange={event => setKind(event.target.value as 'expense' | 'repayment')}><option value="expense">Expense</option><option value="repayment">Debt repayment</option></NativeSelect></Field>
            <Field label="Planned date"><Input className="mt-2" type="date" name="date" min={tomorrow} max="9999-12-31" required defaultValue={date} /></Field>
            <Field label="Pay from"><NativeSelect className="mt-2" name="account" defaultValue={plan?.account_id ?? data.accounts.find(isCash)?.id ?? ''} required><option value="">Choose account</option>{data.accounts.filter(isCash).map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></Field>
            {kind === 'repayment' ? <Field label="Debt account"><NativeSelect className="mt-2" name="destination" defaultValue={plan?.destination_account_id ?? ''} required><option value="">Choose account</option>{data.accounts.filter(isDebt).map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></Field> : <Field label="Planned category"><NativeSelect className="mt-2" name="category" defaultValue={plan?.category_id ?? ''}><option value="">Uncategorized</option>{data.categories.map(category => <option key={category.id} value={category.id} disabled={category.is_archived}>{category.name}{category.is_archived ? ' (archived)' : ''}</option>)}</NativeSelect></Field>}
          </fieldset>
          {error && <p className="mt-4 text-sm" role="alert">{error}</p>}
        </div>
        <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">Discard unsaved plan changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={onClose}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save plan'}</Button></DialogFooter>}</div>
      </form>
    </DialogContent>
  </Dialog>
}
