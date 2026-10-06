import { t as translate, useLanguage } from "../lib/i18n"
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
  useLanguage()

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
    try { await saveInstitution({ logo_change: logoChange, id: editing === 'new' ? null : editing.id, name, is_archived: editing === 'new' ? false : editing.is_archived }); setEditing(null); toast.success(translate("Institution saved.")) }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  async function toggle(item: Institution) {
    if (busy || logoBusy) return
    setBusy(true); setError(null)
    try { await saveInstitution({ id: item.id, name: item.name, is_archived: !item.is_archived }); toast.success(item.is_archived ? translate("Institution restored.") : translate("Institution archived.")) }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  async function remove() {
    if (!deleting || busy) return
    setBusy(true); setError(null)
    try { await deleteInstitution(deleting.id); setDeleting(null); toast.success(translate("Institution permanently deleted.")) }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  return <section className="min-w-0">
    <div className="flex flex-wrap items-center justify-between gap-3"><h3 className="text-lg font-semibold">{translate("Institutions")}</h3><Button disabled={busy || directory.loading || directory.error} onClick={() => { setEditing('new'); setLogoChange(undefined); setName(''); setError(null); setDiscard(false) }}>{translate("Add institution")}</Button></div>
    <p className="my-3 text-sm">{translate("Renaming updates linked accounts and their institution labels throughout the app, including transaction history. Archiving hides an institution from new selections and keeps existing links.")}</p>
    <p className="mb-3 text-sm">{translate("Permanent deletion is available only when no accounts are linked, including archived accounts. Transaction history links through those accounts.")}</p>
    <Input aria-label={translate("Search institutions")} placeholder={translate("Search institutions")} value={search} onChange={e => setSearch(e.target.value)} />
    {directory.loading && <p role="status">{translate("Loading institutions…")}</p>}
    {directory.error && <p role="alert">{translate("Could not load institutions.")}{" "}<Button variant="outline" onClick={directory.reload}>{translate("Retry")}</Button></p>}
    {!directory.error && <ul className="mt-3 divide-y divide-line">{directory.institutions.filter(i => [i.name, i.short_name, i.bank_code, i.swift_code].some(s => s?.toLowerCase().includes(search.toLowerCase()))).map(i => <li key={i.id} className="flex flex-wrap items-center gap-3 py-3">
      <InstitutionLogo institution={i} className="size-7" /><div className="min-w-0 flex-1"><p className="break-words text-sm font-medium text-ink">{i.name}{i.is_archived && translate(" (archived)")}</p>{i.short_name && <p className="text-xs">{i.short_name} · {i.bank_code} · {i.swift_code}</p>}</div>
      <Button size="sm" variant="outline" disabled={busy || logoBusy} aria-label={translate("Edit institution {value0}", { value0: i.name })} onClick={() => { setEditing(i); setLogoChange(undefined); setName(i.name); setError(null); setDiscard(false) }}>{translate("Edit")}</Button>
      <Button size="sm" variant="ghost" disabled={busy || logoBusy} aria-label={translate("{value0} institution {value1}", { value0: i.is_archived ? translate("Restore") : translate("Archive"), value1: i.name })} onClick={() => void toggle(i)}>{i.is_archived ? translate("Restore") : translate("Archive")}</Button>
      <Button size="sm" variant="destructive" disabled={busy || logoBusy || directory.loading || directory.accounts.some(a => a.institution_id === i.id)} aria-label={translate("Delete institution {value0}", { value0: i.name })} onClick={() => { setDeleting(i); setError(null) }}>{translate("Delete")}</Button>
    </li>)}</ul>}
    {error && !editing && !deleting && <p role="alert">{translate(error)}</p>}
    {deleting && <Dialog open onOpenChange={open => { if (!open && !busy) { setDeleting(null); setError(null) } }}><DialogContent onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }} showCloseButton={!busy}>
      <DialogHeader><DialogTitle>{translate("Delete institution permanently?")}</DialogTitle><DialogDescription>{translate("Delete")}{" "}{deleting.name}{translate("? This cannot be undone. Its custom logo will also be removed if no other record uses it.")}</DialogDescription></DialogHeader>
      {error && <p role="alert">{translate(error)}</p>}
      <DialogFooter><Button autoFocus variant="outline" disabled={busy} onClick={() => { setDeleting(null); setError(null) }}>{translate("Cancel")}</Button><Button variant="destructive" disabled={busy} onClick={() => void remove()}>{busy ? translate("Deleting…") : translate("Delete permanently")}</Button></DialogFooter>
    </DialogContent></Dialog>}
    {editing && <Dialog open onOpenChange={open => { if (!open) close() }}><DialogContent className="max-h-[85dvh] overflow-y-auto" onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy || logoBusy) e.preventDefault() }} showCloseButton={!busy && !logoBusy}>
      <DialogHeader><DialogTitle>{editing === 'new' ? translate("Add institution") : translate("Edit institution")}</DialogTitle><DialogDescription>{translate("The institution name is shared across linked accounts. Account names and balances stay the same.")}</DialogDescription></DialogHeader>
      <form onSubmit={save} className="space-y-4"><FormField label={translate("Institution name")}><Input autoFocus required maxLength={100} value={name} disabled={busy || logoBusy} onChange={e => setName(e.target.value)} /></FormField><LogoPicker hasSavedLogo={editing !== 'new' && !!editing.logo_asset_id} name={name} value={logoChange} savedSource={savedLogo} defaultSource={institutionDefaultLogo(editing === 'new' ? undefined : editing)} disabled={busy} onChange={setLogoChange} onBusyChange={setLogoBusy} />{error && <p role="alert">{translate(error)}</p>}
      {discard ? <><p role="alert">{translate("Discard your unsaved institution?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={() => setEditing(null)}>{translate("Discard changes")}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={busy || logoBusy} onClick={close}>{translate("Cancel")}</Button><Button type="submit" disabled={busy || logoBusy}>{busy ? translate("Saving…") : translate("Save institution")}</Button></DialogFooter>}</form>
    </DialogContent></Dialog>}
  </section>
}
