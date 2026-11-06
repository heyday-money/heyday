import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { toast } from 'sonner'
import { cloudEnableSync, cloudPauseSync, cloudResolveSync, cloudSyncNow, cloudSyncStatus, type CloudSyncStatus, type SyncConflict } from '../lib/desktop'
import { t } from '../lib/i18n'
import { useCloudSession } from './CloudSessionProvider'

interface SyncContext {
  status: CloudSyncStatus | null; busy: boolean; error: string
  verified: boolean; hasLocalChanges: boolean
  act: (action: 'enable' | 'pause' | 'now' | 'status', automatic?: boolean) => Promise<boolean>
  resolve: (conflict: SyncConflict, choice: 'local' | 'remote' | 'rename', name?: string) => Promise<boolean>
}
const Context = createContext<SyncContext | null>(null)
const needsAction = new Set([
  'Encryption changed on another device. Restore with the latest recovery key.',
  'Verify cloud history with a code from a trusted device before syncing.',
  'Finish or cancel the pending recovery-key replacement before syncing.',
  'Cloud history is too large to verify in this app version. Update the app before syncing.',
  'Enable complete financial sync to download this cloud data.',
  'Enable income forecast sync to download this cloud data.',
  'Enable income and payroll sync to download this cloud data.',
  'Enable account and transaction sync to download this cloud data.',
  'Account balances do not match transaction history. Review your local data before enabling financial sync.',
  'Sync access has ended or has not been granted. Your cloud data is retained.',
  'Set up or unlock encryption before syncing.',
  'Register this device before syncing.',
  'Register this device in Manage devices before setting up encryption.',
  'Cloud sync is disabled on this service.',
  'Cloud history could not be verified. Sync stopped to protect your data.',
  'The saved encryption package does not match the server. Access was blocked to protect your data.',
  'This cloud data needs a newer app. Update before syncing.',
  'This local database is linked to another cloud account. Sign in to its original account to sync.',
])
export function CloudSyncProvider({ children }: { children: ReactNode }) {
  const { status: session, syncAccess, setStatus: setSession, refresh } = useCloudSession()
  const [status, setStatus] = useState<CloudSyncStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [verified, setVerified] = useState(false)
  const [hasLocalChanges, setHasLocalChanges] = useState(false)
  const editRevision = useRef(0)
  const owner = useRef(session.account?.id); owner.current = session.account?.id
  const saved = useRef(status); saved.current = status
  const inFlight = useRef(false), nextTry = useRef(0), failures = useRef(0), rateLimit = useRef(0)
  const automaticPaused = useRef<string | null>(null)
  const access = useRef(syncAccess); access.current = syncAccess
  const execute = useCallback(async (operation: () => Promise<CloudSyncStatus>, automatic = false, network = false, verifiesSync = false) => {
    if (!owner.current || inFlight.current || (network && Date.now() < rateLimit.current) || (automatic && (automaticPaused.current || Date.now() < nextTry.current))) return false
    if (network && access.current === 'blocked') {
      setVerified(false)
      if (!automatic) setError('Sync access has ended or has not been granted. Your cloud data is retained.')
      return false
    }
    if (network && !automatic) automaticPaused.current = null
    const account = owner.current
    const revision = editRevision.current
    inFlight.current = true; setBusy(true)
    if (!automatic) setError('')
    try {
      const value = await operation()
      if (owner.current !== account) return false
      setStatus(value); setError(''); failures.current = 0; nextTry.current = 0
      // Reading cached status or enabling sync cannot confirm that changes reached
      // the cloud. Edits made during a request require another successful pass.
      if (verifiesSync && revision === editRevision.current) {
        setVerified(true); setHasLocalChanges(false)
      }
      if (network) for (const event of ['transaction-options-changed', 'accounts-changed', 'transactions-changed', 'incomes-changed', 'plans-changed', 'settings-changed', 'verification-changed', 'institutions-changed', 'subscription-providers-changed']) window.dispatchEvent(new CustomEvent(event, { detail: { source: 'cloud-sync' } }))
      return true
    } catch (error) {
      if (owner.current !== account) return false
      const message = typeof error === 'string' ? error : 'Could not sync. Check your connection and try again.'
      setError(message); setVerified(false)
      if (needsAction.has(message)) automaticPaused.current = message
      if (message === 'Sync access has ended or has not been granted. Your cloud data is retained.' || message === 'Cloud sync is disabled on this service.') void refresh()
      failures.current++
      nextTry.current = Date.now() + Math.min(300_000, 5000 * 2 ** Math.min(failures.current, 6)) + Math.random() * 1000
      if (message === 'Too many sync requests. Try again in a minute.') rateLimit.current = Date.now() + 60_000
      if (message === 'Your sign-in has expired or device access was revoked. Please sign in again.') {
        setSession({ account: null, online: false }); toast.error(t(message))
      } else {
        // Setup may have linked locally before a connection failed; keep its queue visible.
        try { const value = await cloudSyncStatus(); if (owner.current === account) setStatus(value) } catch { /* Keep the original error. */ }
      }
      if (network) for (const event of ['transaction-options-changed', 'accounts-changed', 'transactions-changed', 'incomes-changed', 'plans-changed', 'settings-changed', 'verification-changed', 'institutions-changed', 'subscription-providers-changed']) window.dispatchEvent(new CustomEvent(event, { detail: { source: 'cloud-sync' } }))
      return false
    } finally { inFlight.current = false; setBusy(false) }
  }, [setSession, refresh])
  const act = useCallback((action: 'enable' | 'pause' | 'now' | 'status', automatic = false) => {
    if (action === 'enable' || action === 'pause') setVerified(false)
    return execute(
      action === 'enable' ? cloudEnableSync : action === 'pause' ? cloudPauseSync : action === 'now' ? cloudSyncNow : cloudSyncStatus,
      automatic, action === 'enable' || action === 'now', action === 'now',
    )
  }, [execute])
  const resolve = useCallback(async (conflict: SyncConflict, choice: 'local' | 'remote' | 'rename', name?: string) => { setVerified(false); const ok = await execute(() => cloudResolveSync(conflict, choice, name)); if (ok) for (const event of ['transaction-options-changed', 'accounts-changed', 'transactions-changed', 'incomes-changed', 'plans-changed', 'settings-changed', 'verification-changed', 'institutions-changed', 'subscription-providers-changed']) window.dispatchEvent(new CustomEvent(event, { detail: { source: 'cloud-sync' } })); return ok }, [execute])
  useEffect(() => {
    setStatus(null); setError(''); failures.current = 0; nextTry.current = 0; rateLimit.current = 0; automaticPaused.current = null
    setVerified(false); setHasLocalChanges(false); editRevision.current++
    if (!session.account) return
    let active = true
    const account = session.account.id
    // The native metadata check is deduplicated across StrictMode mounts.
    void cloudSyncStatus().then(value => { if (active && owner.current === account) setStatus(value) }).catch(() => { /* The explicit status action exposes errors. */ })
    return () => { active = false }
  }, [session.account?.id])
  useEffect(() => {
    if (session.deviceRegistered && ['Register this device before syncing.', 'Register this device in Manage devices before setting up encryption.'].includes(automaticPaused.current ?? '')) { automaticPaused.current = null; nextTry.current = 0 }
  }, [session.deviceRegistered])
  useEffect(() => {
    if (syncAccess === 'blocked') setVerified(false)
    if (syncAccess === 'allowed' && ['Sync access has ended or has not been granted. Your cloud data is retained.', 'Cloud sync is disabled on this service.'].includes(automaticPaused.current ?? '')) {
      automaticPaused.current = null; nextTry.current = 0; failures.current = 0
      setError('')
      // Only retry sync the user already enabled. Never enable a paused dataset.
      if (saved.current?.enabled && navigator.onLine) void act('now', true)
    }
  }, [syncAccess, session.subscription, act])
  useEffect(() => {
    if (!session.account) return
    const run = () => { if (saved.current?.enabled && session.online && access.current !== 'blocked' && navigator.onLine) void act('now', true) }
    let debounce: ReturnType<typeof setTimeout> | undefined
    const edited = (event: Event) => {
      if ((event as CustomEvent).detail?.source === 'cloud-sync') return
      editRevision.current++; setVerified(false); setHasLocalChanges(true)
      clearTimeout(debounce); debounce = setTimeout(run, 3000)
    }
    const start = setTimeout(run, 1000)
    const interval = setInterval(run, 60_000)
    const offline = () => { setVerified(false) }
    window.addEventListener('online', run); window.addEventListener('focus', run); window.addEventListener('offline', offline)
    const edits = ['transaction-options-changed', 'accounts-changed', 'transactions-changed', 'incomes-changed', 'plans-changed', 'settings-changed', 'verification-changed', 'institutions-changed', 'subscription-providers-changed']
    for (const event of edits) window.addEventListener(event, edited)
    return () => { clearTimeout(start); clearTimeout(debounce); clearInterval(interval); window.removeEventListener('online', run); window.removeEventListener('focus', run); window.removeEventListener('offline', offline); edits.forEach(event => window.removeEventListener(event, edited)) }
  }, [session.account?.id, session.online, syncAccess, status?.enabled, act])
  return <Context.Provider value={{ status, busy, error, verified, hasLocalChanges, act, resolve }}>{children}</Context.Provider>
}
export function useCloudSync() { const value = useContext(Context); if (!value) throw new Error('CloudSyncProvider is required'); return value }
