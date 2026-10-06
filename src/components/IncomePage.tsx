import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { DataTable } from './ui/data-table'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from './ui/dropdown-menu'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'
import { SalaryPaymentDialog, SalaryPaymentHistory } from './SalaryPaymentDialog'
import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { AccountLabel } from './InstitutionLogo'
import { AccountSelect } from './AccountSelect'
import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import { Link } from '@tanstack/react-router'
import { ArrowUpRight, Plus, Pencil, MoreHorizontal, ListMinus, History } from 'lucide-react'
import { toast } from 'sonner'
import { createIncome, updateIncome, desktopAvailable, getSettings, listAccounts, listIncomes, type Account, type Income, type IncomeType, type Settings } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Dialog, DialogTrigger, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField } from './FormField'
import { deductionInputs, initialDeductions, SalaryDeductionFields, SalaryDeductionsDialog } from './SalaryDeductions'

const incomeTypes = [
  { value: 'salary', get label() { return translate("Salary") } },
  { value: 'variable', get label() { return translate("Variable") } },
  { value: 'investment', get label() { return translate("Investment") } },
  { value: 'other', get label() { return translate("Other") } },
] as const
function amountText(amount: string, currency: string) {
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits), value = BigInt(amount)
  return `${value / scale}${digits ? `.${(value % scale).toString().padStart(digits, '0')}` : ''}`
}
const fieldStyle = 'mt-2 w-full bg-page'

