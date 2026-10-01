import { t as translate, useLanguage } from "../lib/i18n"
import { ChevronDown } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import type { Account, AccountType } from '../lib/desktop'
import { useInstitutions } from './InstitutionProvider'
import { InstitutionLogo } from './InstitutionLogo'
import { Input } from './ui/input'
import { Popover, PopoverAnchor, PopoverContent } from './ui/popover'

const groups: { type: AccountType; label: string }[] = [
  { type: 'cash', get label() { return translate("Cash") } },
  { type: 'bank', get label() { return translate("Bank") } },
  { type: 'wallet', get label() { return translate("Wallets") } },
  { type: 'credit_card', get label() { return translate("Credit cards") } },
  { type: 'loan', get label() { return translate("Loans") } },
  { type: 'investment', get label() { return translate("Investments") } },
]
const normalize = (value: string) => value.trim().replace(/\s+/gu, ' ').toLowerCase()

export function AccountCombobox({ id, value, accounts, disabled, onChange }: {
  id?: string
  value: string
  accounts: Account[]
  disabled?: boolean
  onChange: (value: string) => void
}) {
  useLanguage()

  const directory = useInstitutions()
  const listId = useId()
  const input = useRef<HTMLInputElement>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const selected = accounts.find(account => account.id === value)
  const matches = groups.map(group => ({ ...group, accounts: accounts.filter(account =>
    account.type === group.type && normalize(`${account.name} ${group.label}`).includes(normalize(query)),
  ) })).filter(group => group.accounts.length > 0)
  const choices = matches.flatMap(group => group.accounts)
  const index = Math.min(active, choices.length - 1)
  function institution(accountId: string) {
    const account = directory.accounts.find(account => account.id === accountId)
    return directory.institutions.find(item => item.id === account?.institution_id)
  }
  function show() { setQuery(''); setActive(0); setOpen(true) }
  function choose(account: Account) {
    if (disabled) return
    onChange(account.id); setOpen(false); input.current?.focus()
  }
  useEffect(() => {
    input.current?.setCustomValidity(selected ? '' : translate("Choose an account from the list."))
  }, [selected])
  useEffect(() => {
    if (!open || index < 0) return
    const list = document.getElementById(listId)
    const option = document.getElementById(`${listId}-${choices[index].id}`)
    if (!list || !option) return
    const listBounds = list.getBoundingClientRect()
    const optionBounds = option.getBoundingClientRect()
    if (optionBounds.top < listBounds.top) list.scrollTop -= listBounds.top - optionBounds.top
    else if (optionBounds.bottom > listBounds.bottom) list.scrollTop += optionBounds.bottom - listBounds.bottom
  }, [open, index, listId, choices[index]?.id])
  return <Popover open={open} onOpenChange={setOpen}><div className="mt-2 min-w-0" onBlur={event => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false)
  }}>
    <PopoverAnchor asChild><div className="relative">
      <span className="pointer-events-none absolute top-2 left-2"><InstitutionLogo institution={institution(value)} name={selected?.name ?? ''} /></span>
      <Input id={id} ref={input} className="pr-8 pl-9" role="combobox" aria-autocomplete="list" aria-expanded={open}
        aria-controls={open ? listId : undefined} aria-activedescendant={open && index >= 0 ? `${listId}-${choices[index].id}` : undefined}
        aria-required="true" disabled={disabled} autoComplete="off" placeholder={translate("Search accounts")} title={selected?.name}
        value={open ? query : selected?.name ?? ''}
        onClick={() => { if (!open) show() }}
        onChange={event => { setQuery(event.target.value); setActive(0); setOpen(true) }}
        onKeyDown={event => {
          if (event.nativeEvent.isComposing) return
          if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); setOpen(false) }
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            if (!open) show()
            else if (choices.length) setActive((index + (event.key === 'ArrowDown' ? 1 : -1) + choices.length) % choices.length)
          }
          if (event.key === 'Enter' && open) { event.preventDefault(); if (choices[index]) choose(choices[index]) }
        }} />
      <ChevronDown className="pointer-events-none absolute top-2.5 right-2 size-4 text-muted" aria-hidden="true" />
    </div></PopoverAnchor>
    <PopoverContent id={listId} role="listbox" aria-label={translate("Accounts")}
      className="max-h-[min(13rem,var(--radix-popover-content-available-height))] w-[var(--radix-popover-trigger-width)] overflow-y-auto overscroll-contain p-1"
      onOpenAutoFocus={event => event.preventDefault()}
      onCloseAutoFocus={event => event.preventDefault()}
      onInteractOutside={event => { if (event.target === input.current) event.preventDefault() }}
      onEscapeKeyDown={event => { event.preventDefault(); setOpen(false) }}>
      {matches.map(group => <div key={group.type} role="group" aria-labelledby={`${listId}-group-${group.type}`}>
        <div id={`${listId}-group-${group.type}`} className="px-2 py-2 text-xs font-semibold text-muted">{group.label}</div>
        {group.accounts.map(account => <div key={account.id} id={`${listId}-${account.id}`} role="option" aria-selected={account.id === value}
          className={`flex cursor-pointer items-center gap-2 rounded px-2 py-2 text-sm ${choices[index]?.id === account.id ? 'bg-soft text-ink' : ''}`}
          onMouseDown={event => event.preventDefault()} onMouseMove={() => setActive(choices.findIndex(choice => choice.id === account.id))} onClick={() => choose(account)}>
          <InstitutionLogo institution={institution(account.id)} name={account.name} /><span className="min-w-0 break-words">{account.name}</span>
        </div>)}
      </div>)}
      {!choices.length && <p role="status" className="px-2 py-3 text-sm text-muted">{translate("No matching accounts.")}</p>}
    </PopoverContent>
  </div></Popover>
}
