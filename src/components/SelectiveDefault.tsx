import { useEffect, useState } from 'react'
import { CircleAlert } from 'lucide-react'
import { toast } from 'sonner'
import { t as translate, useLanguage } from '../lib/i18n'
import type { Account } from '../lib/desktop'
import { currentCycle, cycleLabel } from '../lib/cashflow'
import { exclusionFor, getSelectiveDefault, saveSelectiveDefault, type SelectiveDefault, type SelectiveDefaultStatus } from '../lib/selective-defaults'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Tooltip, TooltipProvider, TooltipTrigger, TooltipContent } from './ui/tooltip'

export function SelectiveDefaultWarning({ period }: { period?: SelectiveDefault }) {
  useLanguage()
  if (!period) return null
  return <TooltipProvider><Tooltip><TooltipTrigger asChild>
    <button type="button" className="inline-flex shrink-0 rounded p-0.5 text-red-700 focus-visible:outline-2 dark:text-red-400" aria-label={translate("Selective Default")}><CircleAlert className="size-4" aria-hidden="true" /></button>
  </TooltipTrigger><TooltipContent>
    {translate("Selective Default — planned repayments excluded from Outlook totals since {value0}.", { value0: period.start_month })}
    {period.end_month && <> {translate("Repayments resume in {value0}.", { value0: period.end_month })}</>}
    {' '}{translate("Recorded payments remain in Actual only during excluded cycles. Balances and saved plans are unchanged.")}
  </TooltipContent></Tooltip></TooltipProvider>
}

export function SelectiveDefaultSettings({ account }: { account: Account }) {
  useLanguage()
  const [status, setStatus] = useState<SelectiveDefaultStatus | null>(null)
  const [failed, setFailed] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [action, setAction] = useState<'exclude' | 'resume' | null>(null)
  useEffect(() => {
    let active = true
    setFailed(false)
    getSelectiveDefault(account.id).then(value => { if (active) setStatus(value) }).catch(() => { if (active) setFailed(true) })
    return () => { active = false }
  }, [account.id, attempt])
  useEffect(() => {
    const refresh = () => setAttempt(n => n + 1)
    window.addEventListener('accounts-changed', refresh)
    return () => window.removeEventListener('accounts-changed', refresh)
  }, [])
  const open = status?.periods.find(p => !p.end_month)
  const editable = !account.is_archived && !account.paid_off_on
  return <section className="space-y-3 rounded-2xl border border-line bg-card p-6">
    <div className="flex flex-wrap items-center justify-between gap-3"><h3 className="flex items-center gap-2 font-semibold">{translate("Selective Default")}<SelectiveDefaultWarning period={status ? exclusionFor(status.periods,account.id,currentCycle(status.period_start_day)) ?? open : undefined} /></h3>
      {status && !failed && editable && <Button variant="outline" onClick={() => setAction(open ? 'resume' : 'exclude')}>{open ? translate("Resume repayments") : translate("Exclude planned repayments")}</Button>}
    </div>
    <p className="text-sm">{translate("Exclude this account’s planned repayments from Outlook. Its balance, debt and recorded payments remain unchanged.")}</p>
    {failed ? <p role="alert" className="text-sm">{translate("Could not load Selective Default.")} <Button size="xs" variant="outline" onClick={() => setAttempt(n => n + 1)}>{translate("Retry")}</Button></p> : !status ? <p role="status">{translate("Loading…")}</p> : <>
      {status.periods.length > 0 && <ul className="space-y-1 text-sm">{status.periods.map(p => <li key={p.start_month}>{translate("Excluded from {value0}", { value0: p.start_month })}{p.end_month ? ` · ${translate("Repayments resume in {value0}.", { value0: p.end_month })}` : ` · ${translate("Until resumed")}`}</li>)}</ul>}
      {action && <SelectiveDefaultDialog account={account} status={status} action={action} close={() => setAction(null)} saved={() => { setAction(null); setAttempt(n => n + 1) }} />}
    </>}
  </section>
}

