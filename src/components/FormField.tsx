import { cloneElement, useId, type ReactElement } from 'react'
import { Label } from './ui/label'

export function FormField({ label, children }: { label: string; children: ReactElement<{ id?: string }> }) {
  const id = useId()
  return <div><Label htmlFor={id} className="block text-[13px] font-medium">{label}</Label>{cloneElement(children, { id })}</div>
}
