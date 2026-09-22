import { NativeSelect } from './ui/native-select'
import { useEffect, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { listAccounts, listIncomeDeductions, saveSalaryDeductions, type Account, type Income, type IncomeDeduction, type IncomeDeductionInput } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

export interface DeductionDraft { key: string; id: string | null; name: string; description: string; amount: string; debt_account_id?: string | null; debt_account_name?: string | null }
export function initialDeductions(): DeductionDraft[] {
  return ['Withholding Tax', 'Social Security', 'Provident Fund', 'Payroll Loan Payments', 'Other Deductions'].map(name => ({ key: crypto.randomUUID(), id: null, name, description: '', amount: '' }))
}
export function deductionInputs(rows: DeductionDraft[], currency: string): IncomeDeductionInput[] {
  return rows.filter(row => row.id !== null || row.amount.trim() !== '' || !!row.debt_account_id).map(row => {
    if (!row.name.trim()) throw new Error('Enter a name for each deduction.')
    const amount = decimalToInteger(row.amount, fractionDigits(currency))
    if (BigInt(amount) < 0n) throw new Error('Deductions cannot be negative.')
    return { id: row.id, name: row.name.trim(), description: row.description.trim(), amount, debt_account_id: row.debt_account_id || null }
  })
}
function amountText(value: string, currency: string) {
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits), amount = BigInt(value)
  return `${amount / scale}${digits ? '.' + (amount % scale).toString().padStart(digits, '0') : ''}`
}
export function SalaryDeductionFields({ rows, onChange, gross, currency, accounts }: { accounts: Account[]; rows: DeductionDraft[]; onChange: (rows: DeductionDraft[]) => void; gross: string; currency: string }) {
  let preview: { total: bigint; net: bigint } | null = null
  try {
    const total = deductionInputs(rows, currency).reduce((sum, row) => sum + BigInt(row.amount), 0n)
    preview = { total, net: BigInt(decimalToInteger(gross, fractionDigits(currency))) - total }
  } catch { /* Incomplete drafts are validated on submit. */ }
  function update(key: string, field: 'name' | 'description' | 'amount' | 'debt_account_id', value: string) { onChange(rows.map(row => row.key === key ? { ...row, [field]: value } : row)) }
  return <section className="mt-5 space-y-3 rounded-xl border border-line p-4" aria-label="Salary deductions">
    <div className="flex items-center justify-between gap-2"><h3 className="font-semibold">Income Deductions</h3><Button type="button" variant="outline" size="sm" disabled={rows.length >= 100} onClick={() => onChange([...rows, { key: crypto.randomUUID(), id: null, name: '', description: '', amount: '' }])}>Add deduction</Button></div>
    <p className="text-xs">Enter the amount withheld from each monthly salary payment. Leave unused new rows blank; explicit zero is saved. Enter amounts yourself—no rates are assumed. Payroll loans entered here must not be repeated under Debt Payments.</p>
    {rows.map((row, index) => <div key={row.key} className="space-y-2 border-t border-line pt-3">
      <div className="grid grid-cols-[minmax(0,1fr)_minmax(90px,0.6fr)_auto] items-end gap-2">
        <Field label={`Deduction ${index + 1} name`}><Input maxLength={100} value={row.name} onChange={e => update(row.key, 'name', e.target.value)} /></Field>
        <Field label={`Deduction ${index + 1} amount per month (${currency})`}><Input inputMode="decimal" placeholder="0.00" value={row.amount} onChange={e => update(row.key, 'amount', e.target.value)} /></Field>
        <Button type="button" variant="ghost" size="sm" aria-label={`Remove deduction ${index + 1}`} onClick={() => onChange(rows.filter(item => item.key !== row.key))}>Remove</Button>
      </div>
      <Field label={`Deduction ${index + 1} debt account (optional)`}><NativeSelect value={row.debt_account_id ?? ''} onChange={e => update(row.key, 'debt_account_id', e.target.value)}>
        <option value="">No linked debt account</option>
        {row.debt_account_id && !accounts.some(account => account.id === row.debt_account_id) && <option value={row.debt_account_id}>{row.debt_account_name ?? 'Linked debt'} (unavailable)</option>}
        {accounts.filter(account => account.type === 'loan' || account.type === 'credit_card').map(account => <option key={account.id} value={account.id}>{account.name}</option>)}
      </NativeSelect></Field>
      {row.debt_account_id && <p className="text-xs text-muted">Enter the monthly payroll repayment above. The linked account identifies the debt; its balance does not set this amount. Outlook Debt Payments is for additional payments outside payroll. This plan does not change the actual debt balance.</p>}
      <details><summary className="cursor-pointer text-xs text-muted">Description</summary><Input aria-label={`Deduction ${index + 1} description`} className="mt-2" maxLength={2000} value={row.description} onChange={e => update(row.key, 'description', e.target.value)} /></details>
    </div>)}
    {preview && <div className="border-t border-line pt-3 text-sm"><p>Total Deductions: {formatAmount(preview.total.toString(), currency)}</p><p className={preview.net < 0n ? 'text-red-700 dark:text-red-400' : 'font-semibold text-ink'}>Estimated Net Salary: {formatAmount(preview.net.toString(), currency)}</p></div>}
  </section>
}
export function SalaryDeductionsDialog({ income, currency, onSaved, onClose }: { income: Income; currency: string; onSaved: (income: Income) => void; onClose: () => void }) {
  const [rows, setRows] = useState<DeductionDraft[] | null>(null)
  const [accounts, setAccounts] = useState<Account[]>([])
  const [original, setOriginal] = useState<IncomeDeduction[]>([])
  const [error, setError] = useState<string | null>(null), [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false), [discard, setDiscard] = useState(false), [attempt, setAttempt] = useState(0)
  useEffect(() => {
    let active = true
    setError(null)
    Promise.all([listIncomeDeductions(income.id), listAccounts()]).then(([items, accounts]) => {
      if (!active) return
      setAccounts(accounts)
      setOriginal(items)
      setRows(items.length ? items.map(item => ({ ...item, key: item.id, amount: amountText(item.amount, currency) })) : initialDeductions())
    }).catch(e => { if (active) setError(typeof e === 'string' ? e : 'Could not load deductions.') })
    return () => { active = false }
  }, [income.id, currency, attempt])
  function close() { if (saving) return; if (dirty) setDiscard(true); else onClose() }
  async function submit(event: FormEvent) {
    event.preventDefault(); if (!rows || saving) return
    setError(null); setSaving(true)
    try {
      const deductions = deductionInputs(rows, currency)
      if (deductions.reduce((sum, row) => sum + BigInt(row.amount), 0n) > BigInt(income.estimated_amount)) throw new Error('Total deductions cannot exceed gross salary.')
      const saved = await saveSalaryDeductions({ income_id: income.id, currency, deductions })
      onSaved(saved); onClose(); toast.success('Salary deductions saved.')
    } catch (e) { setError(typeof e === 'string' ? e : e instanceof Error ? e.message : 'Could not save deductions.') } finally { setSaving(false) }
  }
  const removed = original.filter(item => !rows?.some(row => row.id === item.id))
  return <Dialog open onOpenChange={open => { if (!open) close() }}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[660px]" showCloseButton={!saving} onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (saving) e.preventDefault() }}>
    <DialogHeader className="px-6 pt-6"><DialogTitle>Salary Deductions · {income.name}</DialogTitle><DialogDescription>Gross salary: {formatAmount(income.estimated_amount, currency)}. Deductions follow this salary’s recurrence and appear separately in Outlook. Account balances stay unchanged.</DialogDescription></DialogHeader>
    <form className="flex min-h-0 flex-col" onSubmit={submit}><fieldset disabled={saving} className="min-h-0 overflow-y-auto px-6 pb-4">
      {rows ? <SalaryDeductionFields accounts={accounts} rows={rows} onChange={next => { setRows(next); setDirty(true) }} gross={amountText(income.estimated_amount, currency)} currency={currency} /> : !error && <p role="status">Loading deductions…</p>}
      {rows && removed.length > 0 && <p className="mt-3 text-xs">Saving removes {removed.map(row => row.name).join(', ')} and their Outlook cycle overrides. Other deductions and overrides are preserved.</p>}
      {rows && <p className="mt-3 text-xs">Changes update generated deductions in every cycle, including past cycles. Saved Outlook overrides remain. Review any old manual Outlook deductions to avoid counting them twice.</p>}
      {error && <p className="mt-3 text-sm" role="alert">{error}{!rows && <Button type="button" variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry deductions</Button>}</p>}
    </fieldset><div className="border-t border-line px-6 py-4">{discard ? <><p role="alert" className="mb-3">Discard unsaved deduction changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={onClose}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={close}>Cancel</Button><Button type="submit" disabled={!rows || saving}>{saving ? 'Saving…' : 'Save deductions'}</Button></DialogFooter>}</div></form>
  </DialogContent></Dialog>
}
