import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { cloudAccount, desktopAvailable, type CloudStatus } from '../lib/desktop'

interface CloudSession {
  status: CloudStatus
  loading: boolean
  error: string
  syncAccess: 'allowed' | 'blocked' | 'unknown'
  setStatus: (status: CloudStatus) => void
  refresh: () => Promise<void>
}
const Context = createContext<CloudSession | null>(null)

/** Share account metadata in memory; credentials remain in the native store. */
export function CloudSessionProvider({ children }: { children: ReactNode }) {
  const [status, updateStatus] = useState<CloudStatus>({ account: null, online: false })
  const [loading, setLoading] = useState(desktopAvailable)
  const [error, setError] = useState('')
  const revision = useRef(0)
  const pending = useRef(0)
  const [clock, setClock] = useState(Date.now)
  const setStatus = useCallback((next: CloudStatus) => {
    revision.current++
    updateStatus(next)
    setError('')
    setLoading(false)
  }, [])
  const check = useCallback(async (background = false) => {
    if (!desktopAvailable) return
    if (background && pending.current > 0) return
    pending.current++
    const request = ++revision.current
    if (!background) setLoading(true)
    setError('')
    try {
      const next = await cloudAccount()
      if (request === revision.current) { updateStatus(next); setClock(Date.now()) }
    } catch (error) {
      if (request === revision.current) {
        if (error === 'Sign-in account changed. Please sign in again.' || error === 'Your sign-in has expired or device access was revoked. Please sign in again.') updateStatus({ account: null, online: false })
        else updateStatus(previous => ({ ...previous, online: false, subscription: null }))
        setError(typeof error === 'string' ? error : 'Could not load your sign-in status. Please try again.')
      }
    } finally {
      pending.current--
      if (request === revision.current) setLoading(false)
    }
  }, [])
  const refresh = useCallback(() => check(), [check])
  useEffect(() => {
    void refresh()
    return () => { revision.current++ }
  }, [refresh])
  useEffect(() => {
    if (!status.account) return
    let lastCheck = 0
    const returned = () => {
      if (document.visibilityState === 'hidden' || !navigator.onLine || Date.now() - lastCheck < 2000) return
      lastCheck = Date.now()
      void check(true)
    }
    const interval = setInterval(returned, 60_000)
    window.addEventListener('focus', returned)
    window.addEventListener('online', returned)
    document.addEventListener('visibilitychange', returned)
    return () => { clearInterval(interval); window.removeEventListener('focus', returned); window.removeEventListener('online', returned); document.removeEventListener('visibilitychange', returned) }
  }, [status.account?.id, check])
  const entitlement = status.subscription?.entitlement
  const expires = entitlement?.accessUntil ? Date.parse(entitlement.accessUntil) : NaN
  useEffect(() => {
    if (!Number.isFinite(expires) || expires <= Date.now()) return
    const timer = setTimeout(() => setClock(Date.now()), Math.min(2_147_483_647, expires - Date.now() + 1))
    return () => clearTimeout(timer)
  }, [expires, clock])
  const syncAccess = !status.online || !entitlement ? 'unknown'
    : entitlement.canSync && Number.isFinite(expires) && expires > Math.max(clock, Date.now()) ? 'allowed' : 'blocked'
  return <Context.Provider value={{ status, loading, error, syncAccess, setStatus, refresh }}>{children}</Context.Provider>
}

export function useCloudSession() {
  const session = useContext(Context)
  if (!session) throw new Error('CloudSessionProvider is required')
  return session
}
