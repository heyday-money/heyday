import { useRef, useState, type FormEvent } from 'react'
import { Banknote, Building2, CreditCard, Landmark, TrendingUp, Wallet } from 'lucide-react'
import { createAccount, updateAccount, loanTypes, type LoanType, type Account, type AccountType } from '../lib/desktop'
import { decimalToInteger, fractionDigits } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { toast } from 'sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField as Field } from './FormField'
import { Textarea } from './ui/textarea'
import { NativeSelect } from './ui/native-select'

export const accountTypes = [
  { value: 'cash', label: 'Cash', groupLabel: 'Cash', icon: Banknote },
  { value: 'bank', label: 'Bank', groupLabel: 'Bank', icon: Building2 },
  { value: 'wallet', label: 'Digital Wallet', groupLabel: 'Wallets', icon: Wallet },
  { value: 'credit_card', label: 'Credit card', groupLabel: 'Credit cards', icon: CreditCard },
  { value: 'loan', label: 'Loan', groupLabel: 'Loans', icon: Landmark },
  { value: 'investment', label: 'Investment', groupLabel: 'Investments', icon: TrendingUp },
] as const
const inputStyle = 'mt-2 w-full rounded-[10px] border border-line bg-page px-3 py-2.5 text-[14px] text-ink focus-visible:outline-2 focus-visible:outline-brand'
function DayField({ name, label, value }: { name: string; label: string; value?: number | null }) {
  return <Field label={label}><NativeSelect name={name} className={inputStyle} defaultValue={value ?? ''}>
    <option value="">Not set</option>
    {Array.from({ length: 31 }, (_, i) => i + 1).map(day => <option key={day} value={day}>{day}</option>)}
  </NativeSelect></Field>
}

function amountText(value: string, currency: string) {
  const digits = fractionDigits(currency)
  const amount = BigInt(value), absolute = amount < 0n ? -amount : amount
  const scale = 10n ** BigInt(digits)
  return `${amount < 0n ? '-' : ''}${absolute / scale}${digits ? '.' + (absolute % scale).toString().padStart(digits, '0') : ''}`
}

