import { t as translate, useLanguage } from "../lib/i18n"
import { ChevronDown } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { useInstitutions } from './InstitutionProvider'
import { InstitutionLogo } from './InstitutionLogo'
import { Input } from './ui/input'
import { Button } from './ui/button'
const normalize = (s: string) => s.trim().replace(/\s+/gu, ' ')
export function InstitutionCombobox({ id, value, draftName, disabled = false, onChange, onUnresolvedChange }: {
  id?: string; value: string; draftName: string | null; disabled?: boolean
  onChange: (id: string, name: string | null) => void; onUnresolvedChange: (value: boolean) => void
}) {
  useLanguage()

  const { institutions, loading, error, reload } = useInstitutions()
  const selected = institutions.find(i => i.id === value)
  const selectedName = selected?.name ?? draftName ?? ''
  const [query, setQuery] = useState(selectedName)
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const listId = useId()
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => { setQuery(selectedName) }, [selectedName])
  const name = normalize(query), key = name.toLowerCase()
  const exact = institutions.find(i => normalize(i.name).toLowerCase() === key || i.short_name?.toLowerCase() === key || i.bank_code === key || i.swift_code?.toLowerCase() === key)
  const searching = query !== selectedName
  const matches = institutions.filter(i => (!i.is_archived || i.id === value) && (!searching || [i.name, i.short_name, i.bank_code, i.swift_code].some(v => v?.toLowerCase().includes(key))))
  const choices = [
    { id: '', name: translate("No institution"), create: false },
    ...matches.map(i => ({ id: i.id, name: i.name, create: false })),
    ...(name && !exact && [...name].length <= 100 ? [{ id: 'create', name, create: true }] : []),
  ]
  const index = Math.min(active, choices.length - 1)
  function choose(n: number) {
    if (disabled || loading || error) return
    const choice = choices[n]
    if (!choice) return
    onChange(choice.create ? '' : choice.id, choice.create ? name : null)
    onUnresolvedChange(false)
    setQuery(choice.id ? choice.name : ''); setOpen(false); input.current?.focus()
  }
  useEffect(() => { if (open) document.getElementById(`${listId}-${index}`)?.scrollIntoView({ block: 'nearest' }) }, [open, index, listId])
  return <div className="mt-2 min-w-0" onBlur={e => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOpen(false) }}>
    <div className="relative"><span className="pointer-events-none absolute top-2 left-2"><InstitutionLogo institution={selected} name={query} /></span>
    <Input id={id} ref={input} className="pr-8 pl-9" title={selectedName || undefined} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={open ? listId : undefined} aria-activedescendant={open ? `${listId}-${index}` : undefined} aria-describedby={`${listId}-help`} value={query} disabled={disabled || loading || error} placeholder={translate("Search institutions")} autoComplete="off"
      onClick={() => { setOpen(true); setActive(0) }}
      onChange={e => { setQuery(e.target.value); setOpen(true); setActive(1); onUnresolvedChange(normalize(e.target.value) !== normalize(selectedName)) }}
      onKeyDown={e => {
        if (e.nativeEvent.isComposing) return
        if (e.key === 'Escape' && open) { e.preventDefault(); e.stopPropagation(); setOpen(false) }
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); setOpen(true); setActive(n => open ? (n + (e.key === 'ArrowDown' ? 1 : -1) + choices.length) % choices.length : 0) }
        if (e.key === 'Enter' && open) { e.preventDefault(); choose(index) }
      }} /><ChevronDown className="pointer-events-none absolute top-2.5 right-2 size-4 text-muted" aria-hidden="true" /></div>
    {open && <div id={listId} role="listbox" aria-label={translate("Institutions")} className="mt-1 max-h-48 overflow-y-auto rounded-md border border-line bg-card p-1 shadow-sm">
      {choices.map((choice, n) => <div key={choice.id} id={`${listId}-${n}`} role="option" aria-disabled={disabled} aria-selected={n === index} className={`flex cursor-pointer items-center gap-2 rounded px-2 py-2 text-sm ${n === index ? 'bg-soft' : ''}`} onMouseDown={e => e.preventDefault()} onMouseMove={() => setActive(n)} onClick={() => choose(n)}>
        <InstitutionLogo institution={institutions.find(i => i.id === choice.id)} name={choice.name} /><span className="min-w-0 break-words">{choice.create ? translate("Add “{value0}”", { value0: choice.name }) : choice.name}</span>
      </div>)}
    </div>}
    <p id={`${listId}-help`} className="mt-1 text-xs">{loading ? translate("Loading institutions…") : error ? translate("Could not load institutions.") : exact?.is_archived && exact.id !== value ? translate("Restore this institution in Settings to select it.") : [...name].length > 100 ? translate("Use at most 100 characters.") : draftName ? translate("New institution will be added when you save this account.") : translate("Search by name, abbreviation, bank code, or SWIFT. Choose Add for a new institution.")}</p>
    {error && <Button type="button" variant="outline" size="xs" onClick={reload}>{translate("Retry institutions")}</Button>}
  </div>
}
