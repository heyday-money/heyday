import { t as translate, useLanguage } from "../lib/i18n"
import { useEffect, useId, useRef, useState } from 'react'
import { prepareLogo, type LogoChange } from '../lib/logos'
import { LogoImage } from './LogoImage'
import { Button } from './ui/button'
export function LogoPicker({ name, value, savedSource, defaultSource, hasSavedLogo, disabled, onChange, onBusyChange }: {
  name: string; value?: LogoChange; savedSource?: string | null; defaultSource?: string | null; hasSavedLogo?: boolean; disabled?: boolean
  onChange: (change: LogoChange) => void; onBusyChange: (busy: boolean) => void
}) {
  useLanguage()

  const input = useRef<HTMLInputElement>(null)
  const version = useRef(0)
  const id = useId()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => () => { version.current++ }, [])
  const src = value?.kind === 'custom' ? value.data : value?.kind === 'default' ? defaultSource : value?.kind === 'none' ? null : savedSource
  async function choose(file: File) {
    const request = ++version.current
    setBusy(true); onBusyChange(true); setError(null)
    try {
      const data = await prepareLogo(file)
      if (request === version.current) onChange({ kind: 'custom', data })
    } catch (e) { if (request === version.current) setError(e instanceof Error ? e.message : "Could not read this image.") }
    finally { if (request === version.current) { setBusy(false); onBusyChange(false) } }
  }
  return <fieldset className="space-y-2" disabled={disabled || busy}>
    <legend className="mb-2 text-sm font-medium">{translate("Logo (optional)")}</legend>
    <div className="flex flex-wrap items-center gap-3"><LogoImage src={src} name={name} className="size-12 text-lg" />
      <input ref={input} id={id} className="sr-only" type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml,.svg" aria-label={translate("Choose logo file")} onChange={e => { const file = e.currentTarget.files?.[0]; e.currentTarget.value = ''; if (file) void choose(file) }} />
      <Button type="button" variant="outline" onClick={() => input.current?.click()}>{busy ? translate("Preparing…") : src ? translate("Replace logo") : translate("Choose logo")}</Button>
      {(src || savedSource || hasSavedLogo || value?.kind === 'custom') && <Button type="button" variant="ghost" onClick={() => { setError(null); onChange({ kind: 'none' }) }}>{translate("Remove logo")}</Button>}
      {defaultSource && src !== defaultSource && <Button type="button" variant="ghost" onClick={() => { setError(null); onChange({ kind: 'default' }) }}>{translate("Use default logo")}</Button>}
    </div>
    <p className="text-xs">{translate("PNG, JPG, WebP, or SVG, up to 5 MB. Resized to fit 128 × 128 pixels. Stored only on this device when you save.")}</p>
    {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{translate(error)}</p>}
  </fieldset>
}
