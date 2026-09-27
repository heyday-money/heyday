import { CategoryIcon, categoryIcons } from './CategoryIcon'
import { useEffect, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { listTransactionOptions, saveTransactionOption, type TransactionOption, type TransactionOptionKind, type TransactionOptions } from '../lib/desktop'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

const groups = [
  { kind: 'payee', key: 'payees', title: 'Payees', label: 'payee', description: 'People and businesses you pay.' },
  { kind: 'category', key: 'categories', title: 'Spending categories', label: 'category', description: 'Organize expenses, such as Groceries, Transport, or Housing.' },
] as const
const errorText = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : 'Could not save. Please try again.'

export function TransactionOptionsSettings({ kind }: { kind: TransactionOptionKind }) {
  const [options, setOptions] = useState<TransactionOptions>({ payees: [], categories: [] })
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [editing, setEditing] = useState<{ kind: TransactionOptionKind; item?: TransactionOption } | null>(null)
  const [icon, setIcon] = useState('tag')
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
    setEditing({ kind, item }); setName(item?.name ?? ''); setIcon(item?.icon ?? 'tag'); setError(null); setDiscard(false)
  }
  function close() { setEditing(null); setError(null); setDiscard(false) }
  function changeOpen(open: boolean) {
    if (open || saving) return
    if (name !== (editing?.item?.name ?? '') || (editing?.kind === 'category' && icon !== (editing.item?.icon ?? 'tag'))) setDiscard(true)
    else close()
  }
  async function save(event: FormEvent) {
    event.preventDefault()
    if (!editing || saving) return
    if (!name.trim()) { setError('Enter a name.'); return }
    setSaving(true); setError(null)
    try {
      const value = await saveTransactionOption({ kind: editing.kind, icon: editing.kind === 'category' ? icon : undefined, id: editing.item?.id ?? null, name, is_archived: editing.item?.is_archived ?? false })
      replace(editing.kind, value); close(); toast.success('Saved.')
    } catch (error) { setError(errorText(error)) } finally { setSaving(false) }
  }
  async function toggle(kind: TransactionOptionKind, item: TransactionOption) {
    if (saving) return
    setSaving(true); setError(null)
    try {
      const value = await saveTransactionOption({ kind, icon: kind === 'category' ? item.icon : undefined, id: item.id, name: item.name, is_archived: !item.is_archived })
      replace(kind, value); toast.success(value.is_archived ? 'Archived. Existing transactions are preserved.' : 'Restored.')
    } catch (error) { setError(errorText(error)) } finally { setSaving(false) }
  }
  return <div className="space-y-5">
    <p className="text-sm">Manage the payees and categories available when recording an expense. Renaming updates history labels; archiving hides an option from new expenses and preserves its history.</p>
    {loading ? <p role="status">Loading payees and categories…</p> : loadError ? <div role="alert">Could not load payees and categories. <Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry lists</Button></div> : groups.filter(group => group.kind === kind).map(group => <section key={group.kind} aria-label={group.title} className="rounded-[22px] border border-line bg-card p-6">
      <div className="flex flex-wrap items-center justify-between gap-3"><h2 className="text-lg font-semibold">{group.title}</h2><Button disabled={saving} onClick={() => start(group.kind)}>Add {group.label}</Button></div>
      <p className="mt-2 text-sm">{group.description}</p>
      {!options[group.key].length ? <p className="mt-4 text-sm">No {group.key} yet. Add your first {group.label}.</p> : <ul className="mt-4 divide-y divide-line" aria-label={group.title}>{options[group.key].map(item => <li key={item.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
        <span className="flex min-w-0 items-center gap-2 break-words font-medium">{kind === 'category' && <CategoryIcon name={item.icon} />}{item.name}{item.is_archived && <span className="ml-2 text-xs font-normal text-muted">Archived</span>}</span>
        <div className="flex gap-2"><Button size="sm" variant="outline" disabled={saving} onClick={() => start(group.kind, item)} aria-label={`${kind === 'category' ? 'Edit' : 'Rename'} ${item.name}`}>{kind === 'category' ? 'Edit' : 'Rename'}</Button><Button size="sm" variant="outline" disabled={saving} onClick={() => toggle(group.kind, item)} aria-label={`${item.is_archived ? 'Restore' : 'Archive'} ${item.name}`}>{item.is_archived ? 'Restore' : 'Archive'}</Button></div>
      </li>)}</ul>}
    </section>)}
    {error && !editing && <p role="alert">{error}</p>}
    <Dialog open={!!editing} onOpenChange={changeOpen}>
      <DialogContent showCloseButton={!saving} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (saving) event.preventDefault() }}>
        <DialogHeader><DialogTitle>{editing?.item ? editing.kind === 'category' ? 'Edit' : 'Rename' : 'Add'} {editing?.kind}</DialogTitle><DialogDescription>{editing?.item ? 'The updated name will appear on existing transactions.' : 'Use this name to organize your expenses.'}</DialogDescription></DialogHeader>
        <form onSubmit={save} className="space-y-4">
          <FormField label={editing?.kind === 'payee' ? 'Payee name' : 'Category name'}><Input className="mt-2" autoFocus required maxLength={100} value={name} disabled={saving} onChange={event => setName(event.target.value)} /></FormField>
          {editing?.kind === 'category' && <fieldset disabled={saving}><legend className="mb-2 text-sm font-medium">Category icon</legend><div className="grid max-h-48 grid-cols-6 gap-2 overflow-y-auto p-1">{categoryIcons.map(choice => <Button key={choice.key} type="button" variant={icon === choice.key ? 'secondary' : 'outline'} size="icon" aria-label={`Icon: ${choice.label}`} aria-pressed={icon === choice.key} title={choice.label} onClick={() => setIcon(choice.key)}><CategoryIcon name={choice.key} /></Button>)}</div></fieldset>}
          {error && <p role="alert">{error}</p>}
          {discard ? <><p role="alert">Discard your unsaved changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={saving} onClick={() => changeOpen(false)}>Cancel</Button><Button type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save'}</Button></DialogFooter>}
        </form>
      </DialogContent>
    </Dialog>
  </div>
}
