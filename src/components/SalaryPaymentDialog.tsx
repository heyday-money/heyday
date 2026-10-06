import { useEffect, useRef, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { t, useLanguage } from '../lib/i18n'
import { listIncomeDeductions, type Income, type IncomeDeduction } from '../lib/desktop'
import { getLoanAccount, type LoanContract } from '../lib/loans'
import { listAccounts } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { localToday } from '../lib/card-billing'
import { recordSalaryPayment, listSalaryPayments, deleteSalaryPayment, type PayrollBreakdown, type SalaryPayment } from '../lib/salary-payments'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
function editable(value: string, currency: string) {
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits), n = BigInt(value)
  return `${n / scale}${digits ? '.' + (n % scale).toString().padStart(digits, '0') : ''}`
}
interface Row { deduction: IncomeDeduction; amount: string; interest: string; fee: string; contract: string; contracts: LoanContract[]; loan: boolean }
export function SalaryPaymentDialog({ income, currency, onClose }: { income: Income; currency: string; onClose: () => void }) {
  useLanguage()
  const [rows, setRows] = useState<Row[] | null>(null), [gross, setGross] = useState(editable(income.estimated_amount, currency))
  const [date, setDate] = useState(localToday()), [month, setMonth] = useState(localToday().slice(0, 7))
  const [confirmed, setConfirmed] = useState(false), [dirty, setDirty] = useState(false), [discard, setDiscard] = useState(false)
  const [saving, setSaving] = useState(false), [error, setError] = useState(''), [attempt, setAttempt] = useState(0)
  const lock = useRef(false)
  useEffect(() => {
    let active = true
    setError('')
    Promise.all([listIncomeDeductions(income.id), listAccounts()]).then(async ([deductions, accounts]) => {
      const next = await Promise.all(deductions.map(async deduction => {
        const loan = accounts.find(a => a.id === deduction.debt_account_id)?.type === 'loan'
        const contracts = loan ? (await getLoanAccount(deduction.debt_account_id!)).contracts : []
        return { deduction, amount: editable(deduction.amount, currency), interest: '0', fee: '0', contract: contracts.length === 1 ? contracts[0].id : '', contracts, loan }
      }))
      if (active) setRows(next)
    }).catch(e => { if (active) setError(String(e)) })
    return () => { active = false }
  }, [income.id, currency, attempt])
  const minor = (value: string) => { const result = decimalToInteger(value, fractionDigits(currency)); if (BigInt(result) < 0n) throw new Error('Amounts cannot be negative.'); return result }
  let net: string | null = null
  try { if (rows) net = (BigInt(minor(gross)) - rows.reduce((sum, r) => sum + BigInt(minor(r.amount)), 0n)).toString() } catch { /* Incomplete draft. */ }
  function close() { if (lock.current) return; if (dirty) setDiscard(true); else onClose() }
  function update(index: number, key: 'amount' | 'interest' | 'fee' | 'contract', value: string) { setRows(current => current!.map((r, i) => i === index ? { ...r, [key]: value } : r)); setConfirmed(false) }
  async function save(event: FormEvent) {
    event.preventDefault(); if (lock.current || !rows || !confirmed || discard) return
    lock.current = true; setSaving(true); setError('')
    try {
      await recordSalaryPayment({ income_id: income.id, destination_account_id: income.destination_account_id, occurrence: month, date, gross: minor(gross), currency, confirmed,
        deductions: rows.map(r => ({ id: r.deduction.id, debt_account_id: r.deduction.debt_account_id ?? null, amount: minor(r.amount), interest: minor(r.interest), fee: minor(r.fee), contract_id: r.contract || null })) })
      toast.success(t('Salary payment recorded.')); onClose()
    } catch (e) { setError(e instanceof Error ? e.message : String(e)) } finally { lock.current = false; setSaving(false) }
  }
  return <Dialog open onOpenChange={open => { if (!open) close() }}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[680px]" showCloseButton={!saving} onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (saving) e.preventDefault() }}>
    <DialogHeader className="px-6 pt-6"><DialogTitle>{t('Record salary payment')} · {income.name}</DialogTitle><DialogDescription>{t('Confirm your payslip amounts. Only net pay enters the destination account; loan principal reduces debt without another bank withdrawal. Interest and fees remain in the payroll breakdown.')}</DialogDescription></DialogHeader>
    <form onSubmit={save} onChange={() => { setDirty(true) }} className="flex min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-4"><fieldset disabled={saving} className="min-w-0 space-y-4">
        <p className="text-sm">{t('Destination account')}: {income.destination_account_name}</p>
        <div className="grid grid-cols-2 gap-3"><FormField label={t('Salary month')}><Input type="month" required value={month} max={localToday().slice(0, 7)} onChange={e => { setMonth(e.target.value); setConfirmed(false) }} /></FormField><FormField label={t('Payment date')}><Input type="date" required value={date} max={localToday()} onChange={e => { setDate(e.target.value); setConfirmed(false) }} /></FormField></div>
        <FormField label={t('Actual gross salary')}><Input inputMode="decimal" required value={gross} onChange={e => { setGross(e.target.value); setConfirmed(false) }} /></FormField>
        {rows?.map((r, i) => {
          let principal: string | null = null
          try { principal = (BigInt(minor(r.amount)) - BigInt(minor(r.interest)) - BigInt(minor(r.fee))).toString() } catch { /* Incomplete draft. */ }
          return <section key={r.deduction.id} className="space-y-3 rounded-xl border border-line p-3"><h3 className="font-medium">{r.deduction.name}</h3>{r.deduction.debt_account_id && <p className="text-sm">{r.deduction.debt_account_name}</p>}
            <FormField label={t('Deduction amount')}><Input aria-label={`${r.deduction.name} · ${t('Deduction amount')}`} inputMode="decimal" value={r.amount} onChange={e => update(i, 'amount', e.target.value)} /></FormField>
            {r.loan && <><div className="grid grid-cols-2 gap-3"><FormField label={t('Interest')}><Input aria-label={`${r.deduction.name} · ${t('Interest')}`} inputMode="decimal" value={r.interest} onChange={e => update(i, 'interest', e.target.value)} /></FormField><FormField label={t('Fees')}><Input aria-label={`${r.deduction.name} · ${t('Fees')}`} inputMode="decimal" value={r.fee} onChange={e => update(i, 'fee', e.target.value)} /></FormField></div><p>{t('Principal')}: {principal === null ? '—' : formatAmount(principal, currency)}</p>
            {r.contracts.length > 0 && <FormField label={t('Loan contract')}><NativeSelect value={r.contract} required onChange={e => update(i, 'contract', e.target.value)}><option value="">{t('Choose a contract')}</option>{r.contracts.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}</NativeSelect></FormField>}</>}
          </section>
        })}
        {!rows && !error && <p role="status">{t('Loading deductions…')}</p>}
        <p className="font-semibold">{t('Net salary deposited')}: {net === null ? '—' : formatAmount(net, currency)}</p>
        <p className="text-xs">{t('If this salary was already entered as ordinary income, reverse that receipt first. Unassigned receipts cannot be matched automatically.')}</p>
        <label className="flex items-start gap-2 text-sm"><Input className="size-4 shrink-0" type="checkbox" checked={confirmed} onChange={e => setConfirmed(e.target.checked)} />{t('I confirm these actual payslip amounts and linked repayments.')}</label>
        {error && <p role="alert">{t(error)}{!rows && <Button type="button" variant="outline" onClick={() => setAttempt(n => n + 1)}>{t('Retry deductions')}</Button>}</p>}
      </fieldset></div>
      <div className="shrink-0 border-t border-line bg-card p-4">{discard ? <><p>{t('Discard unsaved changes?')}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{t('Keep editing')}</Button><Button type="button" variant="destructive" onClick={onClose}>{t('Discard changes')}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={close}>{t('Cancel')}</Button><Button type="submit" disabled={!rows || !confirmed || saving || net === null || BigInt(net) < 0n}>{saving ? t('Saving…') : t('Record salary payment')}</Button></DialogFooter>}</div>
    </form>
  </DialogContent></Dialog>
}
export function SalaryPaymentHistory({ incomeId = null, paymentId = null, accountId = null, currency, onClose }: { incomeId?: string | null; paymentId?: string | null; accountId?: string | null; currency: string; onClose: () => void }) {
  useLanguage()
  const [payments, setPayments] = useState<SalaryPayment[] | null>(null), [error, setError] = useState(''), [attempt, setAttempt] = useState(0)
  const [reversing, setReversing] = useState<string | null>(null), [busy, setBusy] = useState(false)
  const lock = useRef(false)
  useEffect(() => { let active = true; listSalaryPayments(incomeId, paymentId, accountId).then(p => { if (active) setPayments(p) }).catch(e => { if (active) setError(String(e)) }); return () => { active = false } }, [incomeId, paymentId, accountId, attempt])
  async function reverse() {
    if (!reversing || lock.current) return
    lock.current = true; setBusy(true); setError('')
    try { await deleteSalaryPayment(reversing); setReversing(null); setAttempt(n => n + 1); toast.success(t('Salary payment reversed.')) } catch (e) { setError(String(e)) } finally { lock.current = false; setBusy(false) }
  }
  return <Dialog open onOpenChange={open => { if (!open && !lock.current) onClose() }}><DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-[700px]" showCloseButton={!busy} onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }}><DialogHeader><DialogTitle>{t('Salary payment history')}</DialogTitle><DialogDescription>{t('Saved payslip breakdowns do not change when salary definitions are edited.')}</DialogDescription></DialogHeader>
    {!payments && !error && <p role="status">{t('Loading…')}</p>}{payments?.length === 0 && <p>{t('No salary payments recorded.')}</p>}
    {payments?.map(p => <section key={p.id} className="space-y-2 rounded-xl border border-line p-4"><h3 className="font-semibold">{p.occurrence} · {p.date}</h3><p>{t('Actual gross salary')}: {formatAmount(p.gross, currency)}</p><p>{t('Net salary deposited')}: {formatAmount(p.net, currency)}</p>
      {(JSON.parse(p.breakdown) as PayrollBreakdown[]).map(d => <div key={d.id} className="border-t border-line pt-2"><p>{d.name} · {formatAmount(d.amount, currency)}</p>{d.debt_account_id && <p className="text-sm">{d.debt_account_name} · {t('Principal')}: {formatAmount(d.principal, currency)} · {t('Interest')}: {formatAmount(d.interest, currency)} · {t('Fees')}: {formatAmount(d.fee, currency)}</p>}</div>)}
      <Button variant="outline" disabled={busy} onClick={() => setReversing(p.id)}>{t('Reverse salary payment')}</Button></section>)}
    {reversing && <div role="alert" className="space-y-3 border-t border-line pt-3"><p>{t('Reverse the entire salary payment, including the net deposit and every linked principal repayment? Reconciled entries will be reversed and affected reconciliation history marked for review. Record again to correct the payslip.')}</p><Button variant="outline" disabled={busy} onClick={() => setReversing(null)}>{t('Cancel')}</Button><Button variant="destructive" disabled={busy} onClick={reverse}>{t('Confirm reversal')}</Button></div>}
    {error && <p role="alert">{t(error)}<Button variant="outline" onClick={() => { setError(''); setAttempt(n => n + 1) }}>{t('Try again')}</Button></p>}
  </DialogContent></Dialog>
}
