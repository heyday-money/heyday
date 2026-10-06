import { t as translate, useLanguage } from "../lib/i18n"
import { InstitutionCombobox } from "./InstitutionCombobox"
import { useInstitutions } from "./InstitutionProvider"
import { sharedCredit, useCardLimits } from '../lib/card-limits'
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
  { value: 'cash', get label() { return translate("Cash") }, get groupLabel() { return translate("Cash") }, icon: Banknote },
  { value: 'bank', get label() { return translate("Bank") }, get groupLabel() { return translate("Bank") }, icon: Building2 },
  { value: 'wallet', get label() { return translate("Digital Wallet") }, get groupLabel() { return translate("Wallets") }, icon: Wallet },
  { value: 'credit_card', get label() { return translate("Credit card") }, get groupLabel() { return translate("Credit cards") }, icon: CreditCard },
  { value: 'loan', get label() { return translate("Loan") }, get groupLabel() { return translate("Loans") }, icon: Landmark },
  { value: 'investment', get label() { return translate("Investment") }, get groupLabel() { return translate("Investments") }, icon: TrendingUp },
] as const
const inputStyle = 'mt-2 w-full rounded-[10px] border border-line bg-page text-[14px] text-ink focus-visible:outline-2 focus-visible:outline-brand'
function DayField({ name, label, value }: { name: string; label: string; value?: number | null }) {
  useLanguage()

  return <Field label={label}><NativeSelect name={name} className={inputStyle} defaultValue={value ?? ''}>
    <option value="">{translate("Not set")}</option>
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
  useLanguage()

  const directory = useInstitutions()
  const limits = useCardLimits()
  const sharedLimit = editing ? sharedCredit(limits.data, editing.id) : null
  const preserveIndividualLimit = !!editing && (!!sharedLimit || limits.loading || limits.error)
  const [chosenInstitution, setChosenInstitution] = useState<{ id: string; name: string | null } | null>(null)
  const [unresolvedInstitution, setUnresolvedInstitution] = useState(false)
  const institutionId = chosenInstitution?.id ?? directory.accounts.find(a => a.id === editing?.id)?.institution_id ?? directory.institutions.find(i => i.name === editing?.institution)?.id ?? ''
  const draftInstitution = chosenInstitution ? chosenInstitution.name : institutionId ? null : editing?.institution ?? null
  const [type, setType] = useState<AccountType>(editing?.type ?? initialType)
  const [revolving, setRevolving] = useState(false)
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
    if (type !== 'cash' && (unresolvedInstitution || directory.loading || directory.error)) { setError("Select or add an institution, or choose No institution. If loading failed, retry first."); return }
    const data = new FormData(event.currentTarget)
    const text = (key: string) => String(data.get(key) ?? '').trim()
    const optional = (key: string) => text(key) || null
    const day = (key: string) => text(key) ? Number(text(key)) : null
    setError(null); setSaving(true)
    try {
      const digits = fractionDigits(currency)
      const loanType = type === 'loan' ? revolving && !editing ? 'personal_loan' : text('loan_type') : null
      if (type === 'loan' && !(editing?.type === 'loan' && editing.loan_type === null && !loanType) && !loanTypes.some(item => item.value === loanType)) throw new Error("Choose a loan type.")
      const details = {
        loan_type: (loanType || null) as LoanType | null,
        name: text('name'), type, currency,
        institution: type === 'cash' ? null : draftInstitution,
        institution_id: type === 'cash' ? null : institutionId || null,
        last_four: optional('last_four'),
        notes: optional('notes'),
        monthly_installment: editing?.payroll_linked ? editing.monthly_installment : type === 'loan' && text('monthly_installment') ? decimalToInteger(text('monthly_installment'), digits) : null,
        initial_loan_amount: type === 'loan' && text('initial_loan_amount') ? decimalToInteger(text('initial_loan_amount'), digits) : null,
        credit_limit: type === 'credit_card' ? preserveIndividualLimit ? editing!.credit_limit : text('credit_limit') ? decimalToInteger(text('credit_limit'), digits) : null : null,
        statement_day: type === 'credit_card' ? day('statement_day') : null,
        payment_due_day: debt ? day('payment_due_day') : null,
        interest_rate_ten_thousandths: debt && text('interest') ? Number(decimalToInteger(text('interest'), 4)) : null,
      }
      if (details.initial_loan_amount !== null && BigInt(details.initial_loan_amount) < 0n) throw new Error("Initial loan amount cannot be negative.")
      if (details.monthly_installment !== null && BigInt(details.monthly_installment) < 0n) throw new Error("Monthly installment cannot be negative.")
      const account = editing
        ? await updateAccount({ ...details, id: editing.id })
        : await createAccount({ ...details, opening_balance: decimalToInteger(text('balance'), digits), revolving_credit_limit: type === 'loan' && revolving ? decimalToInteger(text('revolving_limit'), digits) : null })
      onSaved(account)
      close(); toast.success(editing ? translate("Account updated.") : translate("Account added."), { id: 'account-saved' })
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : "Could not save account. Please try again.")
    } finally { setSaving(false) }
  }

  return <Dialog open onOpenChange={changeOpen}>
        <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[640px]" showCloseButton={!saving}
          onOpenAutoFocus={event => { event.preventDefault(); nameRef.current?.focus() }}
          onCloseAutoFocus={event => { event.preventDefault(); returnFocus.current?.isConnected && returnFocus.current.focus() }}
          onInteractOutside={event => event.preventDefault()}
          onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
          <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12">
            <DialogTitle>{editing ? translate("Edit account") : translate("Add account")}</DialogTitle>
            <DialogDescription>{editing ? translate("Update your account details. Opening balance and account type are locked.") : translate("Choose an account type and enter your starting balance.")}</DialogDescription>
          </DialogHeader>
          <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col" aria-label={editing ? translate("Edit account") : translate("Add account")}>
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              <fieldset disabled={saving}>
                <div className="grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                  <Field label={translate("Account type")}><NativeSelect disabled={!!editing} value={type} onChange={event => { setType(event.target.value as AccountType); setError(null) }} className={inputStyle}>{accountTypes.map(item => <option value={item.value} key={item.value}>{item.label}</option>)}</NativeSelect></Field>
                  {type === 'loan' && !revolving && <Field label={translate("Loan type")}><NativeSelect name="loan_type" required={!editing || editing.loan_type !== null} defaultValue={editing?.loan_type ?? ''} className={inputStyle}><option value="" disabled={!editing || editing.loan_type !== null}>{editing ? translate("Unclassified") : translate("Choose a loan type")}</option>{loanTypes.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}</NativeSelect></Field>}
                  {type === 'loan' && !editing && <><Field label={translate("Loan structure")}><NativeSelect value={revolving ? 'revolving' : 'single'} onChange={e => setRevolving(e.target.value === 'revolving')}><option value="single">{translate("Single loan")}</option><option value="revolving">{translate("Revolving credit / cash card (Personal Loan)")}</option></NativeSelect></Field>{revolving && <Field label={translate("Shared credit limit ({value0})", { value0: currency })}><Input name="revolving_limit" required inputMode="decimal" /></Field>}</>}
                  {type === 'loan' && !revolving && <div><Field label={translate("Monthly installment ({value0}, optional)", { value0: currency })}><Input readOnly={!!editing?.payroll_linked} aria-describedby={editing?.payroll_linked ? 'payroll-installment-note' : undefined} name="monthly_installment" defaultValue={editing?.monthly_installment != null ? amountText(editing.monthly_installment, currency) : ''} inputMode="decimal" placeholder={translate("Not set")} className={inputStyle} /></Field>{editing?.payroll_linked && <p id="payroll-installment-note" className="mt-2 text-xs text-muted">{translate("Deducted from salary")}. {translate("Monthly installment is read-only while linked to salary deductions. Manage the deduction or remove its account link in Income.")}</p>}</div>}
                  {type === 'loan' && !revolving && <Field label={translate("Initial Loan Amount ({value0}, optional)", { value0: currency })}><Input name="initial_loan_amount" defaultValue={editing?.initial_loan_amount != null ? amountText(editing.initial_loan_amount, currency) : ''} inputMode="decimal" placeholder={translate("Not set")} className={inputStyle} /></Field>}
                  <Field label={translate("Account name")}><Input ref={nameRef} name="name" defaultValue={editing?.name ?? ''} required maxLength={100} className={inputStyle} placeholder={translate("e.g. Everyday wallet")} /></Field>
                  <Field label={`${editing ? translate("Opening balance") : debt ? translate("Amount owed") : type === 'investment' ? translate("Current value") : translate("Opening balance")} (${currency})`}><Input name="balance" inputMode="decimal" readOnly={!!editing} required defaultValue={editing ? amountText(editing.opening_balance, currency) : '0'} className={inputStyle} /></Field>
                </div>
                <p className="mt-4 text-[12px]">{editing ? translate("Opening balance cannot be edited. Record transactions to update the current balance.") : debt ? translate("Enter debt as a positive amount. Use a negative amount for an overpayment or credit.") : translate("Enter the balance you own. Use a negative amount for an overdraft.")} {!editing && translate("This is a manual starting balance.")}</p>
                {type === 'loan' && <p className="mt-3 text-xs">{translate("Initial Loan Amount is the original borrowed principal, separate from the remaining debt entered as Amount owed. It is for reference only and does not change balances. For revolving cash cards, manage the approved Credit Limit in the loan overview.")}</p>}
                {type === 'loan' && <p className="mt-3 text-xs">{translate("Enter the full monthly installment, including interest and fees. Outlook subtracts linked payroll deductions first and forecasts only the uncovered amount. Explicit cycle entries remain additional payments. Leave blank if unknown; zero means no scheduled payment. Saving does not change your balance.")}{currency !== 'THB' && translate(" This account is excluded from the THB planner; no currency conversion is applied.")}</p>}
                <details open={editing ? true : undefined} className="mt-5 rounded-xl border border-line p-4">
                  <summary className="cursor-pointer text-[13px] font-semibold">{translate("Optional details")}</summary>
                  <div className="mt-4 grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                    {type !== 'cash' && <Field label={translate("Institution (optional)")}><InstitutionCombobox disabled={saving} value={institutionId} draftName={draftInstitution} onChange={(id, name) => { setChosenInstitution({ id, name }); setDirty(true); setError(null) }} onUnresolvedChange={setUnresolvedInstitution} /></Field>}
                    <Field label={translate("Last four digits (optional)")}><Input name="last_four" defaultValue={editing?.last_four ?? ''} inputMode="numeric" pattern="[0-9]{4}" maxLength={4} className={inputStyle} /></Field>
                    {type === 'credit_card' && <>
                      {editing && limits.loading ? <p role="status" className="text-sm">{translate("Loading credit limit…")}</p> : editing && limits.error ? <div role="alert" className="text-sm">{translate("Could not load credit limit.")}{" "}<Button type="button" variant="outline" onClick={() => void limits.reload()}>{translate("Retry")}</Button></div> : sharedLimit ? <div className="col-span-full rounded-xl bg-soft p-3">
                        <Field label={translate("Shared credit limit ({value0})", { value0: currency })}><Input readOnly value={amountText(sharedLimit.credit_limit, currency)} className={inputStyle} /></Field>
                        <p className="mt-2 text-sm font-medium">{translate("Shared group:")}{" "}{sharedLimit.name}</p>
                        <p className="mt-1 text-xs">{translate("This limit is shared by all cards in the group. To change it, edit the shared group under Accounts → Credit cards.")}</p>
                      </div> : <Field label={translate("Credit limit ({value0}, optional)", { value0: currency })}><Input name="credit_limit" defaultValue={editing?.credit_limit != null ? amountText(editing.credit_limit, currency) : ''} inputMode="decimal" className={inputStyle} /></Field>}
                      <DayField value={editing?.statement_day} name="statement_day" label={translate("Statement day (optional)")} />
                    </>}
                    {debt && <><DayField value={editing?.payment_due_day} name="payment_due_day" label={translate("Payment due day (optional)")} /><Field label={translate("Annual interest rate (%, optional)")}><Input name="interest" defaultValue={editing?.interest_rate_ten_thousandths != null ? interestRateText(String(editing.interest_rate_ten_thousandths), 4) : ''} type="number" min="0" max="100" step="0.0001" className={inputStyle} /></Field></>}
                  </div>
                  {debt && <p className="mt-3 text-[12px]">{translate("Scheduled days use month-end if unavailable. Payments and interest are not calculated automatically.")}</p>}
                  <div className="mt-4"><Field label={translate("Notes (optional)")}><Textarea name="notes" defaultValue={editing?.notes ?? ''} maxLength={1000} rows={2} className={inputStyle} /></Field></div>
                </details>
              </fieldset>
              {error && <p className="mt-4 text-[14px]" role="alert">{translate(error)}</p>}
            </div>
            {confirmDiscard ? <div className="shrink-0 border-t border-line px-6 py-4">
              <p className="mb-3 text-sm" role="alert">{translate("Discard your unsaved account?")}</p>
              <DialogFooter><Button type="button" variant="outline" onClick={() => setConfirmDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard changes")}</Button></DialogFooter>
            </div> : <DialogFooter className="shrink-0 border-t border-line px-6 py-4">
              <Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>{translate("Cancel")}</Button>
              <Button type="submit" disabled={saving}>{saving ? translate("Saving…") : translate("Save account")}</Button>
            </DialogFooter>}
          </form>
        </DialogContent>
  </Dialog>
}
