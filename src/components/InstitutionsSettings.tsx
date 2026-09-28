import { LogoPicker } from './LogoPicker'
import type { LogoChange } from '../lib/logos'
import { useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { deleteInstitution, saveInstitution, type Institution } from '../lib/institutions'
import { useInstitutions } from './InstitutionProvider'
import { InstitutionLogo, useInstitutionLogo, institutionDefaultLogo } from './InstitutionLogo'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
export function InstitutionsSettings() {
  const directory = useInstitutions()
  const [search, setSearch] = useState('')
  const [editing, setEditing] = useState<Institution | 'new' | null>(null)
  const [deleting, setDeleting] = useState<Institution | null>(null)
  const [name, setName] = useState('')
  const [logoChange, setLogoChange] = useState<LogoChange>()
  const [logoBusy, setLogoBusy] = useState(false)
  const savedLogo = useInstitutionLogo(editing && editing !== 'new' ? editing : undefined)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [discard, setDiscard] = useState(false)
  const dirty = editing && (name !== (editing === 'new' ? '' : editing.name) || logoChange !== undefined)
  function close() { if (busy || logoBusy) return; if (dirty) setDiscard(true); else setEditing(null) }
  async function save(e: FormEvent) {
    e.preventDefault(); if (!editing || busy || logoBusy) return
    setBusy(true); setError(null)
    try { await saveInstitution({ logo_change: logoChange, id: editing === 'new' ? null : editing.id, name, is_archived: editing === 'new' ? false : editing.is_archived }); setEditing(null); toast.success('Institution saved.') }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  async function toggle(item: Institution) {
    if (busy || logoBusy) return
    setBusy(true); setError(null)
    try { await saveInstitution({ id: item.id, name: item.name, is_archived: !item.is_archived }); toast.success(item.is_archived ? 'Institution restored.' : 'Institution archived.') }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  async function remove() {
    if (!deleting || busy) return
    setBusy(true); setError(null)
    try { await deleteInstitution(deleting.id); setDeleting(null); toast.success('Institution permanently deleted.') }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  return <section className="rounded-[22px] border border-line bg-card p-6">
    <div className="flex items-center justify-between gap-3"><h3 className="text-lg font-semibold">Institutions</h3><Button disabled={busy || directory.loading || directory.error} onClick={() => { setEditing('new'); setLogoChange(undefined); setName(''); setError(null); setDiscard(false) }}>Add institution</Button></div>
    <p className="my-3 text-sm">Renaming updates linked accounts and their institution labels throughout the app, including transaction history. Archiving hides an institution from new selections and keeps existing links.</p>
    <p className="mb-3 text-sm">Permanent deletion is available only when no accounts are linked, including archived accounts. Transaction history links through those accounts.</p>
    <Input aria-label="Search institutions" placeholder="Search institutions" value={search} onChange={e => setSearch(e.target.value)} />
    {directory.loading && <p role="status">Loading institutions…</p>}
    {directory.error && <p role="alert">Could not load institutions. <Button variant="outline" onClick={directory.reload}>Retry</Button></p>}
    {!directory.error && <ul className="mt-3 divide-y divide-line">{directory.institutions.filter(i => [i.name, i.short_name, i.bank_code, i.swift_code].some(s => s?.toLowerCase().includes(search.toLowerCase()))).map(i => <li key={i.id} className="flex flex-wrap items-center gap-3 py-3">
      <InstitutionLogo institution={i} className="size-7" /><div className="min-w-0 flex-1"><p className="break-words text-sm font-medium text-ink">{i.name}{i.is_archived && ' (archived)'}</p>{i.short_name && <p className="text-xs">{i.short_name} · {i.bank_code} · {i.swift_code}</p>}</div>
      <Button size="sm" variant="outline" disabled={busy || logoBusy} aria-label={`Edit institution ${i.name}`} onClick={() => { setEditing(i); setLogoChange(undefined); setName(i.name); setError(null); setDiscard(false) }}>Edit</Button>
      <Button size="sm" variant="ghost" disabled={busy || logoBusy} aria-label={`${i.is_archived ? 'Restore' : 'Archive'} institution ${i.name}`} onClick={() => void toggle(i)}>{i.is_archived ? 'Restore' : 'Archive'}</Button>
      <Button size="sm" variant="destructive" disabled={busy || logoBusy || directory.loading || directory.accounts.some(a => a.institution_id === i.id)} aria-label={`Delete institution ${i.name}`} onClick={() => { setDeleting(i); setError(null) }}>Delete</Button>
    </li>)}</ul>}
    {error && !editing && !deleting && <p role="alert">{error}</p>}
    {deleting && <Dialog open onOpenChange={open => { if (!open && !busy) { setDeleting(null); setError(null) } }}><DialogContent onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }} showCloseButton={!busy}>
      <DialogHeader><DialogTitle>Delete institution permanently?</DialogTitle><DialogDescription>Delete {deleting.name}? This cannot be undone. Its custom logo will also be removed if no other record uses it.</DialogDescription></DialogHeader>
      {error && <p role="alert">{error}</p>}
      <DialogFooter><Button autoFocus variant="outline" disabled={busy} onClick={() => { setDeleting(null); setError(null) }}>Cancel</Button><Button variant="destructive" disabled={busy} onClick={() => void remove()}>{busy ? 'Deleting…' : 'Delete permanently'}</Button></DialogFooter>
    </DialogContent></Dialog>}
    {editing && <Dialog open onOpenChange={open => { if (!open) close() }}><DialogContent className="max-h-[85dvh] overflow-y-auto" onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy || logoBusy) e.preventDefault() }} showCloseButton={!busy && !logoBusy}>
      <DialogHeader><DialogTitle>{editing === 'new' ? 'Add institution' : 'Edit institution'}</DialogTitle><DialogDescription>The institution name is shared across linked accounts. Account names and balances stay the same.</DialogDescription></DialogHeader>
      <form onSubmit={save} className="space-y-4"><FormField label="Institution name"><Input autoFocus required maxLength={100} value={name} disabled={busy || logoBusy} onChange={e => setName(e.target.value)} /></FormField><LogoPicker hasSavedLogo={editing !== 'new' && !!editing.logo_asset_id} name={name} value={logoChange} savedSource={savedLogo} defaultSource={institutionDefaultLogo(editing === 'new' ? undefined : editing)} disabled={busy} onChange={setLogoChange} onBusyChange={setLogoBusy} />{error && <p role="alert">{error}</p>}
      {discard ? <><p role="alert">Discard your unsaved institution?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={() => setEditing(null)}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={busy || logoBusy} onClick={close}>Cancel</Button><Button type="submit" disabled={busy || logoBusy}>{busy ? 'Saving…' : 'Save institution'}</Button></DialogFooter>}</form>
    </DialogContent></Dialog>}
  </section>
}
