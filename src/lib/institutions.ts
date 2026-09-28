import type { LogoChange } from './logos'
import { invoke } from '@tauri-apps/api/core'
export interface Institution {
  id: string; name: string; short_name: string | null; bank_code: string | null
  swift_code: string | null; logo: string | null; logo_asset_id?: string | null; logo_mode?: 'default' | 'custom' | 'none'; is_archived: boolean
}
export interface AccountIdentity { id: string; name: string; institution_id: string | null }
export interface InstitutionDirectory { institutions: Institution[]; accounts: AccountIdentity[] }
export const listInstitutions = () => invoke<InstitutionDirectory>('list_institutions')
export async function deleteInstitution(id: string) {
  await invoke('delete_institution', { id })
  window.dispatchEvent(new Event('institutions-changed'))
  window.dispatchEvent(new Event('accounts-changed'))
}
export async function saveInstitution(input: { logo_change?: LogoChange; id: string | null; name: string; is_archived: boolean }) {
  const result = await invoke<Institution>('save_institution', { input })
  window.dispatchEvent(new Event('institutions-changed'))
  window.dispatchEvent(new Event('accounts-changed'))
  return result
}
