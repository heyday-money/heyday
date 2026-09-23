import { useEffect, useRef, useState, type FormEvent } from 'react'
import { Link } from '@tanstack/react-router'
import { toast } from 'sonner'
import { createTransaction, getSettings, listAccounts, listTransactionOptions, type TransactionOptions, type Account, type TransactionType } from '../lib/desktop'
import { decimalToInteger, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const labels = { income: 'Money received · In', expense: 'Expense · Out', transfer: 'Transfer', repayment: 'Repayment · Transfer' }
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
  const [accounts, setAccounts] = useState<Account[]>([])
  const [currency, setCurrency] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [kind, setKind] = useState<TransactionType>('expense')
  const [accountId, setAccountId] = useState('')
  const [destinationId, setDestinationId] = useState('')
  const [sourceCleared, setSourceCleared] = useState(false)
  const [destinationCleared, setDestinationCleared] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [saveMode, setSaveMode] = useState<'close' | 'another' | null>(null)
  const saving = saveMode !== null
  const savingRef = useRef(false)
  const focusNextAmount = useRef(false)
  const [amount, setAmount] = useState('')
  const [description, setDescription] = useState('')
  const [payeeId, setPayeeId] = useState('')
  const [categoryId, setCategoryId] = useState('')
  const [options, setOptions] = useState<TransactionOptions>({ payees: [], categories: [] })
  const [error, setError] = useState<string | null>(null)
  const amountRef = useRef<HTMLInputElement>(null)
  const returnFocus = useRef(document.activeElement as HTMLElement | null)
  useEffect(() => {
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(), listTransactionOptions()]).then(([settings, accounts, options]) => {
      if (active) { setCurrency(settings.currency); setAccounts(accounts); setOptions(options); setAccountId(accounts.some(account => account.id === initialAccountId) ? initialAccountId! : initialAccount(accounts)) }
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
  const paired = kind === 'transfer' || kind === 'repayment'
  const close = onClose
  function changeOpen(next: boolean) {
    if (next || saving) return
    if (dirty) setDiscard(true)
    else close()
  }
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!currency || savingRef.current) return
    const submitter = (event.nativeEvent as SubmitEvent).submitter
    const addAnother = submitter instanceof HTMLButtonElement && submitter.value === 'another'
    const data = new FormData(event.currentTarget)
    savingRef.current = true
    setSaveMode(addAnother ? 'another' : 'close'); setError(null)
    try {
      const amount = decimalToInteger(String(data.get('amount')), fractionDigits(currency))
      if (BigInt(amount) <= 0n) throw new Error('Amount must be greater than zero.')
      const clearedIds = [sourceCleared ? accountId : '', paired && destinationCleared ? destinationId : ''].filter(id => accounts.some(account => account.id === id && ['bank', 'wallet', 'credit_card'].includes(account.type)))
      await createTransaction({ cleared_account_ids: clearedIds, type: kind, account_id: accountId, destination_account_id: paired ? String(data.get('destination')) : null, amount, currency, date: String(data.get('date')), description: String(data.get('description') ?? '').trim(), payee_id: kind === 'expense' ? payeeId || null : null, category_id: kind === 'expense' ? categoryId || null : null })
      try { localStorage.setItem(rememberedAccountKey, accountId) } catch { /* Saving does not depend on preferences. */ }
      if (addAnother) {
        setSourceCleared(false); setDestinationCleared(false); setAmount(''); setDescription(''); setPayeeId(''); setCategoryId(''); setDirty(false); setDiscard(false)
        focusNextAmount.current = true
      } else close()
      toast.success('Transaction saved.')
    } catch (error) { setError(error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not save changes. Please try again.') } finally { savingRef.current = false; setSaveMode(null) }
  }
  return <Dialog open onOpenChange={changeOpen}>
      <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[600px]" onCloseAutoFocus={event => { event.preventDefault(); if (returnFocus.current?.isConnected) returnFocus.current.focus() }} onOpenAutoFocus={event => { event.preventDefault(); amountRef.current?.focus() }} showCloseButton={!saving} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
        <DialogHeader className="shrink-0 border-b border-line px-6 py-5"><DialogTitle>Add transaction</DialogTitle><DialogDescription>Record a payment, purchase, transfer, or repayment.</DialogDescription></DialogHeader>
        {loading ? <p className="p-6" role="status">Loading accounts…</p>
          : loadError ? <div className="p-6" role="alert">Could not load accounts or spending options. <Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Try again</Button></div>
          : !currency ? <p className="p-6">Choose your currency first in <Link to="/settings" className="text-brand" onClick={close}>Settings</Link>.</p>
          : !accounts.length ? <p className="p-6">Add an active account in <Link to="/accounts" className="text-brand" onClick={close}>Accounts</Link> first.</p>
          : <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col">
          <div className="min-h-0 overflow-y-auto px-6 py-5">
            <fieldset disabled={saving} className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div className="sm:col-span-2"><Field label={`Amount (${currency ?? ''})`}><Input ref={amountRef} className="mt-2 h-14 w-full text-2xl tabular-nums md:text-2xl" name="amount" value={amount} onChange={event => setAmount(event.target.value)} inputMode="decimal" required placeholder="0" /></Field></div>
              <Field label="Transaction type"><NativeSelect className={control} value={kind} onChange={event => { setKind(event.target.value as TransactionType); setDestinationId(''); setDestinationCleared(false) }}>{Object.entries(labels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</NativeSelect></Field>
              <Field label="Date"><Input className={control} name="date" type="date" required min="0001-01-01" max={today()} defaultValue={today()} /></Field>
              <Field label={paired ? 'From account' : 'Account'}><NativeSelect className={control} required value={accountId} onChange={event => { setAccountId(event.target.value); setSourceCleared(false); setDestinationId(''); setDestinationCleared(false) }}><option value="">Choose account</option>{accounts.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></Field>
              {paired && <Field label="To account"><NativeSelect key={`${kind}-${accountId}`} className={control} name="destination" required value={destinationId} onChange={event => { setDestinationId(event.target.value); setDestinationCleared(false) }}><option value="">Choose destination</option>{accounts.filter(a => a.id !== accountId && (kind !== 'repayment' || ['credit_card', 'loan'].includes(a.type))).map(a => <option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></Field>}
              {accounts.some(account => account.id === accountId && ['bank', 'wallet', 'credit_card'].includes(account.type)) && <label className="flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4 p-0" checked={sourceCleared} onChange={event => setSourceCleared(event.target.checked)} />{paired ? 'Cleared in from account' : 'Cleared'}</label>}
              {paired && accounts.some(account => account.id === destinationId && ['bank', 'wallet', 'credit_card'].includes(account.type)) && <label className="flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4 p-0" checked={destinationCleared} onChange={event => setDestinationCleared(event.target.checked)} />Cleared in to account</label>}
              {kind === 'expense' && <>
                <Field label="Payee (optional)"><NativeSelect className={control} value={payeeId} onChange={event => setPayeeId(event.target.value)}><option value="">No payee</option>{options.payees.filter(item => !item.is_archived).map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</NativeSelect></Field>
                <Field label="Category (optional)"><NativeSelect className={control} value={categoryId} onChange={event => setCategoryId(event.target.value)}><option value="">Uncategorized</option>{options.categories.filter(item => !item.is_archived).map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</NativeSelect></Field>
                <p className="text-xs sm:col-span-2">Add or manage payees and spending categories in Settings.</p>
              </>}
              <Field label="Description (optional)"><Input className={control} name="description" value={description} onChange={event => setDescription(event.target.value)} maxLength={200} placeholder="e.g. Groceries" /></Field>
            </fieldset>
            <p className="mt-4 text-xs">Enter a positive amount. Expenses on credit cards increase debt; repayments reduce it. Opening balances stay unchanged.</p>
            {error && <p className="mt-4 text-sm" role="alert">{error}</p>}
          </div>
          <div className="shrink-0 border-t border-line px-6 py-4">{discard ? <><p className="mb-3 text-sm" role="alert">Discard your unsaved transaction?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" value="close" disabled={saving}>{saveMode === 'close' ? 'Saving…' : 'Save transaction'}</Button><Button type="submit" value="another" variant="outline" disabled={saving}>{saveMode === 'another' ? 'Saving…' : 'Save & add another'}</Button></DialogFooter>}</div>
        </form>}
      </DialogContent>
  </Dialog>
}
