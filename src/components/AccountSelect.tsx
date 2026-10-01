import { t as translate, useLanguage } from "../lib/i18n"
import { useEffect, useRef, useState, type ComponentProps } from 'react'
import { NativeSelect } from './ui/native-select'
import { useInstitutions } from './InstitutionProvider'
import { InstitutionLogo } from './InstitutionLogo'
export function AccountSelect({ value, defaultValue, onChange, children, className, accountIds, ...props }: ComponentProps<typeof NativeSelect> & { accountIds?: Record<string, string> }) {
  useLanguage()

  const directory = useInstitutions()
  const select = useRef<HTMLSelectElement>(null)
  const [selected, setSelected] = useState(String(defaultValue ?? ''))
  useEffect(() => { if (value === undefined) setSelected(select.current?.value ?? '') }, [children, defaultValue, value])
  const selectedValue = String(value ?? selected)
  const id = accountIds?.[selectedValue] ?? selectedValue
  const account = directory.accounts.find(a => a.id === id)
  const institution = directory.institutions.find(i => i.id === account?.institution_id)
  return <div className="flex min-w-0 items-center gap-2"><span className="mb-2 shrink-0 self-end" title={institution?.name}><InstitutionLogo institution={institution} name={account?.name ?? ''} /></span><NativeSelect ref={select} {...props} className={className} value={value} defaultValue={defaultValue} onChange={e => { setSelected(e.target.value); onChange?.(e) }}>{children}</NativeSelect></div>
}
