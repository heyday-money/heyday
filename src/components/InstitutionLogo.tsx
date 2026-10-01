import { LogoImage, useLogoAsset } from './LogoImage'
import type { Institution } from '../lib/institutions'
import { useInstitutions } from './InstitutionProvider'
const logos = new Set(Object.keys(import.meta.glob('/public/institutions/*.svg')).map(path => path.split('/').pop()))
export function institutionDefaultLogo(institution?: Institution) {
  return institution?.logo && logos.has(institution.logo) ? `/institutions/${institution.logo}` : null
}
export function useInstitutionLogo(institution?: Institution) {
  const custom = useLogoAsset(institution?.logo_asset_id)
  return institution?.logo_mode === 'none' ? null : institution?.logo_mode === 'custom' ? custom : institutionDefaultLogo(institution)
}
export function InstitutionLogo({ institution, name = '', className = '' }: { institution?: Institution; name?: string; className?: string }) {
  const src = useInstitutionLogo(institution)
  return <LogoImage src={src} name={institution?.name || name} className={className} />
}
export function AccountLabel({ id, name, compact = false }: { id?: string | null; name: string; compact?: boolean }) {
  const directory = useInstitutions()
  const account = directory.accounts.find(a => a.id === id)
  const institution = directory.institutions.find(i => i.id === account?.institution_id)
  const label = account?.name ?? name
  return <span className={`inline-flex min-w-0 max-w-full items-center gap-1.5 align-middle ${compact ? 'w-full' : ''}`} title={institution ? `${label} · ${institution.name}` : label}>
    <InstitutionLogo institution={institution} name={label} />
    <span className={compact ? 'min-w-0 truncate' : 'min-w-0 break-words'}>{label}{institution && !compact && <span className="block text-[10px] font-normal text-muted">{institution.name}</span>}</span>
  </span>
}
