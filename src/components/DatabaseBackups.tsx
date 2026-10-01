import { t as translate, useLanguage } from "../lib/i18n"
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { desktopAvailable } from '../lib/desktop'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

type BackupPreview = { token: string; filename: string; schema_version: number; accounts: number; transactions: number; currency: string | null }
const confirmationText = 'RESTORE BACKUP'
const restoreNotice = 'heyday-backup-restored'
const message = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : "Could not complete the backup operation. Please try again."

export function DatabaseBackups({ disabled = false }: { disabled?: boolean }) {
  useLanguage()

  const [preview, setPreview] = useState<BackupPreview | null>(null)
  const [confirmation, setConfirmation] = useState('')
  const [busy, setBusy] = useState<'export' | 'preview' | 'restore' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [savedPath, setSavedPath] = useState<string | null>(null)
  const lock = useRef(false)
  useEffect(() => {
    try {
      const recovery = sessionStorage.getItem(restoreNotice)
      if (recovery) {
        sessionStorage.removeItem(restoreNotice)
        setSavedPath(recovery)
        toast.success(translate("Backup restored. Your previous data is saved in a recovery backup."))
      }
    } catch { /* Reload still refreshes all restored records. */ }
  }, [])
  async function select(kind: 'export' | 'preview') {
    if (lock.current) return
    lock.current = true; setBusy(kind); setError(null)
    try {
      if (kind === 'export') {
        const path = await invoke<string | null>('export_backup')
        if (path) { setSavedPath(path); toast.success(translate("Backup exported.")) }
      } else {
        const result = await invoke<BackupPreview | null>('preview_backup')
        setConfirmation(''); setPreview(result)
      }
    } catch (error) { setError(message(error)) }
    finally { lock.current = false; setBusy(null) }
  }
  async function cancel() {
    if (lock.current) return
    lock.current = true
    try { await invoke('cancel_backup_restore'); setPreview(null); setConfirmation(''); setError(null) }
    catch (error) { setError(message(error)) }
    finally { lock.current = false }
  }
  async function restore(event: FormEvent) {
    event.preventDefault()
    if (!preview || confirmation !== confirmationText || lock.current) return
    lock.current = true; setBusy('restore'); setError(null)
    try {
      const result = await invoke<{ recovery_path: string }>('restore_backup', { token: preview.token, confirmation })
      try { sessionStorage.setItem(restoreNotice, result.recovery_path) } catch { /* Reload remains mandatory. */ }
      window.location.reload()
    } catch (error) {
      setError(message(error)); lock.current = false; setBusy(null)
    }
  }
  return <section className="mt-6 rounded-2xl border border-line bg-card p-6" aria-labelledby="backups-title">
    <h3 id="backups-title" className="text-lg font-semibold">{translate("Backup & restore")}</h3>
    <p className="mt-2 text-sm text-muted">{translate("Save a local backup of all financial records, settings, and custom logos. Keep backup files private; they contain your financial data and are not encrypted. Appearance is a separate preference on this device.")}</p>
    <div className="mt-4 flex flex-wrap gap-2">
      <Button variant="outline" disabled={!desktopAvailable || disabled || !!busy || !!preview} onClick={() => void select('export')}>{busy === 'export' ? translate("Exporting…") : translate("Export backup")}</Button>
      <Button variant="outline" disabled={!desktopAvailable || disabled || !!busy || !!preview} onClick={() => void select('preview')}>{busy === 'preview' ? translate("Checking backup…") : translate("Restore backup")}</Button>
    </div>
    {savedPath && <p className="mt-3 break-all text-xs text-muted">{translate("Backup saved at:")}{" "}{savedPath}</p>}
    {error && !preview && <p role="alert" className="mt-3 text-sm text-red-600 dark:text-red-400">{translate(error)}</p>}
    <Dialog open={!!preview} onOpenChange={open => { if (!open && !busy) void cancel() }}>
      <DialogContent showCloseButton={!busy} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy) event.preventDefault() }}>
        <DialogHeader><DialogTitle>{translate("Restore backup?")}</DialogTitle><DialogDescription>{translate("This replaces all current financial records and database settings. A recovery backup of your current data will be saved first. The app will reload and discard open drafts. Appearance and existing backup files remain.")}</DialogDescription></DialogHeader>
        {preview && <dl className="space-y-2 rounded-lg bg-soft p-3 text-sm">
          <div><dt className="text-muted">{translate("Selected backup")}</dt><dd className="break-all font-medium">{preview.filename}</dd></div>
          <div><dt className="text-muted">{translate("Contents")}</dt><dd>{preview.accounts} {" "}{translate("accounts ·")}{" "}{preview.transactions} {" "}{translate("transactions ·")}{" "}{preview.currency ?? translate("Currency not selected")}</dd></div>
        </dl>}
        <form onSubmit={restore} className="space-y-4">
          <div className="space-y-2"><label htmlFor="restore-confirmation" className="text-sm font-medium">{translate("Type")}{" "}{confirmationText} {" "}{translate("to confirm")}</label><Input id="restore-confirmation" value={confirmation} onChange={event => setConfirmation(event.target.value)} disabled={!!busy} autoComplete="off" spellCheck={false}/></div>
          {busy === 'restore' && <p role="status" className="text-sm">{translate("Saving recovery backup and restoring…")}</p>}
          {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{translate(error)}</p>}
          <DialogFooter><Button type="button" variant="outline" disabled={!!busy} onClick={() => void cancel()}>{translate("Cancel")}</Button><Button type="submit" variant="destructive" disabled={!!busy || confirmation !== confirmationText}>{busy === 'restore' ? translate("Restoring…") : translate("Replace data and restore")}</Button></DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </section>
}
