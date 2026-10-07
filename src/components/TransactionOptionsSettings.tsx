import { t as translate, useLanguage } from "../lib/i18n"
import { LogoPicker } from './LogoPicker'
import { PayeeLogo } from './PayeeLogo'
import { useLogoAsset } from './LogoImage'
import type { LogoChange } from '../lib/logos'
import { CategoryIcon, categoryIcons } from './CategoryIcon'
import { useEffect, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { listTransactionOptions, saveTransactionOption, type TransactionOption, type TransactionOptionKind, type TransactionOptions } from '../lib/desktop'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const groups = [
  { kind: 'payee', key: 'payees', get title() { return translate("Payees") }, get label() { return translate("payee") }, get description() { return translate("People and businesses you pay.") } },
  { kind: 'category', key: 'categories', get title() { return translate("Spending categories") }, get label() { return translate("category") }, get description() { return translate("Organize expenses, such as Groceries, Transport, or Housing.") } },
] as const
const errorText = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : "Could not save. Please try again."

export function TransactionOptionsSettings({ kind }: { kind: TransactionOptionKind }) {
  useLanguage()

  const [options, setOptions] = useState<TransactionOptions>({ payees: [], categories: [] })
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [editing, setEditing] = useState<{ kind: TransactionOptionKind; item?: TransactionOption } | null>(null)
  const [icon, setIcon] = useState('tag')
  const [logoChange, setLogoChange] = useState<LogoChange>()
  const [logoBusy, setLogoBusy] = useState(false)
  const savedLogo = useLogoAsset(editing?.item?.logo_asset_id)
  const [name, setName] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [discard, setDiscard] = useState(false)
  useEffect(() => {
    let active = true
    setLoading(true); setLoadError(false)
    listTransactionOptions().then(value => { if (active) setOptions(value) })
      .catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  function replace(kind: TransactionOptionKind, value: TransactionOption) {
    const key = kind === 'payee' ? 'payees' : 'categories'
    setOptions(current => ({ ...current, [key]: [...current[key].filter(item => item.id !== value.id), value].sort((a, b) => Number(a.is_archived) - Number(b.is_archived) || a.name.localeCompare(b.name)) }))
  }
  function start(kind: TransactionOptionKind, item?: TransactionOption) {
    setEditing({ kind, item }); setLogoChange(undefined); setName(item?.name ?? ''); setIcon(item?.icon ?? 'tag'); setError(null); setDiscard(false)
  }
  function close() { setEditing(null); setError(null); setDiscard(false) }
  function changeOpen(open: boolean) {
    if (open || saving || logoBusy) return
    if (logoChange !== undefined || name !== (editing?.item?.name ?? '') || (editing?.kind === 'category' && icon !== (editing.item?.icon ?? 'tag'))) setDiscard(true)
    else close()
  }
  async function save(event: FormEvent) {
    event.preventDefault()
    if (!editing || saving || logoBusy) return
    if (!name.trim()) { setError("Enter a name."); return }
    setSaving(true); setError(null)
    try {
      const value = await saveTransactionOption({ logo_change: editing.kind === 'payee' ? logoChange : undefined, kind: editing.kind, icon: editing.kind === 'category' ? icon : undefined, id: editing.item?.id ?? null, name, is_archived: editing.item?.is_archived ?? false })
      replace(editing.kind, value); close(); toast.success(translate("Saved."))
    } catch (error) { setError(errorText(error)) } finally { setSaving(false) }
  }
  async function toggle(kind: TransactionOptionKind, item: TransactionOption) {
    if (saving || logoBusy) return
    setSaving(true); setError(null)
    try {
      const value = await saveTransactionOption({ kind, icon: kind === 'category' ? item.icon : undefined, id: item.id, name: item.name, is_archived: !item.is_archived })
      replace(kind, value); toast.success(value.is_archived ? translate("Archived. Existing transactions are preserved.") : translate("Restored."))
    } catch (error) { setError(errorText(error)) } finally { setSaving(false) }
  }
  return <div className="space-y-5">
    <p className="text-sm">{translate("Manage the payees and categories available when recording an expense. Renaming updates history labels; archiving hides an option from new expenses and preserves its history.")}</p>
    {loading ? <p role="status">{translate("Loading payees and categories…")}</p> : loadError ? <div role="alert">{translate("Could not load payees and categories.")}{" "}<Button variant="outline" onClick={() => setAttempt(value => value + 1)}>{translate("Retry lists")}</Button></div> : groups.filter(group => group.kind === kind).map(group => <section key={group.kind} aria-label={group.title} className="min-w-0 border-t border-line pt-5">
      <div className="flex flex-wrap items-center justify-between gap-3"><h2 className="text-lg font-semibold">{group.title}</h2><Button disabled={saving || logoBusy} onClick={() => start(group.kind)}>{translate("Add")}{" "}{group.label}</Button></div>
      <p className="mt-2 text-sm">{group.description}</p>
      {!options[group.key].length ? <p className="mt-4 text-sm">{translate("No")}{" "}{group.key} {" "}{translate("yet. Add your first")}{" "}{group.label}.</p> : <ul className="mt-4 divide-y divide-line" aria-label={group.title}>{options[group.key].map(item => <li key={item.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
        <span className="flex min-w-0 items-center gap-2 break-words font-medium">{kind === 'category' ? <CategoryIcon name={item.icon} /> : <PayeeLogo id={item.id} name={item.name} assetId={item.logo_asset_id} />}{item.name}{item.is_archived && <span className="ml-2 text-xs font-normal text-muted">{translate("Archived")}</span>}</span>
        <div className="flex flex-wrap gap-2"><Button size="sm" variant="outline" disabled={saving || logoBusy} onClick={() => start(group.kind, item)} aria-label={translate("Edit {value0}", { value0: item.name })}>{translate("Edit")}</Button><Button size="sm" variant="outline" disabled={saving || logoBusy} onClick={() => toggle(group.kind, item)} aria-label={translate("{value0} {value1}", { value0: item.is_archived ? translate("Restore") : translate("Archive"), value1: item.name })}>{item.is_archived ? translate("Restore") : translate("Archive")}</Button></div>
      </li>)}</ul>}
    </section>)}
    {error && !editing && <p role="alert">{translate(error)}</p>}
    <Dialog open={!!editing} onOpenChange={changeOpen}>
      <DialogContent className="max-h-[85dvh] overflow-y-auto" showCloseButton={!saving && !logoBusy} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving || logoBusy) event.preventDefault() }}>
        <DialogHeader><DialogTitle>{editing?.item ? translate("Edit") : translate("Add")} {editing?.kind}</DialogTitle><DialogDescription>{editing?.item ? translate("The updated name will appear on existing transactions.") : translate("Use this name to organize your expenses.")}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="space-y-4">
          <FormField label={editing?.kind === 'payee' ? translate("Payee name") : translate("Category name")}><Input className="mt-2" autoFocus required maxLength={100} value={name} disabled={saving || logoBusy} onChange={event => setName(event.target.value)} /></FormField>
          {editing?.kind === 'payee' && <LogoPicker hasSavedLogo={!!editing.item?.logo_asset_id} name={name} value={logoChange} savedSource={savedLogo} disabled={saving} onChange={setLogoChange} onBusyChange={setLogoBusy} />}
          {editing?.kind === 'category' && <fieldset disabled={saving || logoBusy}><legend className="mb-2 text-sm font-medium">{translate("Category icon")}</legend><div className="grid max-h-48 grid-cols-6 gap-2 overflow-y-auto p-1">{categoryIcons.map(choice => <Button key={choice.key} type="button" variant={icon === choice.key ? 'secondary' : 'outline'} size="icon" aria-label={translate("Icon: {value0}", { value0: choice.label })} aria-pressed={icon === choice.key} title={choice.label} onClick={() => setIcon(choice.key)}><CategoryIcon name={choice.key} /></Button>)}</div></fieldset>}
          {error && <p role="alert">{translate(error)}</p>}
          {discard ? <><p role="alert">{translate("Discard your unsaved changes?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard changes")}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving || logoBusy} onClick={() => changeOpen(false)}>{translate("Cancel")}</Button><Button type="submit" disabled={saving || logoBusy}>{saving ? translate("Saving…") : translate("Save")}</Button></DialogFooter>}
        </form>
      </DialogContent>
    </Dialog>
  </div>
}
