import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { desktopAvailable } from './desktop'
import type { LogoChange } from './logos'
export interface SubscriptionProvider {
  id: string; name: string; is_archived: boolean; builtin_icon: string | null
  logo_mode: 'default' | 'custom' | 'none'; logo_asset_id: string | null
}
export const listSubscriptionProviders = () => invoke<SubscriptionProvider[]>('list_subscription_providers')
export async function saveSubscriptionProvider(input: { id: string | null; name: string; is_archived: boolean; logo_change?: LogoChange }) {
  await invoke('save_subscription_provider', { input })
  window.dispatchEvent(new Event('subscription-providers-changed'))
  window.dispatchEvent(new Event('plans-changed'))
}
const icons = new Set(Object.keys(import.meta.glob('/public/subscription-providers/*.svg')).map(path => path.split('/').pop()?.replace('.svg', '')))
export function providerIcon(key?: string | null) { return key && icons.has(key) ? `/subscription-providers/${key}.svg` : null }
export function useSubscriptionProviders() {
  const [providers, setProviders] = useState<SubscriptionProvider[]>([])
  const [loading, setLoading] = useState(desktopAvailable)
  const [error, setError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const reload = useCallback(() => setAttempt(value => value + 1), [])
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setError(false)
    listSubscriptionProviders().then(rows => { if (active) setProviders(rows) }).catch(() => { if (active) setError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  useEffect(() => { window.addEventListener('subscription-providers-changed', reload); return () => window.removeEventListener('subscription-providers-changed', reload) }, [reload])
  return { providers, loading, error, reload }
}
