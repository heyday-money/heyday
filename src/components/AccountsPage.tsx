import { Tabs, TabsList, TabsTrigger, TabsContent } from './ui/tabs'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { Link } from '@tanstack/react-router'
import { Banknote, Building2, CreditCard, Landmark, Plus, TrendingUp } from 'lucide-react'
import { createAccount, loanTypes, type LoanType, desktopAvailable, getSettings, listAccounts, type Account, type AccountType, type Settings } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { toast } from 'sonner'
import { Dialog, DialogTrigger, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField as Field } from './FormField'
import { Textarea } from './ui/textarea'
import { NativeSelect } from './ui/native-select'

const types = [
  { value: 'cash', label: 'Cash', groupLabel: 'Cash', icon: Banknote },
  { value: 'bank', label: 'Bank', groupLabel: 'Bank', icon: Building2 },
  { value: 'credit_card', label: 'Credit card', groupLabel: 'Credit cards', icon: CreditCard },
  { value: 'loan', label: 'Loan', groupLabel: 'Loans', icon: Landmark },
  { value: 'investment', label: 'Investment', groupLabel: 'Investments', icon: TrendingUp },
] as const
const inputStyle = 'mt-2 w-full rounded-[10px] border border-line bg-page px-3 py-2.5 text-[14px] text-ink focus-visible:outline-2 focus-visible:outline-brand'
function DayField({ name, label }: { name: string; label: string }) {
  return <Field label={label}><NativeSelect name={name} className={inputStyle} defaultValue="">
    <option value="">Not set</option>
    {Array.from({ length: 31 }, (_, i) => i + 1).map(day => <option key={day} value={day}>{day}</option>)}
  </NativeSelect></Field>
}

