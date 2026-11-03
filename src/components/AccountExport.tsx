import { useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { FileSpreadsheet } from 'lucide-react'
import { desktopAvailable } from '../lib/desktop'
import { t, useLanguage } from '../lib/i18n'
import { Button } from './ui/button'

export function AccountExport({ disabled }: { disabled: boolean }) {
  useLanguage()
  const lock = useRef(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [path, setPath] = useState<string | null>(null)
  async function exportAccounts() {
    if (lock.current) return
    lock.current = true
    setBusy(true); setError(null); setPath(null)
    try {
      const saved = await invoke<string | null>('export_accounts_excel')
      if (saved) { setPath(saved); toast.success(t('Accounts exported.')) }
    } catch (error) {
      setError(typeof error === 'string' ? error : t('Could not export accounts. Please try again.'))
    } finally { lock.current = false; setBusy(false) }
  }
  return <section className="rounded-2xl border border-line bg-card p-6" aria-labelledby="account-export-title">
    <h3 id="account-export-title" className="text-lg font-semibold">{t('Export accounts')}</h3>
    <p className="mt-2 text-sm text-muted">{t('Download all active, archived, and paid-off accounts as an Excel (.xlsx) file. Includes balances and account details, without transaction history. Amounts are exported as exact text to preserve precision.')}</p>
    <p className="mt-2 text-sm text-muted">{t('Excel exports are for your records and cannot be restored into the app. Use Backup & restore for a complete backup.')}</p>
    <Button className="mt-4" variant="outline" disabled={!desktopAvailable || disabled || busy} onClick={() => void exportAccounts()}><FileSpreadsheet aria-hidden="true" />{busy ? t('Exporting…') : t('Export accounts to Excel')}</Button>
    {path && <p className="mt-3 break-all text-xs text-muted">{t('Export saved at:')} {path}</p>}
    {error && <p role="alert" className="mt-3 text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
  </section>
}