function SelectiveDefaultDialog({ account, status, action, close, saved }: { account: Account; status: SelectiveDefaultStatus; action: 'exclude' | 'resume'; close: () => void; saved: () => void }) {
  useLanguage()
  const [month, setMonth] = useState(() => {
    const current = currentCycle(status.period_start_day)
    const latest = status.periods.at(-1)
    const earliest = action === 'resume' ? latest?.start_month : latest?.end_month
    return earliest && earliest > current ? earliest : current
  })
  // Snapshot the reviewed history so concurrent edits fail instead of replacing it.
  const [expected] = useState(status.periods)
  const [confirmed, setConfirmed] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  let preview = ''
  try { preview = cycleLabel(month, status.period_start_day) } catch { /* Incomplete month. */ }
  const requestClose = () => { if (!busy) { if (dirty) setDiscard(true); else close() } }
  return <Dialog open onOpenChange={open => { if (!open) requestClose() }}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0" showCloseButton={!busy} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy) event.preventDefault() }}>
    <DialogHeader className="px-6 pt-6"><DialogTitle>{action === 'exclude' ? translate("Exclude planned repayments") : translate("Resume repayments")}</DialogTitle><DialogDescription>{account.name}</DialogDescription></DialogHeader>
    <form className="flex min-h-0 flex-col" onChange={() => setDirty(true)} onSubmit={async event => {
      event.preventDefault()
      if (busy || !confirmed) return
      setBusy(true); setError(null)
      try {
        await saveSelectiveDefault({ account_id: account.id, action, month, expected_periods: expected, confirmed })
        toast.success(translate("Selective Default updated.")); saved()
      } catch (e) { setError(String(e)) } finally { setBusy(false) }
    }}>
      <fieldset disabled={busy} className="min-h-0 space-y-4 overflow-y-auto p-6">
        <FormField label={action === 'exclude' ? translate("Start cycle") : translate("Resume cycle")}><Input type="month" required min="0001-01" max="9999-11" value={month} onChange={event => setMonth(event.target.value)} /></FormField>
        <p className="text-sm">{preview}</p>
        <p className="text-sm">{action === 'exclude' ? translate("Linked repayment schedules, payment plans and cycle overrides will be excluded from this cycle until resumed. Saved amounts remain intact.") : translate("Saved plans apply again from this cycle. Earlier excluded cycles remain excluded. Review dates and amounts before relying on the forecast.")}</p>
        <div><h4 className="text-sm font-semibold">{translate("Saved items linked to this account")}</h4><ul className="mt-2 space-y-1 text-sm">{status.linked_items.map((item, i) => <li key={i}>{translate(item.kind)} · {item.name}</li>)}</ul>{!status.linked_items.length && <p className="text-sm">{translate("No linked plans or overrides. Future linked plans will follow this status.")}</p>}</div>
        <p className="text-sm">{translate("Manual rows without account links and payroll deductions stay included. Review manual duplicates in Outlook and change payroll deductions in Income if needed.")}</p>
        <label className="flex items-start gap-2 text-sm"><Input type="checkbox" className="mt-0.5 size-4" checked={confirmed} onChange={event => setConfirmed(event.target.checked)} />{translate("I confirm this forecast change. Account balances and recorded payments will not change.")}</label>
        {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{translate(error)}</p>}
      </fieldset>
      <DialogFooter className="shrink-0 border-t border-line p-4">{discard ? <><p>{translate("Discard unsaved changes?")}</p><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard")}</Button></> : <><Button type="button" variant="outline" disabled={busy} onClick={requestClose}>{translate("Cancel")}</Button><Button type="submit" disabled={busy || !confirmed || !preview}>{busy ? translate("Saving…") : action === 'exclude' ? translate("Confirm exclusion") : translate("Confirm resumption")}</Button></>}</DialogFooter>
    </form>
  </DialogContent></Dialog>
}
