import { useSyncExternalStore } from 'react'
import { thai } from './locales/th'

export type Language = 'en' | 'th'
const preferenceKey = 'heyday-language'
function readLanguage(): Language {
  if (typeof window === 'undefined') return 'en'
  try { return localStorage.getItem(preferenceKey) === 'th' ? 'th' : 'en' } catch { return 'en' }
}
let language: Language = readLanguage()
const listeners = new Set<() => void>()
export function getLanguage(): Language { return language }
export function getLocale(): string { return language === 'th' ? 'th-TH-u-ca-gregory-nu-latn' : 'en-GB' }
function subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } }
export function useLanguage(): Language { return useSyncExternalStore(subscribe, getLanguage, () => 'en') }
export function setLanguage(next: Language) {
  if (next !== 'en' && next !== 'th') return
  language = next
  try { if (typeof window !== 'undefined') localStorage.setItem(preferenceKey, next) } catch { /* In-memory preference still works. */ }
  if (typeof document !== 'undefined') document.documentElement.lang = next
  listeners.forEach(listener => listener())
}
if (typeof document !== 'undefined') document.documentElement.lang = language
export function t(english: string, values: Record<string, unknown> = {}): string {
  let text = language === 'th' && Object.prototype.hasOwnProperty.call(thai, english) ? thai[english] : english
  if (language === 'th' && text === english) {
    const amountError = /^Enter a valid amount with up to (\d+) decimal places\.$/.exec(english)
    if (amountError) return t('Enter a valid amount with up to {digits} decimal places.', { digits: amountError[1] })
  }
  return text.replace(/\{(\w+)\}/g, (match, name: string) => Object.prototype.hasOwnProperty.call(values, name) ? String(values[name]) : match)
}

if (typeof window !== 'undefined') {
  window.addEventListener('storage', event => {
    if (event.key === preferenceKey || event.key === null) {
      language = readLanguage()
      document.documentElement.lang = language
      listeners.forEach(listener => listener())
    }
  })
}