export function AccountsPage() {
  const [accounts, setAccounts] = useState<Account[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [open, setOpen] = useState(false)
  const [type, setType] = useState<AccountType>('cash')
  const [selectedType, setSelectedType] = useState<AccountType | 'all'>('all')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [dirty, setDirty] = useState(false)
  const [confirmDiscard, setConfirmDiscard] = useState(false)
  const nameRef = useRef<HTMLInputElement>(null)
  const addRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts()]).then(([settings, accounts]) => {
      if (active) { setSettings(settings); setAccounts(accounts) }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  useEffect(() => {
    const refresh = () => setAttempt(value => value + 1)
    window.addEventListener('accounts-changed', refresh)
    return () => window.removeEventListener('accounts-changed', refresh)
  }, [])
  const debt = type === 'credit_card' || type === 'loan'
  const currency = settings?.currency
  const close = () => { setOpen(false); setError(null); setDirty(false); setConfirmDiscard(false) }
  const changeOpen = (next: boolean) => {
    if (saving) return
    if (next) { setType(selectedType === 'all' ? 'cash' : selectedType); setError(null); setDirty(false); setConfirmDiscard(false); setOpen(true) }
    else if (dirty) setConfirmDiscard(true)
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
      if (type === 'loan' && !loanTypes.some(item => item.value === loanType)) throw new Error('Choose a loan type.')
      const account = await createAccount({
        loan_type: loanType as LoanType | null,
        name: text('name'), type, currency,
        opening_balance: decimalToInteger(text('balance'), digits),
        institution: type === 'cash' ? null : optional('institution'),
        last_four: ['bank', 'credit_card'].includes(type) ? optional('last_four') : null,
        notes: optional('notes'),
        credit_limit: type === 'credit_card' && text('credit_limit') ? decimalToInteger(text('credit_limit'), digits) : null,
        statement_day: type === 'credit_card' ? day('statement_day') : null,
        payment_due_day: debt ? day('payment_due_day') : null,
        interest_rate_bps: debt && text('interest') ? Number(decimalToInteger(text('interest'), 2)) : null,
      })
      setAccounts(current => [...current.filter(item => item.id !== account.id), account]);
      if (selectedType !== 'all') setSelectedType(account.type)
      close(); toast.success('Account added.', { id: 'account-added' })
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not add account. Please try again.')
    } finally { setSaving(false) }
  }

  return <Dialog open={open} onOpenChange={changeOpen}>
    <div className="mb-6 flex items-start justify-between gap-4">
      <div><h2 className="text-[25px] font-[650]">A place for every account.</h2><p className="mt-2 text-[14px]">Your cash, savings, investments, and debts.</p></div>
      <DialogTrigger asChild><Button ref={addRef} type="button" disabled={!currency || loading || loadError} size="lg"><Plus size={17} />Add account</Button></DialogTrigger>
    </div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your local accounts.</p>
      : loading ? <p role="status">Loading accounts…</p>
      : loadError ? <div role="alert"><p>Could not load accounts.</p><button className="mt-3 text-brand" onClick={() => setAttempt(value => value + 1)}>Try again</button></div>
      : !currency ? <div className="rounded-[22px] border border-line bg-card p-7"><h3 className="font-semibold">Choose your currency first</h3><p className="mt-2">Set the currency for all your accounts before adding a balance.</p><Link to="/settings" className="mt-4 inline-block text-brand">Go to Settings →</Link></div>
      : <>
        <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[640px]" showCloseButton={!saving}
          onOpenAutoFocus={event => { event.preventDefault(); nameRef.current?.focus() }}
          onInteractOutside={event => event.preventDefault()}
          onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
          <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12">
            <DialogTitle>Add account</DialogTitle>
            <DialogDescription>Choose an account type and enter your starting balance.</DialogDescription>
          </DialogHeader>
          <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col" aria-label="Add account">
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              <fieldset disabled={saving}>
                <div className="grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                  <Field label="Account type"><NativeSelect value={type} onChange={event => { setType(event.target.value as AccountType); setError(null) }} className={inputStyle}>{types.map(item => <option value={item.value} key={item.value}>{item.label}</option>)}</NativeSelect></Field>
                  {type === 'loan' && <Field label="Loan type"><NativeSelect name="loan_type" required defaultValue="" className={inputStyle}><option value="" disabled>Choose a loan type</option>{loanTypes.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}</NativeSelect></Field>}
                  <Field label="Account name"><Input ref={nameRef} name="name" required maxLength={100} className={inputStyle} placeholder="e.g. Everyday wallet" /></Field>
                  <Field label={`${debt ? 'Amount owed' : type === 'investment' ? 'Current value' : 'Opening balance'} (${currency})`}><Input name="balance" inputMode="decimal" required defaultValue="0" className={inputStyle} /></Field>
                </div>
                <p className="mt-4 text-[12px]">{debt ? 'Enter debt as a positive amount. Use a negative amount for an overpayment or credit.' : 'Enter the balance you own. Use a negative amount for an overdraft.'} This is a manual starting balance.</p>
                <details className="mt-5 rounded-xl border border-line p-4">
                  <summary className="cursor-pointer text-[13px] font-semibold">Optional details</summary>
                  <div className="mt-4 grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                    {type !== 'cash' && <Field label="Institution (optional)"><Input name="institution" maxLength={100} className={inputStyle} placeholder={type === 'investment' ? 'Broker or fund provider' : 'Bank or lender'} /></Field>}
                    {['bank', 'credit_card'].includes(type) && <Field label="Last four digits (optional)"><Input name="last_four" inputMode="numeric" pattern="[0-9]{4}" maxLength={4} className={inputStyle} /></Field>}
                    {type === 'credit_card' && <><Field label={`Credit limit (${currency}, optional)`}><Input name="credit_limit" inputMode="decimal" className={inputStyle} /></Field><DayField name="statement_day" label="Statement day (optional)" /></>}
                    {debt && <><DayField name="payment_due_day" label="Payment due day (optional)" /><Field label="Annual interest rate (%, optional)"><Input name="interest" type="number" min="0" max="100" step="0.01" className={inputStyle} /></Field></>}
                  </div>
                  {debt && <p className="mt-3 text-[12px]">Scheduled days use month-end if unavailable. Payments and interest are not calculated automatically.</p>}
                  <div className="mt-4"><Field label="Notes (optional)"><Textarea name="notes" maxLength={1000} rows={2} className={inputStyle} /></Field></div>
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
        <Tabs value={selectedType} onValueChange={value => setSelectedType(value as AccountType | 'all')}>
          <div className="min-w-0 overflow-x-auto p-1">
            <TabsList aria-label="Account types" className="w-max min-w-full">
              <TabsTrigger value="all" className="min-w-max shrink-0 whitespace-nowrap">All ({accounts.length})</TabsTrigger>
              {types.map(item => <TabsTrigger key={item.value} value={item.value} className="min-w-max shrink-0 whitespace-nowrap">{item.groupLabel} ({accounts.filter(account => account.type === item.value).length})</TabsTrigger>)}
            </TabsList>
          </div>
          {(['all', ...types.map(item => item.value)] as const).map(tab => {
            const groups = types.filter(item => tab === 'all' ? accounts.some(account => account.type === item.value) : item.value === tab)
            const count = accounts.filter(account => tab === 'all' || account.type === tab).length
            return <TabsContent key={tab} value={tab}>
              {!count ? <div className="rounded-[22px] border border-line bg-card p-12 text-center"><h3 className="font-semibold">{tab === 'all' ? 'No accounts yet' : `No ${types.find(item => item.value === tab)!.label.toLowerCase()} accounts yet`}</h3><p className="mt-2 text-[14px]">Use Add account to {tab === 'all' ? 'start building your overview' : 'add one to this group'}.</p></div>
                : <div className="space-y-6">{groups.map(group => <section key={group.value} aria-label={`${group.groupLabel} accounts`}>
                  <h3 className="mb-3 text-sm font-semibold">{group.groupLabel} ({accounts.filter(account => account.type === group.value).length})</h3>
                  <ul className="grid grid-cols-2 gap-4 max-[900px]:grid-cols-1" aria-label={`${group.groupLabel} accounts`}>{accounts.filter(account => account.type === group.value).map(account => {
            const definition = types.find(item => item.value === account.type)!
            const Icon = definition.icon
            const liability = ['credit_card', 'loan'].includes(account.type)
            return <li key={account.id} className="min-w-0 rounded-[22px] border border-line bg-card p-6">
              <div className="flex items-center gap-3"><Icon className="shrink-0 text-brand" size={22} /><h4 className="break-words font-semibold">{account.name}</h4></div>
              <p className="mt-3 text-[12px]">{account.type === 'loan' ? loanTypes.find(item => item.value === account.loan_type)?.label ?? 'Loan (unclassified)' : definition.label} · {liability ? 'Liability' : 'Asset'}{account.last_four ? ` · •••• ${account.last_four}` : ''}</p>
              {account.institution && <p className="mt-1 break-words text-[13px]">{account.institution}</p>}
              <p className="mt-4 break-words text-[23px] font-semibold text-ink">{formatAmount(account.current_balance ?? account.opening_balance, currency)}</p>
              <p className="mt-1 text-[12px]">{liability ? 'Current amount owed' : 'Current balance'}</p>
              {account.credit_limit !== null && <p className="mt-3 text-[13px]">Credit limit: {formatAmount(account.credit_limit, currency)}</p>}
              {account.statement_day !== null && <p className="mt-2 text-[13px]">Statement day: {account.statement_day}</p>}
              {account.payment_due_day !== null && <p className="mt-2 text-[13px]">Payment due day: {account.payment_due_day}</p>}
              {account.interest_rate_bps !== null && <p className="mt-2 text-[13px]">Annual interest rate: {(account.interest_rate_bps / 100).toFixed(2)}%</p>}
              {account.notes && <p className="mt-3 break-words whitespace-pre-wrap text-[13px]">{account.notes}</p>}
            </li>
                  })}</ul>
                </section>)}</div>}
            </TabsContent>
          })}
        </Tabs>
      </>}
  </Dialog>
}
