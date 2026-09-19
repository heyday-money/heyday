import { useEffect, useRef, useState, type FormEvent } from 'react'
import { Link } from '@tanstack/react-router'
import { ArrowUpRight, Plus } from 'lucide-react'
import { toast } from 'sonner'
import { createIncome, desktopAvailable, getSettings, listAccounts, listIncomes, type Account, type Income, type IncomeType, type Settings } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Dialog, DialogTrigger, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField } from './FormField'

const incomeTypes = [
  { value: 'salary', label: 'Salary' },
  { value: 'variable', label: 'Variable' },
  { value: 'investment', label: 'Investment' },
  { value: 'other', label: 'Other' },
] as const
const fieldStyle = 'mt-2 w-full bg-page'

export function IncomePage() {
  const [sources, setSources] = useState<Income[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [open, setOpen] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [confirmDiscard, setConfirmDiscard] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const nameRef = useRef<HTMLInputElement>(null)
  const currency = settings?.currency

  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(), listIncomes()]).then(([settings, accounts, incomes]) => {
      if (active) { setSettings(settings); setAccounts(accounts); setSources(incomes) }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])

  const close = () => { setOpen(false); setDirty(false); setConfirmDiscard(false); setError(null) }
  const changeOpen = (next: boolean) => {
    if (saving) return
    if (next) { setDirty(false); setConfirmDiscard(false); setError(null); setOpen(true) }
    else if (dirty) setConfirmDiscard(true)
    else close()
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!currency || saving) return
    const data = new FormData(event.currentTarget)
    const text = (key: string) => String(data.get(key) ?? '').trim()
    setError(null); setSaving(true)
    try {
      const amount = decimalToInteger(text('estimated_amount'), fractionDigits(currency))
      if (BigInt(amount) < 0n) throw new Error('Estimated amount cannot be negative.')
      const income = await createIncome({
        name: text('name'), type: text('type') as IncomeType,
        destination_account_id: text('destination_account_id'), estimated_amount: amount,
        currency, recurrence_frequency: 'monthly', recurrence_day_of_month: Number(text('day')),
        is_active: text('is_active') === 'true', is_auto_create_transaction: false,
      })
      setSources(current => [...current, income]); close()
      toast.success('Income source added.', { id: 'income-added' })
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not add income source. Please try again.')
    } finally { setSaving(false) }
  }

  return <Dialog open={open} onOpenChange={changeOpen}>
    <div className="mb-6 flex items-start justify-between gap-4">
      <div><h2 className="text-[25px] font-[650]">Know what’s coming in.</h2><p className="mt-2 text-[14px]">Plan your expected income and where it will go.</p></div>
      <DialogTrigger asChild><Button size="lg" disabled={!currency || !accounts.length || loading || loadError}><Plus size={17} />Add income source</Button></DialogTrigger>
    </div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your income sources.</p>
      : loading ? <p role="status">Loading income sources…</p>
      : loadError ? <div role="alert"><p>Could not load income sources.</p><Button variant="link" onClick={() => setAttempt(value => value + 1)}>Try again</Button></div>
      : !currency ? <section className="rounded-[22px] border border-line bg-card p-7"><h3 className="font-semibold">Choose your currency first</h3><p className="mt-2">Set the shared currency before entering estimated income.</p><Link to="/settings" className="mt-4 inline-block text-brand">Go to Settings →</Link></section>
      : <>
        {!accounts.length && <section className="mb-6 rounded-[22px] border border-line bg-card p-7"><h3 className="font-semibold">Add a destination account first</h3><p className="mt-2">Every income source needs an account to receive it.</p><Link to="/accounts" className="mt-4 inline-block text-brand">Go to Accounts →</Link></section>}
        <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[640px]" showCloseButton={!saving}
          onOpenAutoFocus={event => { event.preventDefault(); nameRef.current?.focus() }}
          onInteractOutside={event => event.preventDefault()}
          onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
          <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12">
            <DialogTitle>Add income source</DialogTitle>
            <DialogDescription>Set up expected income. Saving a source won’t change your account balance.</DialogDescription>
          </DialogHeader>
          <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col" aria-label="Add income source">
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              <fieldset disabled={saving}>
                <div className="grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                  <FormField label="Income name"><Input ref={nameRef} name="name" required maxLength={100} className={fieldStyle} placeholder="e.g. Monthly salary" /></FormField>
                  <FormField label="Income type"><NativeSelect name="type" defaultValue="salary" className={fieldStyle}>{incomeTypes.map(type => <option key={type.value} value={type.value}>{type.label}</option>)}</NativeSelect></FormField>
                  <FormField label="Destination account"><NativeSelect name="destination_account_id" required defaultValue="" className={fieldStyle}><option value="" disabled>Choose an account</option>{accounts.map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</NativeSelect></FormField>
                  <FormField label={`Estimated amount (${currency})`}><Input name="estimated_amount" inputMode="decimal" required className={fieldStyle} placeholder="0" /></FormField>
                  <FormField label="Frequency"><NativeSelect name="frequency" defaultValue="monthly" className={fieldStyle}><option value="monthly">Monthly</option></NativeSelect></FormField>
                  <FormField label="Day of month"><NativeSelect name="day" defaultValue="1" className={fieldStyle}>{Array.from({ length: 31 }, (_, index) => index + 1).map(day => <option key={day} value={day}>Day {day}</option>)}</NativeSelect></FormField>
                  <FormField label="Status"><NativeSelect name="is_active" defaultValue="true" className={fieldStyle}><option value="true">Active</option><option value="false">Inactive</option></NativeSelect></FormField>
                </div>
                <p className="mt-4 text-[12px]">When the scheduled day doesn’t exist, use the last day of that month. This schedule is independent of your payday cycle.</p>
                <div className="mt-5 rounded-xl border border-line p-4"><label className="flex items-center gap-2 text-[13px] text-muted"><input type="checkbox" disabled checked={false} readOnly className="size-4" />Automatically create transactions</label><p className="mt-2 text-[12px]">Coming later. Income sources currently describe estimates only.</p></div>
              </fieldset>
              {error && <p role="alert" className="mt-4 text-[14px]">{error}</p>}
            </div>
            {confirmDiscard ? <div className="shrink-0 border-t border-line px-6 py-4"><p role="alert" className="mb-3 text-sm">Discard your unsaved income source?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setConfirmDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter></div>
              : <DialogFooter className="shrink-0 border-t border-line px-6 py-4"><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save income source'}</Button></DialogFooter>}
          </form>
        </DialogContent>
        {!sources.length ? <section className="rounded-[22px] border border-line bg-card p-12 text-center"><ArrowUpRight size={28} className="mx-auto mb-4 text-brand" /><h3 className="font-semibold">No income sources yet</h3><p className="mt-2 text-[14px]">Add salary, variable income, investments, or another source.</p></section>
          : <ul className="space-y-3" aria-label="Income sources">{sources.map(source => <li key={source.id} className="rounded-[18px] border border-line bg-card p-5">
            <div className="flex items-start justify-between gap-4 max-[600px]:flex-col">
              <div className="min-w-0"><div className="flex flex-wrap items-center gap-2"><h3 className="break-words font-semibold">{source.name}</h3><span className={`rounded-full px-2 py-0.5 text-[11px] ${source.is_active ? 'bg-soft text-brand' : 'bg-page text-muted'}`}>{source.is_active ? 'Active' : 'Inactive'}</span></div><p className="mt-2 break-words text-[13px]">{incomeTypes.find(type => type.value === source.type)?.label} · To {source.destination_account_name}</p><p className="mt-1 text-[12px]">Monthly · Day {source.recurrence_day_of_month}{source.recurrence_day_of_month > 28 ? ' (or month-end)' : ''}</p></div>
              <div className="min-w-0 text-right max-[600px]:text-left"><p className="break-words text-[20px] font-semibold text-ink">{formatAmount(source.estimated_amount, currency)}</p><p className="text-[12px]">Estimated per payment</p></div>
            </div>
          </li>)}</ul>}
      </>}
  </Dialog>
}
