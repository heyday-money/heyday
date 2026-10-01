import { useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { type CardLimitGroup, type CardLimits, deleteCardLimitGroup, saveCardLimitGroup, sharedCredit, useCardLimits } from '../lib/card-limits'
import { decimalToInteger, formatAmount, fractionDigits } from '../lib/money'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

export function SharedCreditLimits({ currency, limits }: { currency: string; limits: ReturnType<typeof useCardLimits> }) {
  const [editing, setEditing] = useState<CardLimitGroup | 'new' | null>(null)
  return <section className="my-5 rounded-2xl border border-line bg-card p-5" aria-label="Shared credit limits">
    <div className="flex flex-wrap items-center justify-between gap-3"><h3 className="font-semibold">Shared credit limits</h3><Button disabled={limits.loading || limits.error} onClick={() => setEditing('new')}>Add shared limit</Button></div>
    <p className="mt-2 text-sm">Group cards that share one credit limit. Billing and payments stay separate. Repayments and overpayments restore shared availability, which can exceed the limit. Estimates exclude pending authorizations and issuer holds.</p>
    {limits.loading ? <p role="status">Loading shared limits…</p> : limits.error ? <p role="alert">Could not load shared limits. <Button variant="outline" onClick={() => void limits.reload()}>Retry</Button></p> : <ul className="mt-3 space-y-3">{limits.data.groups.map(group => {
      const members = limits.data.cards.filter(c => c.group_id === group.id)
      const total = members.reduce((sum,c) => sum + BigInt(c.current_balance), 0n)
      return <li key={group.id} className="rounded-xl border border-line p-4"><div className="flex items-center justify-between gap-3"><h4 className="font-semibold">{group.name}</h4><Button variant="outline" aria-label={`Edit shared limit ${group.name}`} onClick={() => setEditing(group)}>Edit</Button></div>
        <p className="mt-2 text-sm">Limit: {formatAmount(group.credit_limit, currency)} · Combined owed / credit: {formatAmount(total.toString(), currency)}</p>
        <p className="mt-1 text-sm font-semibold">Estimated shared available credit: {formatAmount((BigInt(group.credit_limit) - total).toString(), currency)}</p>
        <p className="mt-2 text-xs">{members.map(c => `${c.name}${c.is_archived ? ' (archived)' : ''}`).join(', ')}</p>
      </li>
    })}</ul>}
    {editing && <GroupEditor group={editing === 'new' ? undefined : editing} data={limits.data} currency={currency} close={() => setEditing(null)} />}
  </section>
}

function GroupEditor({ group, data, currency, close }: { group?: CardLimitGroup; data: CardLimits; currency: string; close: () => void }) {
  const digits = fractionDigits(currency), scale = 10n ** BigInt(digits), initial = BigInt(group?.credit_limit ?? '0')
  const [name, setName] = useState(group?.name ?? '')
  const [limit, setLimit] = useState(group ? `${initial / scale}${digits ? '.' + (initial % scale).toString().padStart(digits, '0') : ''}` : '')
  const [members, setMembers] = useState(data.cards.filter(c => group && c.group_id === group.id).map(c => c.id))
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [dirty, setDirty] = useState(false), [discard, setDiscard] = useState(false), [removing, setRemoving] = useState(false)
  const requestClose = () => { if (busy) return; if (dirty) setDiscard(true); else close() }
  async function save(e: FormEvent) {
    e.preventDefault(); if (busy || discard || removing) return
    setBusy(true); setError(null)
    try { await saveCardLimitGroup({ id: group?.id ?? null, name, credit_limit: decimalToInteger(limit, digits), currency, account_ids: members }); close(); toast.success('Shared credit limit saved.') }
    catch (e) { setError(e instanceof Error ? e.message : String(e)) } finally { setBusy(false) }
  }
  async function remove() {
    if (!group || busy) return
    setBusy(true); setError(null)
    try { await deleteCardLimitGroup(group.id); close(); toast.success('Shared credit limit removed.') }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  return <Dialog open onOpenChange={open => { if (!open) requestClose() }}><DialogContent className="flex max-h-[85dvh] flex-col" onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }} showCloseButton={!busy}>
    <DialogHeader><DialogTitle>{group ? 'Edit shared credit limit' : 'Add shared credit limit'}</DialogTitle><DialogDescription>Each card belongs to at most one group. Removing a card restores its saved individual limit. Balances, statements, and transactions stay unchanged.</DialogDescription></DialogHeader>
    <form onSubmit={save} onChange={() => setDirty(true)} className="flex min-h-0 flex-col gap-4">
      <fieldset disabled={busy || removing || discard} className="min-h-0 space-y-4 overflow-y-auto">
        <FormField label="Group name"><Input autoFocus value={name} onChange={e => setName(e.target.value)} required maxLength={100} /></FormField>
        <FormField label={`Shared credit limit (${currency})`}><Input value={limit} onChange={e => setLimit(e.target.value)} inputMode="decimal" required /></FormField>
        <fieldset className="space-y-2"><legend className="mb-2 font-medium">Member cards</legend>{data.cards.map(card => {
          const unavailable = (!!card.group_id && card.group_id !== group?.id) || (card.is_archived && card.group_id !== group?.id)
          return <label key={card.id} className="flex items-center gap-2 text-sm"><Input className="size-4 p-0" type="checkbox" checked={members.includes(card.id)} disabled={unavailable} onChange={e => setMembers(current => e.target.checked ? [...current, card.id] : current.filter(id => id !== card.id))} />{card.name}{card.is_archived ? ' (archived)' : ''}{card.group_id && card.group_id !== group?.id ? ` — ${data.groups.find(g => g.id === card.group_id)?.name}` : ''}</label>
        })}{!data.cards.length && <p className="text-sm">Add credit card accounts first.</p>}</fieldset>
      </fieldset>
      {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{error}</p>}
      {discard ? <><p role="alert">Discard your unsaved shared limit?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter></> : removing ? <><p role="alert">Remove this shared group? All member cards will use their saved individual limits. This cannot be undone.</p><DialogFooter><Button type="button" variant="outline" disabled={busy} onClick={() => setRemoving(false)}>Keep group</Button><Button type="button" variant="destructive" disabled={busy} onClick={() => void remove()}>Remove group</Button></DialogFooter></> : <DialogFooter>{group && <Button type="button" variant="destructive" disabled={busy} onClick={() => setRemoving(true)}>Remove shared limit</Button>}<Button type="button" variant="outline" disabled={busy} onClick={requestClose}>Cancel</Button><Button disabled={busy || !members.length} type="submit">{busy ? 'Saving…' : 'Save shared limit'}</Button></DialogFooter>}
    </form>
  </DialogContent></Dialog>
}

export function CardCredit({ id, individualLimit, balance, currency, limits }: { id: string; individualLimit: string | null; balance: string; currency: string; limits: ReturnType<typeof useCardLimits> }) {
  if (limits.loading) return <p className="mt-3 text-sm">Loading credit availability…</p>
  if (limits.error) return <p className="mt-3 text-sm" role="alert">Credit availability unavailable. <Button variant="ghost" onClick={() => void limits.reload()}>Retry</Button></p>
  const shared = sharedCredit(limits.data, id)
  const limit = shared?.credit_limit ?? individualLimit
  return <div className="mt-3 text-sm"><p>{shared ? `Shared limit · ${shared.name}` : 'Credit limit'}: {limit == null ? 'Not configured' : formatAmount(limit, currency)}</p>{limit != null && <p className="mt-1">Estimated {shared ? 'shared ' : ''}available credit: {formatAmount((shared?.available ?? BigInt(limit) - BigInt(balance)).toString(), currency)}</p>}</div>
}
