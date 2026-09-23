import { AccountFormDialog } from './AccountFormDialog'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link, useParams, useSearch } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { Check, Circle, LockKeyhole } from 'lucide-react'
import { toast } from 'sonner'
import { desktopAvailable, deleteTransaction, listAccounts, type Account } from '../lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { getReconciliation, finishReconciliation, setVerification, type ReconciliationSnapshot, type VerificationEntry, type VerificationStatus } from '../lib/reconciliation'
import { AddTransactionDialog } from './AddTransactionDialog'
import { DataTable } from './ui/data-table'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const labels = { uncleared: 'Uncleared', cleared: 'Cleared', reconciled: 'Reconciled' }
function message(error: unknown) { return error instanceof Error ? error.message : String(error) }
export function AccountTransactionsPage() {
  const { accountId } = useParams({ from: '/accounts/$accountId' })
  const { reconcile } = useSearch({ from: '/accounts/$accountId' })
  return <AccountTransactions key={accountId} accountId={accountId} startReconcile={reconcile ?? false} />
}

function AccountTransactions({ accountId, startReconcile }: { accountId: string; startReconcile: boolean }) {
  const [data, setData] = useState<ReconciliationSnapshot | null>(null)
  const [refreshing, setRefreshing] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const savingRef = useRef(false)
  const request = useRef(0)
  const [statusFilter, setStatusFilter] = useState<VerificationStatus | ''>('')
  const [hideReconciled, setHideReconciled] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [reconciling, setReconciling] = useState(startReconcile)
  const [posted, setPosted] = useState('')
  const [cardBalanceKind, setCardBalanceKind] = useState<'owed' | 'credit'>('owed')
  const [openingConfirmed, setOpeningConfirmed] = useState(false)
  const [adding, setAdding] = useState(false)
  const [editing, setEditing] = useState<Account | null>(null)
  const [loadingEditor, setLoadingEditor] = useState(false)
  const [correction, setCorrection] = useState<{ kind: 'unlock' | 'delete'; entry: VerificationEntry } | null>(null)
  const [correctionConfirmed, setCorrectionConfirmed] = useState(false)
  const [forceConfirmation, setForceConfirmation] = useState(false)

  const refresh = useCallback(async () => {
    const current = ++request.current
    setRefreshing(true)
    try {
      const result = await getReconciliation(accountId)
      if (current === request.current) { setData(result); setLoadError(null) }
    } catch (error) { if (current === request.current) setLoadError(message(error)) }
    finally { if (current === request.current) setRefreshing(false) }
  }, [accountId])
  useEffect(() => {
    if (!desktopAvailable) { setRefreshing(false); return }
    void refresh()
    const listener = () => { void refresh() }
    const events = ['accounts-changed', 'transactions-changed', 'verification-changed', 'focus']
    events.forEach(event => window.addEventListener(event, listener))
    return () => { request.current++; events.forEach(event => window.removeEventListener(event, listener)) }
  }, [refresh])
  async function editAccount() {
    setLoadingEditor(true); setError(null)
    try {
      const account = (await listAccounts()).find(account => account.id === accountId)
      if (!account) throw new Error('This account is no longer available for editing. Refresh and try again.')
      setEditing(account)
    } catch (error) { setError(message(error)) }
    finally { setLoadingEditor(false) }
  }
  const disabled = saving || refreshing || !!loadError || !data || data.is_archived
  const isCard = data?.account_type === 'credit_card'
  const provider = data?.account_type === 'wallet' ? 'wallet' : 'bank'
  let postedValue: string | null = null
  let postedError: string | null = null
  if (posted.trim() && data) {
    try {
      if (isCard && posted.trim().startsWith('-')) throw new Error('Enter a positive amount, then choose Amount owed or Overpayment / credit.')
      postedValue = decimalToInteger(isCard && cardBalanceKind === 'credit' ? '-' + posted.trim() : posted, fractionDigits(data.currency))
    } catch (error) { postedError = message(error) }
  }
  const difference = postedValue !== null && data ? BigInt(postedValue) - BigInt(data.cleared_balance) : null
  const first = data?.history.length === 0
  const money = (value: string) => data ? formatAmount(value, data.currency) : ''
  const balance = (value: string) => isCard ? `${money((BigInt(value) < 0n ? -BigInt(value) : BigInt(value)).toString())} ${BigInt(value) < 0n ? 'overpayment / credit' : 'owed'}` : money(value)
  const visible = useMemo(() => (data?.entries ?? []).filter(entry => (!statusFilter || entry.status === statusFilter) && (!hideReconciled || entry.status !== 'reconciled')), [data, statusFilter, hideReconciled])
  const selectedEntries = visible.filter(entry => selected.has(entry.transaction_id) && entry.status !== 'reconciled')

  async function mutate(action: () => Promise<unknown>, success: string) {
    if (savingRef.current || disabled) return
    savingRef.current = true; setSaving(true); setError(null)
    try {
      await action()
      setSelected(new Set()); setCorrection(null); setCorrectionConfirmed(false)
      toast.success(success)
      await refresh()
    } catch (error) {
      const text = message(error)
      setError(text)
      if (text.toLowerCase().includes('reconciliation history')) setForceConfirmation(true)
      await refresh()
    } finally { savingRef.current = false; setSaving(false) }
  }
  function askCorrection(kind: 'unlock' | 'delete', entry: VerificationEntry) {
    setCorrection({ kind, entry }); setCorrectionConfirmed(false); setForceConfirmation(false); setError(null)
  }
  const requiresConfirmation = forceConfirmation || correction?.kind === 'unlock' || !!data?.history_entries.some(entry => entry.transaction_id === correction?.entry.transaction_id)
  const columns: ColumnDef<VerificationEntry>[] = [
    { id: 'select', header: 'Select', cell: ({ row: { original: entry } }) => <Input type="checkbox" className="size-4 p-0" aria-label={`Select ${entry.description || entry.type} ${entry.date}`} disabled={disabled || entry.status === 'reconciled'} checked={selected.has(entry.transaction_id)} onChange={event => setSelected(current => { const next = new Set(current); if (event.target.checked) next.add(entry.transaction_id); else next.delete(entry.transaction_id); return next })} /> },
    { id: 'date', header: 'Date', accessorKey: 'date' },
    { id: 'description', header: 'Description', meta: { rowHeader: true }, cell: ({ row: { original: entry } }) => entry.description || entry.type },
    { id: 'type', header: 'Type', cell: ({ row: { original: entry } }) => `${entry.type}${entry.destination_account_id ? entry.destination_account_id === accountId ? ' · In' : ' · Out' : ''}` },
    { id: 'change', header: isCard ? 'Change in amount owed' : 'Balance change', meta: { headerClassName: 'text-right', cellClassName: 'text-right whitespace-nowrap tabular-nums' }, cell: ({ row: { original: entry } }) => <span>{BigInt(entry.balance_change) > 0n ? '+' : ''}{money(entry.balance_change)}</span> },
    { id: 'status', header: 'Verification', cell: ({ row: { original: entry } }) => entry.status === 'reconciled'
      ? <Button variant="outline" size="xs" disabled={disabled} onClick={() => askCorrection('unlock', entry)} aria-label={`Unlock reconciled ${entry.description || entry.type} ${entry.date}`}><LockKeyhole size={14} />Reconciled</Button>
      : <Button variant="outline" size="xs" disabled={disabled} aria-pressed={entry.status === 'cleared'} aria-label={`${labels[entry.status]}: ${entry.description || entry.type} ${entry.date}. Toggle verification`} onClick={() => void mutate(() => setVerification(accountId, [entry], entry.status === 'cleared' ? 'uncleared' : 'cleared'), 'Verification updated.')}>
        {entry.status === 'cleared' ? <Check size={14} /> : <Circle size={14} />}{labels[entry.status]}</Button> },
    { id: 'actions', header: 'Actions', cell: ({ row: { original: entry } }) => <Button variant="ghost" size="xs" disabled={disabled} aria-label={`Delete ${entry.description || entry.type} ${entry.date}`} onClick={() => askCorrection('delete', entry)}>Delete</Button> },
  ]
  const table = useReactTable({ data: visible, columns: columns.map(column => ({ ...column, meta: { ...column.meta, headerClassName: 'px-3 py-2 whitespace-nowrap ' + (column.meta?.headerClassName ?? ''), cellClassName: 'px-3 py-1.5 align-middle ' + (column.meta?.cellClassName ?? '') } })), getRowId: entry => entry.transaction_id, getCoreRowModel: getCoreRowModel() })

  if (!desktopAvailable) return <p>Open the desktop app to reconcile accounts.</p>
  return <>
    <Link to="/accounts" className="text-sm text-brand">← Accounts</Link>
    <div className="my-5 flex flex-wrap items-center justify-between gap-4"><div><h2 className="text-2xl font-semibold">{data?.name ?? 'Account transactions'}</h2><p className="mt-1 text-sm">Verify posted activity against your {provider}. Pending entries stay Uncleared.</p></div><div className="flex flex-wrap gap-2"><Button variant="outline" disabled={disabled || loadingEditor} onClick={() => void editAccount()}>{loadingEditor ? 'Loading details…' : 'Edit account'}</Button><Button variant="outline" disabled={disabled} onClick={() => setAdding(true)}>Record missing entry</Button><Button disabled={disabled || reconciling} onClick={() => { setReconciling(true); setError(null) }}>Reconcile</Button></div></div>
    {loadError && <p role="alert" className="mb-4">{loadError} <Button variant="outline" onClick={() => void refresh()}>Refresh</Button></p>}
    {refreshing && <p role="status" className="mb-3 text-sm">Refreshing account…</p>}
    {data && <>
      {data.is_archived && <p className="mb-4">Archived account. History is read-only.</p>}
      <div className="mb-4 grid gap-3 md:grid-cols-3">{[['Cleared balance', data.cleared_balance], ['Uncleared changes', data.uncleared_changes], ['Working balance', data.working_balance]].map(([label, value]) => <div key={label} className="rounded-xl border border-line bg-card p-4"><p className="text-sm text-muted">{label}</p><p className="mt-2 break-words text-lg font-semibold tabular-nums">{label === 'Uncleared changes' ? money(value) : balance(value)}</p></div>)}</div>
      <p className="mb-2 text-xs">Cleared balance includes the opening balance and Cleared/Reconciled activity. Cleared balance + Uncleared changes = Working balance. Verification never changes your balances.{isCard && ' For cards, positive changes increase debt; negative changes reduce debt or add credit. These figures are not available credit.'}</p>
      <p className="mb-5 text-sm">Last reconciliation: {data.history[0] ? new Date(data.history[0].completed_at).toLocaleString() : 'Never'}{data.history[0]?.needs_review && ' · Needs review'}</p>
      {reconciling && <section aria-label="Reconcile account" className="mb-6 space-y-4 rounded-2xl border border-brand bg-card p-5">
        <h3 className="text-lg font-semibold">Match the {provider}’s posted balance</h3>
        <p className="text-sm">Exclude pending transactions. Check posted entries below, and record anything missing.{isCard && ' Enter the amount owed, not available credit. Choose Overpayment / credit when the bank owes you money.'}</p>
        <div className="flex flex-wrap items-end gap-4">{isCard && <Field label="Posted card balance represents"><NativeSelect disabled={saving} value={cardBalanceKind} onChange={event => setCardBalanceKind(event.target.value as 'owed' | 'credit')}><option value="owed">Amount owed</option><option value="credit">Overpayment / credit</option></NativeSelect></Field>}<Field label={`${provider === 'wallet' ? 'Wallet' : 'Bank'} posted balance (${data.currency})`}><Input disabled={saving} inputMode="decimal" value={posted} onChange={event => setPosted(event.target.value)} placeholder="0" /></Field></div>
        {postedError && <p role="alert">{postedError}</p>}
        <p aria-live="polite" className="font-semibold">Difference: {difference === null ? 'Enter the posted balance' : `${money(difference.toString())}${difference === 0n ? ' · Matched' : ''}`}</p>
        {first && <label className="flex items-start gap-2 text-sm"><Input type="checkbox" className="mt-0.5 size-4 shrink-0 p-0" disabled={saving} checked={openingConfirmed} onChange={event => setOpeningConfirmed(event.target.checked)} /><span>I confirm the opening balance of {balance(data.opening_balance)} is correct. It is the starting point before recorded transactions.</span></label>}
        <p className="text-xs">Finishing locks all {data.entries.filter(entry => entry.status === 'cleared').length} Cleared entries in this account. Uncleared entries and the other side of transfers stay unchanged.</p>
        <div className="flex gap-2"><Button disabled={disabled || difference !== 0n || (first && !openingConfirmed)} onClick={() => void mutate(async () => { await finishReconciliation(data, postedValue!, openingConfirmed); setReconciling(false); setPosted(''); setOpeningConfirmed(false) }, 'Reconciliation completed.')}>{saving ? 'Saving…' : 'Finish reconciliation'}</Button><Button variant="outline" disabled={saving} onClick={() => setReconciling(false)}>Cancel reconciliation</Button></div>
      </section>}
      {error && !correction && <p role="alert" className="mb-4">{error}</p>}
      <div className="mb-4 flex flex-wrap items-end gap-4"><Field label="Verification status"><NativeSelect value={statusFilter} onChange={event => { setStatusFilter(event.target.value as VerificationStatus | ''); setSelected(new Set()) }}><option value="">All statuses</option>{Object.entries(labels).map(([status, label]) => <option key={status} value={status}>{label}</option>)}</NativeSelect></Field><label className="flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4 p-0" checked={hideReconciled} onChange={event => { setHideReconciled(event.target.checked); setSelected(new Set()) }} />Hide reconciled</label><Button variant="outline" disabled={disabled || !visible.some(entry => entry.status !== 'reconciled')} onClick={() => setSelected(new Set(visible.filter(entry => entry.status !== 'reconciled').map(entry => entry.transaction_id)))}>Select visible unlocked</Button><Button variant="outline" disabled={disabled || !selectedEntries.length} onClick={() => void mutate(() => setVerification(accountId, selectedEntries, 'cleared'), 'Selected transactions cleared.')}>Clear selected ({selectedEntries.length})</Button><Button variant="ghost" disabled={!selected.size} onClick={() => setSelected(new Set())}>Deselect</Button></div>
      <DataTable table={table} label="Account transaction verification" className="text-[13px]" headerClassName="border-b border-line bg-soft text-xs text-muted" bodyClassName="divide-y divide-line" rowClassName="hover:bg-soft/40" />
      {!visible.length && <p className="mt-3 text-sm">No transactions match these filters.</p>}
      <section aria-label="Reconciliation history" className="mt-8"><h3 className="mb-3 text-lg font-semibold">Reconciliation history</h3>{!data.history.length && <p className="text-sm">No completed reconciliations.</p>}<div className="space-y-3">{data.history.map(history => <details key={history.id} className="rounded-xl border border-line bg-card p-4"><summary className="cursor-pointer text-sm font-medium">{new Date(history.completed_at).toLocaleString()} · {balance(history.confirmed_balance)} · {history.needs_review ? 'Needs review' : 'Completed'}</summary><p className="my-3 text-xs">Opening balance: {balance(history.opening_balance)}. Entries below are saved evidence from completion, including entries later unlocked or deleted.{history.needs_review && ' A correction affected this confirmation or a balance carried into it. Review against your provider and complete a new reconciliation when ready.'}</p><ul className="space-y-2 text-sm">{data.history_entries.filter(entry => entry.reconciliation_id === history.id).map(entry => <li key={entry.transaction_id} className="flex flex-wrap justify-between gap-2"><span>{entry.date} · {entry.description || entry.type}</span><span className="tabular-nums">{money(entry.balance_change)}</span></li>)}</ul>{!data.history_entries.some(entry => entry.reconciliation_id === history.id) && <p className="text-sm">No newly cleared entries were included.</p>}</details>)}</div></section>
    </>}
    {editing && data && <AccountFormDialog account={editing} initialType={editing.type} currency={data.currency} onClose={() => setEditing(null)} onSaved={() => { void refresh() }} />}
    {adding && <AddTransactionDialog initialAccountId={accountId} onClose={() => setAdding(false)} />}
    <Dialog open={!!correction} onOpenChange={open => { if (!open && !saving) { setCorrection(null); setError(null) } }}><DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}><DialogHeader><DialogTitle>{correction?.kind === 'unlock' ? 'Unlock reconciled transaction?' : 'Delete transaction?'}</DialogTitle><DialogDescription>{correction?.kind === 'unlock' ? 'This account’s entry will become Cleared. Its reconciliation and later confirmations will be marked as needing review. Balances and the other account’s verification stay unchanged.' : 'Deleting reverses this transaction in every participating account. Saved reconciliation evidence is retained; affected history in either account will need review.'}</DialogDescription></DialogHeader>{requiresConfirmation && <label className="flex items-start gap-2 text-sm"><Input type="checkbox" className="mt-0.5 size-4 shrink-0 p-0" checked={correctionConfirmed} disabled={saving} onChange={event => setCorrectionConfirmed(event.target.checked)} />I confirm this correction and understand affected reconciliations will need review.</label>}{error && <p role="alert">{error}</p>}<DialogFooter><Button variant="outline" disabled={saving} onClick={() => setCorrection(null)}>Cancel</Button><Button variant="destructive" disabled={disabled || (requiresConfirmation && !correctionConfirmed)} onClick={() => correction && void mutate(() => correction.kind === 'unlock' ? setVerification(accountId, [correction.entry], 'cleared', correctionConfirmed) : deleteTransaction(correction.entry.transaction_id, correctionConfirmed), correction.kind === 'unlock' ? 'Transaction unlocked. Review reconciliation history.' : 'Transaction deleted. Balances updated.')}>{correction?.kind === 'unlock' ? 'Unlock transaction' : 'Delete transaction'}</Button></DialogFooter></DialogContent></Dialog>
  </>
}
