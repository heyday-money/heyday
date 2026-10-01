import { t as translate, useLanguage } from "../lib/i18n"
import { cloneElement, useId, type ReactElement } from 'react'
import { Label } from './ui/label'

export function FormField({ label, children }: { label: string; children: ReactElement<{ id?: string }> }) {
  useLanguage()

  const id = useId()
  return <div><Label htmlFor={id} className="block text-[13px] font-medium">{translate(label)}</Label>{cloneElement(children, { id })}</div>
}
