import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
import { cloudCancelSignIn, cloudSignIn, cloudSignOut, desktopAvailable } from '../lib/desktop'
import { t, useLanguage } from '../lib/i18n'
import { CloudDevices } from './CloudDevices'
import { CloudEncryption } from './CloudEncryption'
import { useCloudSession } from './CloudSessionProvider'
import { useCloudSync } from './CloudSyncProvider'
import { CloudSync } from './CloudSync'
import { CloudSubscription } from './CloudSubscription'

export function CloudAccount() {
  useLanguage()
  const { status, setStatus, loading, error: statusError, refresh: refreshStatus } = useCloudSession()
  const { busy: syncing } = useCloudSync()
  const [action, setBusy] = useState<'login' | 'logout' | null>(null)
  const busy = action ?? (loading ? 'load' : syncing ? 'sync' : null)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [devicesBusy, setDevicesBusy] = useState(false)
  const [encryptionBusy, setEncryptionBusy] = useState(false)
  const [forgetOpen, setForgetOpen] = useState(false)
  const signingIn = useRef(false)
  useEffect(() => {
    return () => { if (signingIn.current) void cloudCancelSignIn().catch(() => {}) }
  }, [])
  async function login() {
    setError(''); setNotice(''); setBusy('login'); signingIn.current = true
    try { setStatus(await cloudSignIn()); toast.success(t('Signed in.')) }
    catch (error) { if (error !== 'Sign-in cancelled.') setError(typeof error === 'string' ? error : 'Could not sign in. Please try again.') }
    finally { signingIn.current = false; setBusy(null) }
  }
  async function refresh() {
    setError('')
    await refreshStatus()
  }
  async function logout(forgetDevice = false) {
    setError(''); setNotice(''); setBusy('logout')
    try {
      const revoked = await cloudSignOut(forgetDevice)
      setForgetOpen(false)
      setStatus({ account: null, online: false })
      if (!revoked) setNotice('Signed out on this device. The server could not be reached to revoke the session.')
      else toast.success(t('Signed out.'))
    } catch (error) { setError(typeof error === 'string' ? error : 'Could not sign out. Please try again.') }
    finally { setBusy(null) }
  }
  return <section aria-labelledby="cloud-account-heading" className="space-y-4">
    <h3 id="cloud-account-heading" className="text-lg font-semibold">{t('Heyday account')}</h3>
    <p className="text-sm text-muted">{t('Beta sign-in uses a code sent to your email. Your local finances remain available without signing in.')}</p>
    {status.account && <div className="rounded-xl border border-line bg-soft p-4">
      <p className="text-sm">{t('Signed in as')}</p>
      <p className="mt-1 break-all font-medium">{status.account.email}</p>
      {!status.online && <p role="status" className="mt-2 text-sm text-muted">{t('Offline or session unavailable. Showing the account saved on this device.')}</p>}
    </div>}
    {busy === 'login' && <p role="status" className="text-sm">{t('Finish signing in with your email code in the browser. This window will update automatically.')}</p>}
    {busy === 'load' && <p role="status" className="text-sm text-muted">{t('Checking sign-in…')}</p>}
    <div className="flex flex-wrap gap-3">
      {status.account ? <Button variant="outline" disabled={!!busy || devicesBusy || encryptionBusy} onClick={() => void logout()}>{busy === 'logout' ? t('Signing out…') : t('Sign out')}</Button>
        : <Button disabled={!desktopAvailable || !!busy} onClick={login}>{t('Sign in with email')}</Button>}
      {busy === 'login' ? <Button variant="outline" onClick={() => void cloudCancelSignIn().catch(() => setError('Could not cancel sign-in. Please try again.'))}>{t('Cancel sign-in')}</Button>
        : <Button variant="outline" disabled={!desktopAvailable || !!busy || devicesBusy || encryptionBusy} onClick={refresh}>{t('Refresh status')}</Button>}
    </div>
    {status.account && <Button variant="ghost" disabled={!!busy || devicesBusy || encryptionBusy} onClick={() => { setError(''); setForgetOpen(true) }}>{t('Sign out and forget this device')}</Button>}
    {!forgetOpen && (error || statusError) && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error || statusError)}</p>}
    {notice && <p role="status" className="text-sm text-muted">{t(notice)}</p>}
    {status.deviceError && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(status.deviceError)}</p>}
    {status.account && <CloudSubscription key={`subscription-${status.account.id}`} disabled={!desktopAvailable || !!action} />}
    {status.account && <CloudEncryption key={`encryption-${status.account.id}`} disabled={!!busy || devicesBusy} online={status.online} registered={status.deviceRegistered} onBusyChange={setEncryptionBusy} onSignedOut={message => { setStatus({ account: null, online: false }); setDevicesBusy(false); setEncryptionBusy(false); setNotice(message); setError('') }}>
      {(keyState, prepare) => <CloudSync disabled={!!action || loading || devicesBusy || encryptionBusy} online={status.online} email={status.account!.email} keyState={keyState} onPrepare={prepare} />}
    </CloudEncryption>}
    {status.account && <CloudDevices key={status.account.id} disabled={!!busy || encryptionBusy || !status.online} onRegistered={() => void refreshStatus()} onBusyChange={setDevicesBusy} onSignedOut={message => { setStatus({ account: null, online: false }); setDevicesBusy(false); setEncryptionBusy(false); setNotice(message); setError('') }} />}
    <p className="text-sm text-muted">{t('Signing in does not start cloud sync. Uploading financial data requires your explicit confirmation.')}</p>
    <Dialog open={forgetOpen} onOpenChange={value => { if (busy !== 'logout') setForgetOpen(value) }}>
      <DialogContent showCloseButton={busy !== 'logout'} onPointerDownOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy === 'logout') event.preventDefault() }}>
        <DialogHeader><DialogTitle>{t('Sign out and forget this device?')}</DialogTitle><DialogDescription>{t('This removes the saved encryption key and stops automatic device reconnection. After signing in again, register this computer and enter your recovery key. Your local finances remain available.')}</DialogDescription></DialogHeader>
        {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
        <DialogFooter>
          <Button variant="outline" disabled={busy === 'logout'} onClick={() => setForgetOpen(false)}>{t('Cancel')}</Button>
          <Button variant="destructive" disabled={!!busy || devicesBusy || encryptionBusy} onClick={() => void logout(true)}>{busy === 'logout' ? t('Signing out…') : t('Sign out and forget this device')}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
}
