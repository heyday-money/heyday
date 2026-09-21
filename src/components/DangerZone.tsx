import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { clearAllData, desktopAvailable } from '../lib/desktop'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

const confirmationText = 'DELETE ALL DATA'
const resetNotice = 'heyday-data-reset'

export function DangerZone({ disabled = false }: { disabled?: boolean }) {
  const [open, setOpen] = useState(false)
  const [confirmation, setConfirmation] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    try {
      if (sessionStorage.getItem(resetNotice)) {
        sessionStorage.removeItem(resetNotice)
        toast.success('All data cleared. Choose your currency to start again.')
      }
    } catch { /* Storage preferences are optional. */ }
  }, [])

  async function reset(event: React.FormEvent) {
    event.preventDefault()
    if (busy || confirmation !== confirmationText) return
    setBusy(true)
    setError(null)
    try {
      await clearAllData(confirmation)
    } catch {
      setError('Could not clear your data. No changes were saved. Please try again.')
      setBusy(false)
      return
    }
    try { sessionStorage.setItem(resetNotice, 'true') } catch { /* Reload still clears stale data. */ }
    // Reload all mounted views and discard drafts referencing deleted records.
    window.location.reload()
  }

  return <section className="mt-6 rounded-[22px] border border-red-500/40 bg-card p-[27px]" aria-labelledby="danger-zone-title">
    <h3 id="danger-zone-title" className="text-lg font-semibold text-red-600 dark:text-red-400">Danger Zone</h3>
    <p className="mt-2 text-sm text-muted">Permanently delete all financial data and reset currency and payday settings. This cannot be undone.</p>
    <Button className="mt-4" variant="destructive" disabled={!desktopAvailable || disabled} onClick={() => { setConfirmation(''); setError(null); setOpen(true) }}>Clear all data</Button>
    <Dialog open={open} onOpenChange={(value) => { if (!busy) setOpen(value) }}>
      <DialogContent showCloseButton={!busy} onInteractOutside={(event) => event.preventDefault()} onEscapeKeyDown={(event) => { if (busy) event.preventDefault() }}>
        <DialogHeader>
          <DialogTitle>Clear all data?</DialogTitle>
          <DialogDescription>This permanently deletes all accounts and balances, transactions, income and deductions, installments, subscriptions, payment plans, Outlook entries, payees, and spending categories. Currency and payday settings will reset. Built-in Outlook sections and appearance preferences remain. The app will reload and discard open drafts. This cannot be undone.</DialogDescription>
        </DialogHeader>
        <form onSubmit={reset} className="space-y-4">
          <div className="space-y-2">
            <label htmlFor="reset-confirmation" className="text-sm font-medium">Type {confirmationText} to confirm</label>
            <Input id="reset-confirmation" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} disabled={busy} autoComplete="off" spellCheck={false} />
          </div>
          {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{error}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={busy} onClick={() => setOpen(false)}>Cancel</Button>
            <Button type="submit" variant="destructive" disabled={busy || confirmation !== confirmationText}>{busy ? 'Clearing…' : 'Permanently clear all data'}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </section>
}