export function IncomePage() {
  useLanguage()

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
  const [incomeType, setIncomeType] = useState<IncomeType>('salary')
  const [gross, setGross] = useState('')
  const [deductions, setDeductions] = useState(initialDeductions)
  const [editingDeductions, setEditingDeductions] = useState<Income | null>(null)
  const [editing, setEditing] = useState<Income | null>(null)
  const [recording, setRecording] = useState<Income | null>(null)
  const [history, setHistory] = useState<Income | null>(null)
  const saveLock = useRef(false)
  const editTriggerRef = useRef<HTMLButtonElement | null>(null)
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

  const close = () => { setOpen(false); setEditing(null); setDirty(false); setConfirmDiscard(false); setError(null) }
  const changeOpen = (next: boolean) => {
    if (saveLock.current) return
    if (next) { setEditing(null); setIncomeType('salary'); setGross(''); setDeductions(initialDeductions()); setDirty(false); setConfirmDiscard(false); setError(null); setOpen(true) }
    else if (dirty) setConfirmDiscard(true)
    else close()
  }

  const beginEdit = useCallback((source: Income, trigger: HTMLButtonElement) => {
    editTriggerRef.current = trigger
    if (saveLock.current || !currency) return
    setEditing(source); setIncomeType(source.type); setGross(amountText(source.estimated_amount, currency))
    setDirty(false); setConfirmDiscard(false); setError(null); setOpen(true)
  }, [currency])

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!currency || saveLock.current || confirmDiscard) return
    saveLock.current = true
    const data = new FormData(event.currentTarget)
    const text = (key: string) => String(data.get(key) ?? '').trim()
    setError(null); setSaving(true)
    try {
      const amount = decimalToInteger(text('estimated_amount'), fractionDigits(currency))
      if (BigInt(amount) < 0n) throw new Error("Estimated amount cannot be negative.")
      const rows = !editing && incomeType === 'salary' ? deductionInputs(deductions, currency) : []
      if (rows.reduce((sum, row) => sum + BigInt(row.amount), 0n) > BigInt(amount)) throw new Error("Total deductions cannot exceed gross salary.")
      if (editing && BigInt(amount) < BigInt(editing.deductions_total ?? '0')) throw new Error("Estimated gross salary cannot be lower than its existing deductions. Adjust deductions first.")
      const input = {
        name: text('name'), type: editing?.type ?? text('type') as IncomeType,
        destination_account_id: text('destination_account_id'), estimated_amount: amount,
        currency, recurrence_frequency: 'monthly' as const, recurrence_day_of_month: Number(text('day')),
        is_active: text('is_active') === 'true', is_auto_create_transaction: false,
      }
      const income = editing ? await updateIncome({ ...input, id: editing.id }) : await createIncome({ ...input, deductions: rows })
      setSources(current => editing ? current.map(source => source.id === income.id ? income : source) : [...current, income]); close()
      toast.success(editing ? translate("Income source updated.") : translate("Income source added."), { id: editing ? 'income-updated' : 'income-added' })
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : "Could not save income source. Please try again.")
    } finally { saveLock.current = false; setSaving(false) }
  }

  return <Dialog open={open} onOpenChange={changeOpen}>
    <div className="mb-6 flex items-start justify-between gap-4">
      <div><h2 className="text-2xl font-semibold">{translate("Know what’s coming in.")}</h2><p className="mt-2 text-[14px]">{translate("Plan your expected income and where it will go.")}</p></div>
      <DialogTrigger asChild><Button size="lg" disabled={!currency || !accounts.length || loading || loadError}><Plus size={17} />{translate("Add income source")}</Button></DialogTrigger>
    </div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage your income sources.")}</p>
      : loading ? <p role="status">{translate("Loading income sources…")}</p>
      : loadError ? <div role="alert"><p>{translate("Could not load income sources.")}</p><Button variant="link" onClick={() => setAttempt(value => value + 1)}>{translate("Try again")}</Button></div>
      : !currency ? <section className="rounded-2xl border border-line bg-card p-7"><h3 className="font-semibold">{translate("Choose your currency first")}</h3><p className="mt-2">{translate("Set the shared currency before entering estimated income.")}</p><Link to="/settings" className="mt-4 inline-block text-brand">{translate("Go to Settings →")}</Link></section>
      : <>
        {!accounts.length && <section className="mb-6 rounded-2xl border border-line bg-card p-7"><h3 className="font-semibold">{translate("Add a destination account first")}</h3><p className="mt-2">{translate("Every income source needs an account to receive it.")}</p><Link to="/accounts" className="mt-4 inline-block text-brand">{translate("Go to Accounts →")}</Link></section>}
        <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden border-line bg-card p-0 sm:max-w-[640px]" showCloseButton={!saving}
          onCloseAutoFocus={event => { if (editTriggerRef.current) { event.preventDefault(); editTriggerRef.current.focus(); editTriggerRef.current = null } }}
          onOpenAutoFocus={event => { event.preventDefault(); nameRef.current?.focus() }}
          onInteractOutside={event => event.preventDefault()}
          onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
          <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12">
            <DialogTitle>{editing ? translate("Edit income source") : translate("Add income source")}</DialogTitle>
            <DialogDescription>{editing ? translate("Changes affect generated forecasts, including past estimates. Explicit cycle overrides and recorded transactions remain intact. Saving never changes account balances.") : translate("Set up expected income. Saving a source won’t change your account balance.")}</DialogDescription>
          </DialogHeader>
          <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-1 flex-col" aria-label={editing ? translate("Edit income source") : translate("Add income source")} key={editing?.id ?? 'new'}>
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              <fieldset disabled={saving}>
                <div className="grid grid-cols-2 gap-5 max-[520px]:grid-cols-1">
                  <FormField label={translate("Income name")}><Input ref={nameRef} name="name" defaultValue={editing?.name ?? ''} required maxLength={100} className={fieldStyle} placeholder={translate("e.g. Monthly salary")} /></FormField>
                  <FormField label={translate("Income type")}><NativeSelect name="type" disabled={!!editing} value={incomeType} onChange={e => setIncomeType(e.target.value as IncomeType)} className={fieldStyle}>{incomeTypes.map(type => <option key={type.value} value={type.value}>{type.label}</option>)}</NativeSelect></FormField>
                  <FormField label={translate("Destination account")}><AccountSelect name="destination_account_id" required defaultValue={editing?.destination_account_id ?? ''} className={fieldStyle}><option value="" disabled>{translate("Choose an account")}</option>{editing && !accounts.some(account => account.id === editing.destination_account_id) && <option value={editing.destination_account_id}>{editing.destination_account_name} {" "}{translate("· Unavailable (inactive sources only)")}</option>}{accounts.map(account => <option key={account.id} value={account.id}>{account.name}</option>)}</AccountSelect></FormField>
                  <FormField label={translate("Estimated amount ({value0})", { value0: currency })}><Input name="estimated_amount" inputMode="decimal" value={gross} onChange={e => setGross(e.target.value)} required className={fieldStyle} placeholder="0" /></FormField>
                  <FormField label={translate("Frequency")}><NativeSelect name="frequency" defaultValue="monthly" className={fieldStyle}><option value="monthly">{translate("Monthly")}</option></NativeSelect></FormField>
                  <FormField label={translate("Day of month")}><NativeSelect name="day" defaultValue={String(editing?.recurrence_day_of_month ?? 1)} className={fieldStyle}>{Array.from({ length: 31 }, (_, index) => index + 1).map(day => <option key={day} value={day}>{translate("Day")}{" "}{day}</option>)}</NativeSelect></FormField>
                  <FormField label={translate("Status")}><NativeSelect name="is_active" defaultValue={String(editing?.is_active ?? true)} className={fieldStyle}><option value="true">{translate("Active")}</option><option value="false">{translate("Inactive")}</option></NativeSelect></FormField>
                </div>
                {!editing && incomeType === 'salary' && <><p className="mt-3 text-xs">{translate("Enter gross salary before deductions as the estimated amount.")}</p><SalaryDeductionFields accounts={accounts} rows={deductions} onChange={next => { setDeductions(next); setDirty(true) }} gross={gross} currency={currency} /></>}
                {editing && <p className="mt-3 text-xs">{translate("Income type is fixed. Inactive sources stay visible and stop generating estimates. Saved cycle overrides remain.")}{" "}{editing.type === 'salary' && translate("Existing deductions: {value0}. Use Manage Deductions to change them.", { value0: formatAmount(editing.deductions_total ?? '0', currency) })}</p>}
                <p className="mt-4 text-[12px]">{translate("When the scheduled day doesn’t exist, use the last day of that month. This schedule is independent of your payday cycle.")}</p>
                <div className="mt-5 rounded-xl border border-line p-4"><label className="flex items-center gap-2 text-[13px] text-muted"><input type="checkbox" disabled checked={false} readOnly className="size-4" />{translate("Automatically create transactions")}</label><p className="mt-2 text-[12px]">{translate("Coming later. Income sources currently describe estimates only.")}</p></div>
              </fieldset>
              {error && <p role="alert" className="mt-4 text-[14px]">{translate(error)}</p>}
            </div>
            {confirmDiscard ? <div className="shrink-0 border-t border-line px-6 py-4"><p role="alert" className="mb-3 text-sm">{translate("Discard your unsaved income source?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setConfirmDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard changes")}</Button></DialogFooter></div>
              : <DialogFooter className="shrink-0 border-t border-line px-6 py-4"><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>{translate("Cancel")}</Button><Button type="submit" disabled={saving}>{saving ? translate("Saving…") : translate("Save income source")}</Button></DialogFooter>}
          </form>
        </DialogContent>
        {!sources.length ? <section className="rounded-2xl border border-line bg-card p-12 text-center"><ArrowUpRight size={28} className="mx-auto mb-4 text-brand" /><h3 className="font-semibold">{translate("No income sources yet")}</h3><p className="mt-2 text-[14px]">{translate("Add salary, variable income, investments, or another source.")}</p></section>
          : <IncomeTable sources={sources} accounts={accounts} currency={currency} onEdit={beginEdit} onRecord={setRecording} onDeductions={setEditingDeductions} onHistory={setHistory} />}

      </>}
    {recording && currency && <SalaryPaymentDialog income={recording} currency={currency} onClose={() => setRecording(null)} />}
    {history && currency && <SalaryPaymentHistory incomeId={history.id} currency={currency} onClose={() => setHistory(null)} />}
    {editingDeductions && currency && <SalaryDeductionsDialog income={editingDeductions} currency={currency} onSaved={income => setSources(current => current.map(source => source.id === income.id ? income : source))} onClose={() => setEditingDeductions(null)} />}
  </Dialog>
}

