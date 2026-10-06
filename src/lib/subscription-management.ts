import { t } from './i18n'

export const subscriptionPlatforms = [
  { value: 'apple_app_store', get label() { return t('Apple App Store') } },
  { value: 'google_play', get label() { return t('Google Play') } },
  { value: 'website', get label() { return t('Service website') } },
  { value: 'in_app', get label() { return t('In-app') } },
  { value: 'other', get label() { return t('Other') } },
] as const

export function validateManagementUrl(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  const message = 'Enter an HTTPS management link without login credentials (up to 2048 characters).'
  let url: URL
  try { url = new URL(trimmed) } catch { throw new Error(message) }
  if (trimmed.length > 2048 || /[\u0000-\u001f\u007f]/.test(trimmed) || url.protocol !== 'https:' || !url.hostname || url.username || url.password || url.href.length > 2048) throw new Error(message)
  return url.href
}
