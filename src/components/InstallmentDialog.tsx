import { useRef, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { saveInstallment, type FinancialData, type Installment } from '../lib/desktop'
import { dateKey, isCash } from '../lib/financial'
import { installmentSchedule, interestRateText } from '../lib/installments'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function InstallmentDialog({ data, plan, onClose }: { data: FinancialData; plan?: Installment; onClose: () => void }) {
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  const currency = data.settings.currency!
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits)
  const initial = BigInt(plan?.monthly_amount ?? '0')
  const [purchaseKind, setPurchaseKind] = useState<'new_purchase' | 'existing_purchase' | ''>('')
  const [purchaseAmount, setPurchaseAmount] = useState('')
  const [purchaseDate, setPurchaseDate] = useState(dateKey(new Date()))
  const [cardId, setCardId] = useState(plan?.debt_account_id ?? '')
  const newPurchase = !plan && purchaseKind === 'new_purchase'
  const [amount, setAmount] = useState(plan ? `${initial / scale}${digits ? '.' + (initial % scale).toString().padStart(digits, '0') : ''}` : '')
  const [interest, setInterest] = useState(plan ? plan.interest_rate_millis == null ? '' : interestRateText(plan.interest_rate_millis) : '0')
  const [count, setCount] = useState(String(plan?.installment_count ?? 12))
  const [date, setDate] = useState(plan?.first_due_date ?? dateKey(new Date()))
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [error, setError] = useState<string | null>(null)
  let preview: { total: string; end: string } | null = null
  try {
    const monthly = decimalToInteger(amount, digits)
    const schedule = installmentSchedule({ monthly_amount: monthly, installment_count: Number(count), first_due_date: date })
    preview = { total: formatAmount((BigInt(monthly) * BigInt(count)).toString(), currency), end: schedule.at(-1)!.date }
  } catch { /* Incomplete form fields do not have a schedule preview. */ }
  let balancePreview: { owed: string; available: string | null } | null = null
  try {
    const card = data.accounts.find(account => account.id === cardId)
    const value = BigInt(decimalToInteger(purchaseAmount, digits))
    if (newPurchase && card && value > 0n) {
      const owed = BigInt(card.current_balance) + value
      balancePreview = { owed: formatAmount(owed.toString(), currency), available: card.credit_limit == null ? null : formatAmount((BigInt(card.credit_limit) - owed).toString(), currency) }
    }
  } catch { /* Show the preview only for a valid purchase amount. */ }
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
      const interestRate = interest.trim() ? decimalToInteger(interest, 3) : null
      if (interestRate !== null && BigInt(interestRate) < 0n) throw new Error('Interest rate must be zero or greater.')
      if (!plan && !purchaseKind) throw new Error('Choose whether the purchase is already recorded.')
      const purchase = newPurchase ? { amount: decimalToInteger(purchaseAmount, digits), date: purchaseDate } : null
      if (purchase && (BigInt(purchase.amount) <= 0n || purchase.date > dateKey(new Date()) || purchase.date > date)) throw new Error('Enter a positive purchase amount and a purchase date no later than today or the first due date.')
      const input = { purchase, interest_rate_millis: interestRate, id: plan?.id ?? null, name: String(form.get('name')).trim(), account_id: String(form.get('account')), debt_account_id: cardId, monthly_amount: decimalToInteger(amount, digits), installment_count: Number(count), first_due_date: date, currency }
      if (!data.accounts.some(account => account.id === input.debt_account_id && account.type === 'credit_card')) throw new Error('Choose an active credit card.')
      installmentSchedule(input)
      await saveInstallment(input)
      onClose(); toast.success(newPurchase ? 'Purchase recorded and installment saved. Card balance updated.' : 'Installment saved. Account balances are unchanged.')
    } catch (error) { setError(typeof error === 'string' ? error : error instanceof Error ? error.message : 'Could not save the installment.') } finally { setSaving(false) }
  }
  return <Dialog open onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!saving} onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
      <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>{plan ? 'Edit installment' : 'Add installment'}</DialogTitle><DialogDescription>A fixed monthly repayment schedule for a credit card purchase. Payments are recorded separately.</DialogDescription></DialogHeader>
      <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 overflow-y-auto px-6 py-5">
          <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            {!plan && <div className="sm:col-span-2"><Field label="Purchase status"><NativeSelect className="mt-2" required value={purchaseKind} onChange={event => setPurchaseKind(event.target.value as typeof purchaseKind)}><option value="" disabled>Choose purchase status</option><option value="new_purchase">New purchase — add to card balance</option><option value="existing_purchase">Already recorded purchase — schedule only</option></NativeSelect></Field>
              <p className="mt-2 text-xs">{purchaseKind === 'new_purchase' ? 'Save records one credit card expense for the full purchase amount and increases the amount owed.' : purchaseKind === 'existing_purchase' ? 'Use this when the purchase is already in your card balance, including an opening balance. Saving will not add it again.' : 'Choose whether this purchase is already included in your credit card balance.'}</p>
            </div>}
            {newPurchase && <><Field label={`Purchase amount (${currency})`}><Input className="mt-2" inputMode="decimal" required value={purchaseAmount} onChange={event => setPurchaseAmount(event.target.value)} /></Field><Field label="Purchase date"><Input className="mt-2" type="date" min="0001-01-01" max={dateKey(new Date())} required value={purchaseDate} onChange={event => setPurchaseDate(event.target.value)} /></Field></>}
            <Field label="Installment name"><Input className="mt-2" name="name" required maxLength={100} defaultValue={plan?.name ?? ''} placeholder="e.g. Laptop or phone" /></Field>
            <Field label="Credit card"><NativeSelect className="mt-2" name="debt" required value={cardId} onChange={event => setCardId(event.target.value)} disabled={plan?.purchase_kind === 'new_purchase'}><option value="">Choose account</option>{plan && !data.accounts.some(account => account.id === plan.debt_account_id) && <option disabled value={plan.debt_account_id}>{plan.debt_account_name} (unavailable)</option>}{data.accounts.filter(account => account.type === 'credit_card').map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></Field>
            <Field label={`Monthly payment (${currency})`}><Input className="mt-2" inputMode="decimal" required value={amount} onChange={event => setAmount(event.target.value)} /></Field>
            <Field label="Annual interest rate (%, optional)"><Input className="mt-2" inputMode="decimal" placeholder="e.g. 0 or 1.195" value={interest} onChange={event => setInterest(event.target.value)} /></Field>
            <Field label="Number of installments"><Input className="mt-2" type="number" min={1} max={600} step={1} required value={count} onChange={event => setCount(event.target.value)} /></Field>
            <Field label="First due date"><Input className="mt-2" type="date" min="0001-01-01" max="9999-12-31" required value={date} onChange={event => setDate(event.target.value)} /></Field>
            <Field label="Pay from"><NativeSelect className="mt-2" name="account" required defaultValue={plan?.account_id ?? data.accounts.find(isCash)?.id ?? ''}><option value="">Choose account</option>{plan && !data.accounts.some(account => account.id === plan.account_id) && <option disabled value={plan.account_id}>{plan.account_name} (unavailable)</option>}{data.accounts.filter(isCash).map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></Field>
          </fieldset>
          <p className="mt-4 text-xs">Enter the monthly amount charged by your lender, including any interest or fees. The annual interest rate is for reference and does not recalculate the monthly payment. Use up to three decimal places, or leave it blank if unknown. Every installment uses the entered monthly amount. Dates use month-end when needed. An earlier first date lets you enter an existing schedule.</p>
          {balancePreview && <p className="mt-4 rounded-xl bg-soft p-3 text-sm" aria-label="Card balance after purchase">Amount owed after purchase: {balancePreview.owed}{balancePreview.available !== null && <span className="block">Available credit after purchase: {balancePreview.available}</span>}</p>}
          {preview && <p className="mt-4 rounded-xl bg-soft p-3 text-sm" role="status">Scheduled total: {preview.total}<span className="block">Last due date: {preview.end}</span></p>}
          <p className="mt-3 text-xs">{plan ? 'Editing changes only the schedule. Correct recorded purchases separately in Transactions.' : newPurchase ? 'The purchase amount is recorded once, separately from future interest and the repayment schedule.' : 'The purchase must already be included in your credit card balance; saving adds only the schedule.'} Payments are not automatically recorded or marked as paid. Upcoming payments are included in Outlook; avoid adding separate payment plans for the same installments.</p>
          {error && <p className="mt-4 text-sm" role="alert">{error}</p>}
        </div>
        <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">Discard unsaved installment changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={onClose}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save installment'}</Button></DialogFooter>}</div>
      </form>
    </DialogContent>
  </Dialog>
}