function IncomeTable({ sources, accounts, currency, onEdit, onRecord, onDeductions, onHistory }: {
  sources: Income[]; accounts: Account[]; currency: string
  onEdit: (source: Income, trigger: HTMLButtonElement) => void
  onRecord: (source: Income) => void; onDeductions: (source: Income) => void; onHistory: (source: Income) => void
}) {
  useLanguage()
  const meta = { headerClassName: 'px-4 py-3 font-medium whitespace-nowrap', cellClassName: 'px-4 py-4 align-middle' }
  const moneyMeta = { headerClassName: `${meta.headerClassName} text-right`, cellClassName: `${meta.cellClassName} text-right whitespace-nowrap tabular-nums` }
  const columns = useMemo<ColumnDef<Income>[]>(() => [
    { id: 'source', header: translate('Income source'), meta: { ...meta, rowHeader: true, cellClassName: `${meta.cellClassName} min-w-44 max-w-64` }, cell: ({ row: { original: source } }) => <><h3 className="break-words font-semibold text-ink">{source.name}</h3><div className="mt-1 flex flex-wrap items-center gap-2 text-xs font-normal text-muted"><span>{incomeTypes.find(type => type.value === source.type)?.label}</span><span className={`rounded-full px-2 py-0.5 ${source.is_active ? 'bg-soft text-brand' : 'bg-page text-muted'}`}>{source.is_active ? translate('Active') : translate('Inactive')}</span></div></> },
    { id: 'gross', header: translate('Estimated gross'), meta: moneyMeta, cell: ({ row }) => formatAmount(row.original.estimated_amount, currency) },
    { id: 'deductions', header: translate('Deductions'), meta: moneyMeta, cell: ({ row }) => row.original.type === 'salary' ? formatAmount(row.original.deductions_total ?? '0', currency) : '—' },
    { id: 'net', header: translate('Estimated net'), meta: { ...moneyMeta, cellClassName: `${moneyMeta.cellClassName} font-semibold text-ink` }, cell: ({ row }) => formatAmount((BigInt(row.original.estimated_amount) - (row.original.type === 'salary' ? BigInt(row.original.deductions_total ?? '0') : 0n)).toString(), currency) },
    { id: 'destination', header: translate('Destination account'), meta: { ...meta, cellClassName: `${meta.cellClassName} min-w-36 max-w-56 break-words` }, cell: ({ row }) => <><AccountLabel id={row.original.destination_account_id} name={row.original.destination_account_name} />{!accounts.some(a => a.id === row.original.destination_account_id) && <span className="mt-1 block text-xs text-muted">{translate('Account unavailable')}</span>}</> },
    { id: 'schedule', header: translate('Schedule'), meta: { ...meta, cellClassName: `${meta.cellClassName} whitespace-nowrap` }, cell: ({ row }) => <>{translate('Monthly · Day')} {row.original.recurrence_day_of_month}{row.original.recurrence_day_of_month > 28 && <span className="mt-1 block text-xs text-muted">{translate(' (or month-end)')}</span>}</> },
    { id: 'actions', header: translate('Actions'), meta: { ...meta, headerClassName: `${meta.headerClassName} text-right` }, cell: ({ row: { original: source } }) => <div className="flex items-center justify-end gap-1">
      {source.type === 'salary' && <Button size="sm" variant="outline" className="mr-1" aria-label={translate('Record salary payment')} disabled={!source.is_active || !accounts.some(a => a.id === source.destination_account_id && ['cash', 'bank', 'wallet'].includes(a.type))} onClick={() => onRecord(source)}>{translate('Record payment')}</Button>}
      <Tooltip><TooltipTrigger asChild><Button size="icon-sm" variant="ghost" aria-label={translate('Edit {value0}', { value0: source.name })} onClick={event => onEdit(source, event.currentTarget)}><Pencil aria-hidden="true" /></Button></TooltipTrigger><TooltipContent>{translate('Edit income source')}</TooltipContent></Tooltip>
      {source.type === 'salary' && <DropdownMenu modal={false}><DropdownMenuTrigger asChild><Button size="icon-sm" variant="ghost" aria-label={translate('More actions for {value0}', { value0: source.name })}><MoreHorizontal aria-hidden="true" /></Button></DropdownMenuTrigger><DropdownMenuContent align="end"><DropdownMenuItem aria-label={translate('Manage deductions for {value0}', { value0: source.name })} onSelect={() => onDeductions(source)}><ListMinus aria-hidden="true" />{translate('Manage Deductions')}</DropdownMenuItem><DropdownMenuItem onSelect={() => onHistory(source)}><History aria-hidden="true" />{translate('Salary payment history')}</DropdownMenuItem></DropdownMenuContent></DropdownMenu>}
    </div> },
  ], [currency, accounts, onEdit, onRecord, onDeductions, onHistory, getLanguage()])
  const table = useReactTable({ data: sources, columns, getRowId: source => source.id, getCoreRowModel: getCoreRowModel() })
  return <TooltipProvider><DataTable table={table} label={translate('Income sources')} className="min-w-[1050px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" /><p className="mt-3 text-xs text-muted">{translate('Estimates per payment. Salary deductions are included in net income; record actual payments separately.')}</p></TooltipProvider>
}
