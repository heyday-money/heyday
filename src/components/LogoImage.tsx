import { t as translate, useLanguage } from "../lib/i18n"
import { useEffect, useState } from 'react'
import { getLogoAsset } from '../lib/logos'
import { cn } from '../lib/utils'
export function useLogoAsset(id?: string | null) {
  const [loaded, setLoaded] = useState<{ id: string; src: string } | null>(null)
  useEffect(() => {
    let active = true
    if (id) void getLogoAsset(id).then(src => { if (active) setLoaded({ id, src }) }).catch(() => { if (active) setLoaded(null) })
    return () => { active = false }
  }, [id])
  return loaded && loaded.id === id ? loaded.src : null
}
export function LogoImage({ src, name, className = '' }: { src?: string | null; name: string; className?: string }) {
  useLanguage()

  const [failed, setFailed] = useState<string | null>(null)
  const hasLogo = !!src && failed !== src
  return <span aria-hidden="true" className={cn('inline-flex size-5 shrink-0 items-center justify-center overflow-hidden rounded text-[11px] font-semibold text-brand', !hasLogo && 'bg-soft', className)}>
    {hasLogo ? <img src={src} alt="" className="size-full object-contain" onError={() => setFailed(src)} /> : Array.from(name.trim())[0]?.toLocaleUpperCase() || '?'}
  </span>
}
