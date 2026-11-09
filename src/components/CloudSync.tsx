import { useEffect, useState } from 'react'
import { ChevronDown, Cloud, LockKeyhole } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
import { useCloudSync } from './CloudSyncProvider'
import { CloudSyncStatus } from './CloudSyncStatus'
import { CategoryIcon, categoryIcons } from './CategoryIcon'
import { t, useLanguage } from '../lib/i18n'
import { listAccounts, type EncryptionStatus, type SyncConflict, type SyncDetails } from '../lib/desktop'
import { getPlanner } from '../lib/cashflow'
import { MetadataDetails, metadataTitle } from './SyncMetadataDetails'
import { formatAmount } from '../lib/money'

export function CloudSync({ disabled, online, email, keyState, onPrepare }: { disabled: boolean; online: boolean; email: string; keyState: EncryptionStatus['state'] | null; onPrepare: () => void }) {
  useLanguage()
  const { status, busy, error, act, resolve } = useCloudSync()
  const [enable, setEnable] = useState(false)
  const [confirmed, setConfirmed] = useState(false)
  const [conflict, setConflict] = useState<SyncConflict | null>(null)
  const [name, setName] = useState('')
  const blocked = disabled || busy
  const [accountNames, setAccountNames] = useState<Record<string, string>>({})
  useEffect(() => { if (conflict?.entity && conflict.entity !== 'category') void listAccounts(true, true).then(rows => setAccountNames(Object.fromEntries(rows.map(row => [row.id, row.name])))).catch(() => setAccountNames({})) }, [conflict])
  const [sourceNames, setSourceNames] = useState<Record<string, string>>({})
  useEffect(() => {
    let active = true
    setSourceNames({})
    if (conflict?.entity === 'income-plan') void getPlanner().then(data => {
      if (active) setSourceNames(Object.fromEntries([
        ...data.incomes.map(s => [`income:${s.id}`, s.name]),
        ...data.income_deductions.map(d => [`deduction:${d.id}`, `${data.incomes.find(s => s.id === d.income_id)?.name ?? t('Income source')} · ${d.name}`]),
      ]))
    }).catch(() => {})
    return () => { active = false }
  }, [conflict])
  const plannerConflict = conflict?.entity === 'income-plan'
  const categoryConflict = !conflict?.entity || conflict.entity === 'category' 
  async function choose(choice: 'local' | 'remote' | 'rename') {
    if (conflict && await resolve(conflict, choice, name)) { setConflict(null); toast.success(t('Sync conflict resolved.')) }
  }
  return <section aria-labelledby="cloud-sync-heading" className="space-y-4 border-t border-line pt-5">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <h4 id="cloud-sync-heading" className="flex items-center gap-2 font-semibold"><Cloud className="size-5 text-primary" aria-hidden="true" />{t('Cloud sync')}</h4>
      <span className="flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-2.5 py-1 text-xs text-emerald-800 dark:text-emerald-300"><LockKeyhole className="size-3.5" aria-hidden="true" />{t('End-to-end encrypted')}</span>
    </div>
    <CloudSyncStatus disabled={disabled} connected={online} keyState={keyState} onPrepare={onPrepare} onEnable={() => {
      if (status?.bound && status.fullEnabled && status.plannerEnabled && status.incomeEnabled && status.financialEnabled) void act('enable')
      else { setConfirmed(false); setEnable(true) }
    }} />
    {error && !enable && !conflict && <p role="alert" className="rounded-xl border border-rose-500/20 bg-rose-500/5 px-4 py-3 text-sm text-rose-700 dark:text-rose-300">{error === 'Set up or unlock encryption before syncing.' ? t('Restore your encryption key on this device to continue syncing.') : t(error)}</p>}
    {status?.bound && (!status.financialEnabled || !status.incomeEnabled || !status.plannerEnabled || !status.fullEnabled) && <div className="space-y-3 rounded-xl border border-line bg-soft p-4">
      {status.bound && !status.financialEnabled && <div className="space-y-2 text-sm"><p>{t('This connection currently syncs categories only. Confirm before adding accounts and transactions.')}</p><Button variant="outline" disabled={blocked} onClick={() => { setConfirmed(false); setEnable(true) }}>{t('Add accounts and transactions')}</Button></div>}
      {status.bound && status.financialEnabled && !status.incomeEnabled && <div className="space-y-2 text-sm"><p>{t('Confirm before adding income sources, deductions, and complete payslips to this connection.')}</p><Button variant="outline" disabled={blocked} onClick={() => { setConfirmed(false); setEnable(true) }}>{t('Add income and payroll')}</Button></div>}
      {status.bound && status.incomeEnabled && !status.plannerEnabled && <div className="space-y-2 text-sm"><p>{t('Confirm before adding Outlook income and deduction overrides to this connection.')}</p><Button variant="outline" disabled={blocked} onClick={() => { setConfirmed(false); setEnable(true) }}>{t('Add income forecasts')}</Button></div>}
      {status.bound && status.plannerEnabled && !status.fullEnabled && <div className="space-y-2 text-sm"><p>{t('Confirm before adding loans, card bills, schedules, remaining forecasts, reconciliation, payday settings, and custom logos.')}</p><Button variant="outline" disabled={blocked} onClick={() => { setConfirmed(false); setEnable(true) }}>{t('Add remaining financial data')}</Button></div>}
    </div>}
    {status && status.conflictCount > 0 && <div className="space-y-3">
      <p className="font-medium">{t('{count} sync conflicts need your choice', { count: status.conflictCount })}</p>
      <ul aria-label={t('Sync conflicts')} className="space-y-2">{status.conflicts.map(item => <li key={item.recordId} className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line p-3">
        <span className="min-w-0 break-words text-sm">{item.entity === 'metadata' && <>{metadataTitle(item.localDetails && 'fields' in item.localDetails ? item.localDetails.kind : item.remoteDetails && 'fields' in item.remoteDetails ? item.remoteDetails.kind : '')} · </>}{item.local?.name ?? item.remote?.name ?? t('Deleted record')}</span>
        <Button variant="outline" disabled={blocked} onClick={() => { setConflict(item); setName(item.remote?.name ?? '') }}>{t('Review conflict')}</Button>
      </li>)}</ul>
      {status.conflictCount > status.conflicts.length && <p className="text-xs text-muted">{t('More conflicts will appear as you resolve these.')}</p>}
    </div>}
    <details className="group rounded-xl border border-line px-4">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 py-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">{t('What syncs')}<ChevronDown aria-hidden="true" className="size-4 text-muted transition-transform group-open:rotate-180" /></summary>
      <div className="space-y-3 border-t border-line pb-4 pt-3 text-sm">
        <p className="font-medium text-ink">{t('Synced across your devices')}</p>
        <ul className="list-disc space-y-2 pl-5 text-muted marker:text-primary">
          <li>{t('Accounts, transactions, and categories')}</li>
          <li>{t('Income sources, deductions, and payslips')}</li>
          <li>{t('Loans, card bills, and payment schedules')}</li>
          <li>{t('Forecasts and reconciliation history')}</li>
          <li>{t('Payday settings and custom logos')}</li>
        </ul>
        <ul className="list-disc space-y-2 border-t border-line pl-5 pt-3 text-muted">
          <li>{t('Balances follow your opening balances and transactions.')}</li>
          <li>{t('Your financial data is encrypted before upload. Active sync access is required; expired access pauses syncing and retains your cloud data.')}</li>
          <li>{t('Appearance preferences, login credentials, encryption keys, and local backup files stay on this device.')}</li>
        </ul>
      </div>
    </details>
    <Dialog open={enable} onOpenChange={open => { if (!blocked) setEnable(open) }}>
      <DialogContent showCloseButton={!blocked} onPointerDownOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (blocked) event.preventDefault() }}>
        <DialogHeader><DialogTitle>{t('Enable encrypted financial sync?')}</DialogTitle><DialogDescription>{t('Your complete financial records, planning data, reconciliation history, payday settings, and custom logos will be merged with the encrypted data belonging to {email}. Different edits require your choice.', { email })}</DialogDescription></DialogHeader>
        <p className="text-sm">{t('Your financial data is always encrypted before upload. Keep your recovery key safe; support cannot recover it.')}</p>
        <label className="flex items-start gap-3 text-sm"><input type="checkbox" checked={confirmed} disabled={blocked} onChange={event => setConfirmed(event.target.checked)} className="mt-1" /><span>{t('I want to sync all financial data, planning settings, and custom logos from this device with this account.')}</span></label>
        {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
        <DialogFooter><Button variant="outline" disabled={blocked} onClick={() => setEnable(false)}>{t('Cancel')}</Button><Button disabled={blocked || !confirmed} onClick={() => void act('enable').then(ok => { if (ok) setEnable(false) })}>{t('Enable cloud sync')}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog open={!!conflict} onOpenChange={open => { if (!open && !blocked) setConflict(null) }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl" showCloseButton={!blocked} onPointerDownOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (blocked) event.preventDefault() }}>
        <DialogHeader><DialogTitle>{t('Resolve sync conflict')}</DialogTitle><DialogDescription>{plannerConflict ? t('Choose the monthly forecast to keep. Clearing an override restores its scheduled estimate. This does not change balances or recorded payments.') : t('Choose the version to keep. Your choice will be shared on the next sync. Deleting a payslip reverses its entire payment. Deleting transactions reverses their balance effects and may mark reconciliation history for review.')}</DialogDescription></DialogHeader>
        {conflict && <>
          <div className="grid gap-3 sm:grid-cols-2">{[['local', conflict.local], ['remote', conflict.remote]].map(([side, raw]) => {
            const value = raw as SyncConflict['local']
            return <div key={side as string} className="rounded-xl border border-line bg-soft p-3"><p className="text-xs text-muted">{side === 'local' ? t('On this device') : t('In the cloud')}</p><p className="mt-1 flex items-center gap-2 font-medium">{value && categoryConflict && <span role="img" aria-label={t('Icon: {value0}', { value0: categoryIcons.find(icon => icon.key === value.icon)?.label ?? value.icon })}><CategoryIcon name={value.icon} /></span>}<span className="min-w-0 break-words">{value?.name ?? (plannerConflict ? t('Use scheduled estimate') : t('Deleted record'))}</span></p>{value && !plannerConflict && <p className="text-xs text-muted">{value.isArchived ? t('Archived') : t('Active')}</p>}{!categoryConflict && <FinancialDetails value={side === 'local' ? conflict.localDetails : conflict.remoteDetails} accounts={accountNames} sources={sourceNames} />}</div>
          })}</div>
          {(conflict.localDetails && 'entries' in conflict.localDetails && conflict.localDetails.payroll || conflict.remoteDetails && 'entries' in conflict.remoteDetails && conflict.remoteDetails.payroll) && <p className="text-sm">{t('For two payments of the same salary month, reverse the unwanted payment in Income before using the cloud version. Keep local deletion only if you want to remove the cloud payment and sync your own.')}</p>}
          {plannerConflict && <p className="text-sm">{t('If the income source or deduction was removed, resolve its definition conflict before keeping an override.')}</p>}
          {categoryConflict && conflict.remote && <label className="space-y-2 text-sm">{t('Optional new name for the cloud category')}<input value={name} maxLength={100} disabled={blocked} onChange={event => setName(event.target.value)} className="w-full rounded-lg border border-line bg-card px-3 py-2" /></label>}
          {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
          <div className="flex flex-wrap gap-2"><Button variant="outline" disabled={blocked} onClick={() => void choose('local')}>{conflict.local ? t('Keep this device’s version') : plannerConflict ? t('Keep scheduled estimate') : t('Keep local deletion')}</Button><Button variant="outline" disabled={blocked} onClick={() => void choose('remote')}>{conflict.remote ? t('Use cloud version') : plannerConflict ? t('Use scheduled estimate') : t('Use cloud deletion')}</Button>{categoryConflict && conflict.remote && <Button disabled={blocked || !name.trim() || name.trim() === conflict.remote.name} onClick={() => void choose('rename')}>{t('Save cloud category with new name')}</Button>}</div>
        </>}
        <DialogFooter><Button variant="outline" disabled={blocked} onClick={() => setConflict(null)}>{t('Cancel')}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
}

function FinancialDetails({ value, accounts, sources }: { value: SyncDetails | null | undefined; accounts: Record<string, string>; sources: Record<string, string> }) {
  if (!value || 'icon' in value) return null
  if ('fields' in value) return <MetadataDetails value={value} accounts={accounts} />
  if ('source_type' in value) return <div className="mt-3 space-y-2 text-sm"><p>{value.source_type === 'income' ? t('Income source') : t('Salary deduction')}: {sources[`${value.source_type}:${value.source_id}`] ?? t('Source unavailable')}</p><p>{t('Forecast cycle')}: {value.month}</p><p>{t('Entered forecast')}: {formatAmount(value.amount, value.currency)}</p></div>
  if ('entries' in value) return <div className="mt-3 space-y-3 text-sm">{value.protected && !value.payroll && !value.links && <p>{t('Linked payment: manage deletion on its original device.')}</p>}{value.payroll && <><p>{t('Salary month')}: {value.payroll.occurrence} · {value.payroll.date}</p><p>{t('Actual gross salary')}: {formatAmount(value.payroll.gross, value.currency)}</p><p>{t('Net salary deposited')}: {formatAmount(value.payroll.net, value.currency)}</p>{value.payroll.breakdown.map(d => <div key={d.id} className="border-t border-line pt-2"><p>{d.name} · {formatAmount(d.amount, value.currency)}</p>{d.debt_account_id && <p>{d.debt_account_name} · {t('Principal')}: {formatAmount(d.principal, value.currency)} · {t('Interest')}: {formatAmount(d.interest, value.currency)} · {t('Fees')}: {formatAmount(d.fee, value.currency)}</p>}</div>)}</>}{value.entries.map(entry => <div key={entry.id} className="space-y-1 border-t border-line pt-2"><p>{entry.date} · {t(({ income: 'Income', expense: 'Expense', transfer: 'Transfer', repayment: 'Repayment' } as const)[entry.type])}</p><p className="break-words">{accounts[entry.account_id] ?? t('Account unavailable')}{entry.destination_account_id && <> → {accounts[entry.destination_account_id] ?? t('Account unavailable')}</>}</p><p className="font-medium tabular-nums">{formatAmount(entry.amount, value.currency)}</p>{entry.description && <p className="break-words">{entry.description}</p>}{entry.payee && <p>{t('Payee')}: {entry.payee.name}</p>}{entry.income_source && <p>{t('Income source')}: {entry.income_source.name}</p>}</div>)}</div>
  if ('deductions' in value) return <div className="mt-3 space-y-2 text-sm"><p>{t('Destination account')}: {accounts[value.destination_account_id] ?? t('Account unavailable')}</p><p>{t('Estimated gross')}: {value.recurrence_frequency === 'irregular' ? t('Not estimated') : formatAmount(value.estimated_amount, value.currency)}</p><p>{t('Schedule')}: {value.recurrence_frequency === 'irregular' ? t('When received') : `${t('Monthly · Day')} ${value.recurrence_day_of_month}`}</p>{value.deductions.map(d => <div key={d.id} className="border-t border-line pt-2"><p>{d.name} · {formatAmount(d.amount, value.currency)}</p>{d.description && <p className="break-words">{d.description}</p>}{d.debt_account_id && <p>{accounts[d.debt_account_id] ?? t('Account unavailable')}</p>}</div>)}</div>
  const types = { cash: 'Cash', bank: 'Bank', wallet: 'Digital wallet', credit_card: 'Credit card', loan: 'Loan', investment: 'Investment' }
  const rows: [string, string | number | null | undefined][] = [
    [t('Type'), t(types[value.type])], [t('Opening balance'), formatAmount(value.opening_balance, value.currency)],
    [t('Institution'), value.institution], [t('Last four digits'), value.last_four], [t('Notes'), value.notes],
  ]
  if (value.type === 'credit_card') rows.push(
    [t('Credit limit'), value.credit_limit == null ? null : formatAmount(value.credit_limit, value.currency)],
    [t('Statement day'), value.statement_day],
  )
  if (value.type === 'credit_card' || value.type === 'loan') rows.push(
    [t('Payment due day'), value.payment_due_day],
    [t('Annual interest'), value.interest_rate_ten_thousandths == null ? null : `${value.interest_rate_ten_thousandths / 10000}%`],
  )
  if (value.type === 'loan') rows.push(
    [t('Loan type'), value.loan_type ? t(({ mortgage: 'Mortgage', auto_loan: 'Auto Loan', student_loan: 'Student Loan', personal_loan: 'Personal Loan', medical_debt: 'Medical Debt', other_debt: 'Other Debt' } as Record<string, string>)[value.loan_type] ?? value.loan_type) : null],
    [t('Monthly installment'), value.monthly_installment == null ? null : formatAmount(value.monthly_installment, value.currency)],
    [t('Initial Loan Amount'), value.initial_loan_amount == null ? null : formatAmount(value.initial_loan_amount, value.currency)],
  )
  return <dl className="mt-3 space-y-2 text-sm">{rows.map(([label, detail]) => <div key={label}><dt className="text-xs text-muted">{label}</dt><dd className="break-words">{detail ?? '—'}</dd></div>)}</dl>
}