export function AccountFormDialog({ account: editing, initialType, currency, onClose, onSaved }: {
  account?: Account; initialType: AccountType; currency: string; onClose: () => void; onSaved: (account: Account) => void
}) {
  const [type, setType] = useState<AccountType>(editing?.type ?? initialType)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [dirty, setDirty] = useState(false)
  const [confirmDiscard, setConfirmDiscard] = useState(false)
  const nameRef = useRef<HTMLInputElement>(null)
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  const debt = type === 'credit_card' || type === 'loan'
  const close = onClose
  const changeOpen = (next: boolean) => {
    if (next || saving) return
    if (dirty) setConfirmDiscard(true)
    else close()
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!currency || saving) return
    const data = new FormData(event.currentTarget)
    const text = (key: string) => String(data.get(key) ?? '').trim()
    const optional = (key: string) => text(key) || null
    const day = (key: string) => text(key) ? Number(text(key)) : null
    setError(null); setSaving(true)
    try {
      const digits = fractionDigits(currency)
      const loanType = type === 'loan' ? text('loan_type') : null
      if (type === 'loan' && !(editing?.type === 'loan' && editing.loan_type === null && !loanType) && !loanTypes.some(item => item.value === loanType)) throw new Error('Choose a loan type.')
      const details = {
        loan_type: (loanType || null) as LoanType | null,
        name: text('name'), type, currency,
        institution: type === 'cash' ? null : optional('institution'),
        last_four: optional('last_four'),
        notes: optional('notes'),
        credit_limit: type === 'credit_card' && text('credit_limit') ? decimalToInteger(text('credit_limit'), digits) : null,
        statement_day: type === 'credit_card' ? day('statement_day') : null,
        payment_due_day: debt ? day('payment_due_day') : null,
        interest_rate_millis: debt && text('interest') ? Number(decimalToInteger(text('interest'), 3)) : null,
      }
      const account = editing
        ? await updateAccount({ ...details, id: editing.id })
        : await createAccount({ ...details, opening_balance: decimalToInteger(text('balance'), digits) })
      onSaved(account)
      close(); toast.success(editing ? 'Account updated.' : 'Account added.', { id: 'account-saved' })
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not save account. Please try again.')
    } finally { setSaving(false) }
  }

  return <Dialog open onOpenChange={changeOpen}>
        <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[640px]" showCloseButton={!saving}
          onOpenAutoFocus={event => { event.preventDefault(); nameRef.current?.focus() }}
          onCloseAutoFocus={event => { event.preventDefault(); returnFocus.current?.isConnected && returnFocus.current.focus() }}
          onInteractOutside={event => event.preventDefault()}
          onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
          <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12">
            <DialogTitle>{editing ? 'Edit account' : 'Add account'}</DialogTitle>
            <DialogDescription>{editing ? 'Update your account details. Opening balance and account type are locked.' : 'Choose an account type and enter your starting balance.'}</DialogDescription>
          </DialogHeader>
          <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col" aria-label={editing ? 'Edit account' : 'Add account'}>
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              <fieldset disabled={saving}>
                <div className="grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                  <Field label="Account type"><NativeSelect disabled={!!editing} value={type} onChange={event => { setType(event.target.value as AccountType); setError(null) }} className={inputStyle}>{accountTypes.map(item => <option value={item.value} key={item.value}>{item.label}</option>)}</NativeSelect></Field>
                  {type === 'loan' && <Field label="Loan type"><NativeSelect name="loan_type" required={!editing || editing.loan_type !== null} defaultValue={editing?.loan_type ?? ''} className={inputStyle}><option value="" disabled={!editing || editing.loan_type !== null}>{editing ? 'Unclassified' : 'Choose a loan type'}</option>{loanTypes.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}</NativeSelect></Field>}
                  <Field label="Account name"><Input ref={nameRef} name="name" defaultValue={editing?.name ?? ''} required maxLength={100} className={inputStyle} placeholder="e.g. Everyday wallet" /></Field>
                  <Field label={`${editing ? 'Opening balance' : debt ? 'Amount owed' : type === 'investment' ? 'Current value' : 'Opening balance'} (${currency})`}><Input name="balance" inputMode="decimal" readOnly={!!editing} required defaultValue={editing ? amountText(editing.opening_balance, currency) : '0'} className={inputStyle} /></Field>
                </div>
                <p className="mt-4 text-[12px]">{editing ? 'Opening balance cannot be edited. Record transactions to update the current balance.' : debt ? 'Enter debt as a positive amount. Use a negative amount for an overpayment or credit.' : 'Enter the balance you own. Use a negative amount for an overdraft.'} {!editing && 'This is a manual starting balance.'}</p>
                <details open={editing ? true : undefined} className="mt-5 rounded-xl border border-line p-4">
                  <summary className="cursor-pointer text-[13px] font-semibold">Optional details</summary>
                  <div className="mt-4 grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                    {type !== 'cash' && <Field label="Institution (optional)"><Input name="institution" defaultValue={editing?.institution ?? ''} maxLength={100} className={inputStyle} placeholder={type === 'wallet' ? 'Wallet provider, e.g. TrueMoney' : type === 'investment' ? 'Broker or fund provider' : 'Bank or lender'} /></Field>}
                    <Field label="Last four digits (optional)"><Input name="last_four" defaultValue={editing?.last_four ?? ''} inputMode="numeric" pattern="[0-9]{4}" maxLength={4} className={inputStyle} /></Field>
                    {type === 'credit_card' && <><Field label={`Credit limit (${currency}, optional)`}><Input name="credit_limit" defaultValue={editing?.credit_limit != null ? amountText(editing.credit_limit, currency) : ''} inputMode="decimal" className={inputStyle} /></Field><DayField value={editing?.statement_day} name="statement_day" label="Statement day (optional)" /></>}
                    {debt && <><DayField value={editing?.payment_due_day} name="payment_due_day" label="Payment due day (optional)" /><Field label="Annual interest rate (%, optional)"><Input name="interest" defaultValue={editing?.interest_rate_millis != null ? interestRateText(String(editing.interest_rate_millis)) : ''} type="number" min="0" max="100" step="0.001" className={inputStyle} /></Field></>}
                  </div>
                  {debt && <p className="mt-3 text-[12px]">Scheduled days use month-end if unavailable. Payments and interest are not calculated automatically.</p>}
                  <div className="mt-4"><Field label="Notes (optional)"><Textarea name="notes" defaultValue={editing?.notes ?? ''} maxLength={1000} rows={2} className={inputStyle} /></Field></div>
                </details>
              </fieldset>
              {error && <p className="mt-4 text-[14px]" role="alert">{error}</p>}
            </div>
            {confirmDiscard ? <div className="shrink-0 border-t border-line px-6 py-4">
              <p className="mb-3 text-sm" role="alert">Discard your unsaved account?</p>
              <DialogFooter><Button type="button" variant="outline" onClick={() => setConfirmDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter>
            </div> : <DialogFooter className="shrink-0 border-t border-line px-6 py-4">
              <Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button>
              <Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save account'}</Button>
            </DialogFooter>}
          </form>
        </DialogContent>
  </Dialog>
}
