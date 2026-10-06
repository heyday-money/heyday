import { t as translate, useLanguage } from "../lib/i18n"
import { getLoanAccount, recordLoanRepayment, type LoanContract } from '../lib/loans'
import { AccountCombobox } from './AccountCombobox'
import { sharedCreditAfter, useCardLimits } from '../lib/card-limits'
import { CategorySelect } from './CategoryIcon'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { createTransaction, listIncomes, type Income, getSettings, listAccounts, listTransactionOptions, type TransactionOptions, type Account, type TransactionType } from '../lib/desktop'
import { decimalToInteger, fractionDigits, formatAmount } from '../lib/money'
import { PayeeCombobox } from './PayeeCombobox'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const labels = { get income() { return translate("Money received · In") }, get expense() { return translate("Expense · Out") }, get transfer() { return translate("Transfer") }, get repayment() { return translate("Repayment · Transfer") } }
const control = 'mt-2 w-full'
const rememberedAccountKey = 'transaction-last-account'
function initialAccount(accounts: Account[]) {
  let remembered: string | null = null
  try { remembered = localStorage.getItem(rememberedAccountKey) } catch { /* Preferences are optional. */ }
  return accounts.find(account => account.id === remembered)?.id ?? accounts[0]?.id ?? ''
}
function today() {
  const date = new Date()
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

// Mounted for each entry so defaults and active accounts are refreshed each time.
export function AddTransactionDialog({ onClose, initialAccountId }: { onClose: () => void; initialAccountId?: string }) {
  useLanguage()

  const limits = useCardLimits()
  const [accounts, setAccounts] = useState<Account[]>([])
  const [currency, setCurrency] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [kind, setKind] = useState<TransactionType>('expense')
  const [accountId, setAccountId] = useState('')
  const [destinationId, setDestinationId] = useState('')
  const [interest, setInterest] = useState('0')
  const [fee, setFee] = useState('0')
  const [contracts, setContracts] = useState<LoanContract[]>([])
  const [contractId, setContractId] = useState('')
  const [loanLoadedId, setLoanLoadedId] = useState('')
  const [loanLoadError, setLoanLoadError] = useState(false)
  const [loanAttempt, setLoanAttempt] = useState(0)
  const [sourceCleared, setSourceCleared] = useState(false)
  const [destinationCleared, setDestinationCleared] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [saveMode, setSaveMode] = useState<'close' | 'another' | null>(null)
  const [creatingPayee, setCreatingPayee] = useState(false)
  const [unresolvedPayee, setUnresolvedPayee] = useState(false)
  const creatingPayeeRef = useRef(false)
  const saving = saveMode !== null || creatingPayee
  const savingRef = useRef(false)
  const focusNextAmount = useRef(false)
  const [amount, setAmount] = useState('')
  const [description, setDescription] = useState('')
  const [payeeId, setPayeeId] = useState('')
  const [incomeSourceId, setIncomeSourceId] = useState('')
  const [incomes, setIncomes] = useState<Income[]>([])
  const [incomeLoad, setIncomeLoad] = useState<'loading' | 'ready' | 'error'>('loading')
  const [incomeAttempt, setIncomeAttempt] = useState(0)
  const [categoryId, setCategoryId] = useState('')
  const [options, setOptions] = useState<TransactionOptions>({ payees: [], categories: [] })
  const [error, setError] = useState<string | null>(null)
  const amountRef = useRef<HTMLInputElement>(null)
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  useEffect(() => {
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(), listTransactionOptions()]).then(([settings, accounts, options]) => {
      if (active) {
        setCurrency(settings.currency); setAccounts(accounts); setOptions(options)
        const viewedAccount = accounts.find(account => account.id === initialAccountId)
        if (viewedAccount?.type === 'loan') {
          setKind('repayment')
          setDestinationId(viewedAccount.id)
          setAccountId(initialAccount(accounts.filter(account => ['cash', 'bank', 'wallet'].includes(account.type))))
        } else {
          setAccountId(viewedAccount?.id ?? initialAccount(accounts))
        }
      }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt, initialAccountId])
  useEffect(() => { if (!loading) amountRef.current?.focus() }, [loading])
  useEffect(() => {
    if (!saving && focusNextAmount.current) {
      focusNextAmount.current = false
      amountRef.current?.focus()
    }
  }, [saving])
  useEffect(() => {
    if (kind !== 'income') return
    let active = true
    setIncomeLoad('loading')
    listIncomes().then(rows => { if (active) { setIncomes(rows); setIncomeLoad('ready') } }).catch(() => { if (active) setIncomeLoad('error') })
    return () => { active = false }
  }, [kind, incomeAttempt])
  const loanRepayment = kind === 'repayment' && accounts.some(a => a.id === destinationId && a.type === 'loan')
  useEffect(() => {
    setInterest('0'); setFee('0')
  }, [loanRepayment, destinationId])
  useEffect(() => {
    setContractId(''); setContracts([]); setLoanLoadedId(''); setLoanLoadError(false)
    if (!loanRepayment) return
    let active = true
    getLoanAccount(destinationId).then(data => {
      if (!active) return
      setContracts(data.contracts)
      setContractId(data.contracts.length === 1 ? data.contracts[0].id : '')
      setLoanLoadedId(destinationId)
    }).catch(() => { if (active) setLoanLoadError(true) })
    return () => { active = false }
  }, [loanRepayment, destinationId, loanAttempt])
  const loanReady = loanLoadedId === destinationId && !loanLoadError
  let principal: bigint | null = null
  try {
    if (currency && amount) {
      const digits = fractionDigits(currency)
      const charges = [interest, fee].map(value => BigInt(decimalToInteger(value, digits)))
      if (charges.every(value => value >= 0n)) principal = BigInt(decimalToInteger(amount, digits)) - charges[0] - charges[1]
    }
  } catch { /* The draft may contain an incomplete amount. */ }
  const paired = kind === 'transfer' || kind === 'repayment'
  const affectedGroups = limits.data.groups.filter(g => limits.data.cards.some(c => c.group_id === g.id && (c.id === accountId || (paired && c.id === destinationId))))
  let previewAmount: bigint | null = null
  try { if (currency) { const value = BigInt(decimalToInteger(amount, fractionDigits(currency))); if (value > 0n) previewAmount = value } } catch { /* Incomplete amount. */ }
  const close = onClose
  function changeOpen(next: boolean) {
    if (next || saving) return
    if (dirty) setDiscard(true)
    else close()
  }
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!currency || savingRef.current || creatingPayeeRef.current) return
    if (kind === 'expense' && unresolvedPayee) { setError("Select or create a payee, or choose No payee."); return }
    const submitter = (event.nativeEvent as SubmitEvent).submitter
    const addAnother = submitter instanceof HTMLButtonElement && submitter.value === 'another'
    const data = new FormData(event.currentTarget)
    savingRef.current = true
    setSaveMode(addAnother ? 'another' : 'close'); setError(null)
    try {
      const amount = decimalToInteger(String(data.get('amount')), fractionDigits(currency))
      if (BigInt(amount) <= 0n) throw new Error("Amount must be greater than zero.")
      const clearedIds = [sourceCleared ? accountId : '', paired && destinationCleared ? destinationId : ''].filter(id => accounts.some(account => account.id === id && ['bank', 'wallet', 'credit_card'].includes(account.type)))
      if (loanRepayment) {
        if (!loanReady) throw new Error("Load the loan details before saving.")
        if (contracts.length && !contractId) throw new Error("Choose a contract for this loan repayment.")
        if (principal === null || principal < 0n) throw new Error("Interest and fees cannot exceed the total payment.")
        await recordLoanRepayment({ contract_id: contractId || null, loan_account_id: destinationId, account_id: accountId,
          total: amount, principal: principal.toString(), interest: decimalToInteger(interest, fractionDigits(currency)), fee: decimalToInteger(fee, fractionDigits(currency)),
          date: String(data.get('date')), currency, description: description.trim(), cleared: sourceCleared })
      } else await createTransaction({ income_source_id: kind === 'income' ? incomeSourceId || null : null, cleared_account_ids: clearedIds, type: kind, account_id: accountId, destination_account_id: paired ? destinationId : null, amount, currency, date: String(data.get('date')), description: String(data.get('description') ?? '').trim(), payee_id: kind === 'expense' ? payeeId || null : null, category_id: kind === 'expense' ? categoryId || null : null })
      try { localStorage.setItem(rememberedAccountKey, accountId) } catch { /* Saving does not depend on preferences. */ }
      if (addAnother) {
        setInterest('0'); setFee('0'); setSourceCleared(false); setDestinationCleared(false); setAmount(''); setDescription(''); setPayeeId(''); setUnresolvedPayee(false); setCategoryId(''); setIncomeSourceId(''); setDirty(false); setDiscard(false)
        focusNextAmount.current = true
      } else close()
      toast.success(translate("Transaction saved."))
    } catch (error) { setError(error instanceof Error ? error.message : typeof error === 'string' ? error : "Could not save changes. Please try again.") } finally { savingRef.current = false; setSaveMode(null) }
  }
  return <Dialog open onOpenChange={changeOpen}>
      <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[600px]" onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onOpenAutoFocus={event => { event.preventDefault(); amountRef.current?.focus() }} showCloseButton={!saving} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving || (event.target instanceof Element && event.target.matches('[role="combobox"][aria-expanded="true"]'))) event.preventDefault() }}>
        <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>{translate("Add transaction")}</DialogTitle><DialogDescription>{translate("Record a payment, purchase, transfer, or repayment.")}</DialogDescription></DialogHeader>
        {loading ? <p className="p-6" role="status">{translate("Loading accounts…")}</p>
          : loadError ? <div className="p-6" role="alert">{translate("Could not load accounts or spending options.")}{" "}<Button variant="outline" onClick={() => setAttempt(value => value + 1)}>{translate("Try again")}</Button></div>
          : !currency ? <p className="p-6">{translate("Choose your currency first in")}{" "}<Link to="/settings" className="text-brand" onClick={close}>{translate("Settings")}</Link>.</p>
          : !accounts.length ? <p className="p-6">{translate("Add an active account in")}{" "}<Link to="/accounts" className="text-brand" onClick={close}>{translate("Accounts")}</Link> {" "}{translate("first.")}</p>
          : <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
          <div className="min-h-0 overflow-y-auto px-6 py-5">
            <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div className="sm:col-span-2"><Field label={loanRepayment ? translate("Total payment ({value0})", { value0: currency ?? '' }) : translate("Amount ({value0})", { value0: currency ?? '' })}><Input ref={amountRef} className="mt-2 h-14 w-full text-2xl tabular-nums md:text-2xl" name="amount" value={amount} onChange={event => setAmount(event.target.value)} inputMode="decimal" required placeholder="0" /></Field></div>
              <Field label={translate("Transaction type")}><NativeSelect className={control} value={kind} onChange={event => { setKind(event.target.value as TransactionType); setIncomeSourceId(''); setUnresolvedPayee(false); setDestinationId(''); setDestinationCleared(false) }}>{Object.entries(labels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</NativeSelect></Field>
              <Field label={translate("Date")}><Input className={control} name="date" type="date" required min="0001-01-01" max={today()} defaultValue={today()} /></Field>
              <Field label={paired ? translate("From account") : translate("Account")}><AccountCombobox accounts={loanRepayment ? accounts.filter(a => ['cash', 'bank', 'wallet'].includes(a.type)) : accounts} disabled={saving} value={accountId} onChange={id => { setAccountId(id); setDirty(true); setSourceCleared(false); if (id === destinationId) { setDestinationId(''); setDestinationCleared(false) } }} /></Field>
              {paired && <Field label={translate("To account")}><AccountCombobox key={`${kind}-${accountId}`} accounts={accounts.filter(a => a.id !== accountId && (kind !== 'repayment' || ['credit_card', 'loan'].includes(a.type)))} disabled={saving} value={destinationId} onChange={id => { setDestinationId(id); setDirty(true); setDestinationCleared(false) }} /></Field>}
              {loanRepayment && <>
                {!loanReady && <p className="text-sm sm:col-span-2" role={loanLoadError ? 'alert' : 'status'}>{loanLoadError ? translate("Could not load loan details.") : translate("Loading loan details…")}{loanLoadError && <Button type="button" variant="outline" size="xs" onClick={() => setLoanAttempt(n => n + 1)}>{translate("Retry")}</Button>}</p>}
                {loanReady && contracts.length > 1 && <Field label={translate("Contract")}><NativeSelect className={control} required value={contractId} onChange={event => setContractId(event.target.value)}><option value="">{translate("Choose a contract")}</option>{contracts.map(contract => <option key={contract.id} value={contract.id} disabled={contract.needs_review}>{contract.name}</option>)}</NativeSelect></Field>}
                {loanReady && contracts.length === 1 && <p className="text-sm sm:col-span-2">{translate("Contract")}: {contracts[0].name}</p>}
                {contracts.find(c => c.id === contractId)?.needs_review && <p role="alert" className="text-sm sm:col-span-2">{translate("Review the borrowing before recording this repayment.")}</p>}
                <Field label={translate("Interest")}><Input className={control} inputMode="decimal" required value={interest} onChange={event => setInterest(event.target.value)} /></Field>
                <Field label={translate("Fees")}><Input className={control} inputMode="decimal" required value={fee} onChange={event => setFee(event.target.value)} /></Field>
                <Field label={translate("Principal")}><Input className={control} readOnly value={principal === null || !currency ? '' : formatAmount(principal.toString(), currency)} /></Field>
                <p className="text-xs sm:col-span-2">{translate("Principal is total payment minus interest and fees. Only principal reduces loan debt. Enter charges here only if they have not already been added to the loan balance.")}</p>
                {principal !== null && principal < 0n && <p className="text-sm sm:col-span-2" role="alert">{translate("Interest and fees cannot exceed the total payment.")}</p>}
              </>}
              {kind === 'transfer' && accounts.some(account => account.id === destinationId && account.type === 'loan') && <p className="text-sm sm:col-span-2">{translate("To separate principal, interest and fees, choose Repayment as the transaction type.")}</p>}
              {accounts.some(account => account.id === accountId && ['bank', 'wallet', 'credit_card'].includes(account.type)) && <label className="flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4 p-0" checked={sourceCleared} onChange={event => setSourceCleared(event.target.checked)} />{paired ? translate("Cleared in from account") : translate("Cleared")}</label>}
              {paired && accounts.some(account => account.id === destinationId && ['bank', 'wallet', 'credit_card'].includes(account.type)) && <label className="flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4 p-0" checked={destinationCleared} onChange={event => setDestinationCleared(event.target.checked)} />{translate("Cleared in to account")}</label>}
              {kind === 'income' && <div className="sm:col-span-2">
                <Field label={translate("Income source (optional)")}><NativeSelect className={control} value={incomeSourceId} disabled={incomeLoad !== 'ready'} onChange={event => setIncomeSourceId(event.target.value)}><option value="">{translate("Other income / Unassigned")}</option>{incomes.filter(i => i.is_active && accounts.some(a => a.id === i.destination_account_id)).map(i => <option key={i.id} value={i.id}>{i.name} · {i.type === 'salary' ? translate("Salary") : i.type}</option>)}</NativeSelect></Field>
                {incomeLoad === 'loading' ? <p className="mt-2 text-xs" role="status">{translate("Loading income sources…")}</p> : incomeLoad === 'error' ? <p className="mt-2 text-xs" role="alert">{translate("Could not load income sources.")}{" "}<Button type="button" variant="outline" size="xs" onClick={() => setIncomeAttempt(n => n + 1)}>{translate("Retry income sources")}</Button></p> : <p className="mt-2 text-xs">{translate("Choose your salary or another source created in Income. Enter the net amount actually received; payroll deductions are not subtracted again.")}{" "}<Link to="/income" className="text-brand" onClick={event => { if (dirty) { event.preventDefault(); setDiscard(true) } else close() }}>{translate("Manage income sources")}</Link></p>}
              </div>}
              {kind === 'expense' && <>
                <Field label={translate("Payee (optional)")}><PayeeCombobox value={payeeId} options={options.payees}
                  onChange={id => { setPayeeId(id); setDirty(true); setError(null) }}
                  onCreated={payee => setOptions(current => ({ ...current, payees: [...current.payees, payee] }))}
                  onBusyChange={busy => { creatingPayeeRef.current = busy; setCreatingPayee(busy) }}
                  onUnresolvedChange={setUnresolvedPayee} /></Field>
                <Field label={translate("Category (optional)")}><CategorySelect iconName={options.categories.find(item => item.id === categoryId)?.icon} className="w-full" value={categoryId} onChange={event => setCategoryId(event.target.value)}><option value="">{translate("Uncategorized")}</option>{options.categories.filter(item => !item.is_archived).map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</CategorySelect></Field>
                <p className="text-xs sm:col-span-2">{translate("Search or create a payee above. Manage payees and spending categories in Settings.")}</p>
              </>}
              <Field label={translate("Description (optional)")}><Input className={control} name="description" value={description} onChange={event => setDescription(event.target.value)} maxLength={200} placeholder={translate("e.g. Groceries")} /></Field>
            </fieldset>
            {!limits.loading && !limits.error && currency && previewAmount !== null && affectedGroups.map(group => <p key={group.id} className="mt-3 rounded-xl bg-soft p-3 text-sm">{group.name} {" "}{translate("· Estimated shared available credit after transaction:")}{" "}{formatAmount(sharedCreditAfter(limits.data, group.id, kind, accountId, destinationId, previewAmount!).toString(), currency)}</p>)}
            <p className="mt-4 text-xs">{translate("Enter a positive amount. Expenses on credit cards increase debt; repayments reduce it. Opening balances stay unchanged.")}</p>
            {error && <p className="mt-4 text-sm" role="alert">{translate(error)}</p>}
          </div>
          <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">{translate("Discard your unsaved transaction?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard changes")}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>{translate("Cancel")}</Button><Button type="submit" value="close" disabled={saving || (loanRepayment && !loanReady)}>{saveMode === 'close' ? translate("Saving…") : translate("Save transaction")}</Button><Button type="submit" value="another" variant="outline" disabled={saving || (loanRepayment && !loanReady)}>{saveMode === 'another' ? translate("Saving…") : translate("Save & add another")}</Button></DialogFooter>}</div>
        </form>}
      </DialogContent>
  </Dialog>
}
