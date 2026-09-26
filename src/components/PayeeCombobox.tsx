import { useEffect, useId, useRef, useState } from 'react'
import { toast } from 'sonner'
import { saveTransactionOption, type TransactionOption } from '../lib/desktop'
import { Input } from './ui/input'

const normalize = (name: string) => name.trim().replace(/\s+/gu, ' ')

export function PayeeCombobox({ id, value, options, onChange, onCreated, onBusyChange, onUnresolvedChange }: {
  id?: string
  value: string
  options: TransactionOption[]
  onChange: (id: string) => void
  onCreated: (option: TransactionOption) => void
  onBusyChange: (busy: boolean) => void
  onUnresolvedChange: (unresolved: boolean) => void
}) {
  const listId = useId()
  const inputRef = useRef<HTMLInputElement>(null)
  const busyRef = useRef(false)
  const [query, setQuery] = useState('')
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const selectedName = options.find(option => option.id === value)?.name ?? ''
  useEffect(() => { setQuery(selectedName); setError(null) }, [value, selectedName])
  const name = normalize(query)
  const key = name.toLowerCase()
  const exact = options.find(option => normalize(option.name).toLowerCase() === key)
  const matches = options.filter(option => !option.is_archived && normalize(option.name).toLowerCase().includes(key))
  const tooLong = [...name].length > 100
  const choices = [
    { id: '', label: 'No payee', create: false },
    ...matches.map(option => ({ id: option.id, label: option.name, create: false })),
    ...(name && !exact && !tooLong ? [{ id: 'create', label: `Create “${name}”`, create: true }] : []),
  ]
  const activeIndex = Math.min(active, choices.length - 1)
  async function choose(index: number) {
    if (busyRef.current) return
    const choice = choices[index]
    if (!choice) return
    setError(null)
    if (!choice.create) {
      onChange(choice.id); onUnresolvedChange(false)
      setQuery(choice.id ? choice.label : ''); setOpen(false)
      inputRef.current?.focus()
      return
    }
    busyRef.current = true; setBusy(true); onBusyChange(true)
    try {
      const option = await saveTransactionOption({ kind: 'payee', id: null, name, is_archived: false })
      onCreated(option); onChange(option.id); onUnresolvedChange(false)
      setQuery(option.name); setOpen(false)
      toast.success('Payee added.')
    } catch (error) {
      setError(error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not create payee. Please try again.')
    } finally {
      busyRef.current = false; setBusy(false); onBusyChange(false)
      requestAnimationFrame(() => inputRef.current?.focus())
    }
  }
  useEffect(() => {
    if (open) document.getElementById(`${listId}-${activeIndex}`)?.scrollIntoView({ block: 'nearest' })
  }, [open, activeIndex, listId])
  return <div className="mt-2 min-w-0" onBlur={event => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null) && !busyRef.current) setOpen(false)
  }}>
    <Input id={id} ref={inputRef} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={open ? listId : undefined} aria-activedescendant={open ? `${listId}-${activeIndex}` : undefined} aria-describedby={`${listId}-help`} autoComplete="off" placeholder="Search or create a payee" value={query} readOnly={busy}
      onClick={() => setOpen(true)}
      onChange={event => {
        setQuery(event.target.value); setOpen(true); setActive(1); setError(null)
        onUnresolvedChange(normalize(event.target.value) !== normalize(selectedName))
      }}
      onKeyDown={event => {
        if (event.nativeEvent.isComposing) return
        if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); setOpen(false) }
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault(); setOpen(true)
          setActive(current => open ? (current + (event.key === 'ArrowDown' ? 1 : -1) + choices.length) % choices.length : 0)
        }
        if (event.key === 'Enter' && open) { event.preventDefault(); void choose(activeIndex) }
      }} />
    {open && <div id={listId} role="listbox" aria-label="Payees" aria-busy={busy} className="mt-1 max-h-44 overflow-y-auto rounded-md border border-line bg-card p-1 shadow-sm">
      {choices.map((choice, index) => <div key={choice.id} id={`${listId}-${index}`} role="option" aria-selected={index === activeIndex} aria-disabled={busy} className={`cursor-pointer break-words rounded-sm px-3 py-2 text-sm ${index === activeIndex ? 'bg-soft text-ink' : ''}`} onMouseDown={event => event.preventDefault()} onMouseMove={() => setActive(index)} onClick={() => void choose(index)}>{choice.label}</div>)}
    </div>}
    <p id={`${listId}-help`} className="mt-1 text-xs text-muted">{busy ? 'Creating payee…' : exact?.is_archived ? 'This payee is archived. Restore it in Settings to use it.' : tooLong ? 'Use a name of at most 100 characters.' : 'Select a payee or choose Create. New payees are saved immediately.'}</p>
    {error && <p role="alert" className="mt-1 text-sm text-red-700 dark:text-red-400">{error}</p>}
  </div>
}
