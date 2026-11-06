import { useEffect, useRef, useState } from 'react'
import { Laptop, Monitor, ShieldOff } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
import { cloudDevices, cloudRegisterDevice, cloudRevokeDevice, type CloudDevice } from '../lib/desktop'
import { getLocale, t, useLanguage } from '../lib/i18n'

const expired = 'Your sign-in has expired or device access was revoked. Please sign in again.'
const changed = 'Sign-in account changed. Please sign in again.'
const manageError = 'Could not manage devices. Check your connection and try again.'

export function CloudDevices({ disabled, onBusyChange, onSignedOut, onRegistered }: {
  disabled: boolean; onBusyChange: (busy: boolean) => void; onSignedOut: (notice: string) => void; onRegistered: () => void
}) {
  useLanguage()
  const [open, setOpen] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const [busy, setBusy] = useState(false)
  const [current, setCurrent] = useState<CloudDevice | null>(null)
  const [devices, setDevices] = useState<CloudDevice[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [name, setName] = useState(() => t('My computer'))
  const [savedName, setSavedName] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [target, setTarget] = useState<CloudDevice | null>(null)
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])

  function working(value: boolean) { setBusy(value); onBusyChange(value) }
  function failed(error: unknown) {
    if (!mounted.current) return
    const message = typeof error === 'string' ? error : manageError
    if (message === expired || message === changed) onSignedOut(message)
    else setError(message)
  }
  async function fetchPage(next: string | null = null) {
    const result = await cloudDevices(next)
    if (!mounted.current) return
    setCurrent(result.currentDevice)
    setSavedName(result.savedName)
    if (result.savedName) setName(result.savedName)
    setDevices(previous => {
      const combined = next ? [...previous, ...result.page.devices] : result.page.devices
      return [...new Map(combined.map(device => [device.id, device])).values()]
    })
    setCursor(result.page.nextCursor); setLoaded(true)
  }
  async function load(next: string | null = null) {
    working(true); setError('')
    try { await fetchPage(next) } catch (error) { failed(error) }
    finally { if (mounted.current) working(false) }
  }
  async function register() {
    working(true); setError('')
    try {
      const device = await cloudRegisterDevice(name)
      if (!mounted.current) return
      setCurrent(device); toast.success(t('Device registered.'))
      await fetchPage()
      onRegistered()
    } catch (error) { failed(error) }
    finally { if (mounted.current) working(false) }
  }
  async function revoke() {
    if (!target) return
    working(true); setError('')
    try {
      const result = await cloudRevokeDevice(target.id)
      if (!mounted.current) return
      toast.success(t('Device access revoked.'))
      setTarget(null)
      if (result.current) {
        onBusyChange(false)
        onSignedOut(result.credentialsCleared
          ? 'Access revoked for this computer. Your local finances are still available.'
          : 'Device access was revoked, but saved credentials could not be cleared from the secure store.')
      } else {
        setDevices(previous => previous.map(device => device.id === target.id ? { ...device, revokedAt: new Date().toISOString() } : device))
        await fetchPage()
      }
    } catch (error) { failed(error) }
    finally { if (mounted.current) working(false) }
  }
  function date(value: string) {
    const parsed = new Date(value)
    return Number.isNaN(parsed.getTime()) ? t('Unavailable') : parsed.toLocaleString(getLocale(), { dateStyle: 'medium', timeStyle: 'short' })
  }
  function row(device: CloudDevice, isCurrent = false) {
    const Icon = device.revokedAt ? ShieldOff : device.platform === 'macos' ? Laptop : Monitor
    return <li key={device.id} className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-card p-4">
      <Icon aria-hidden="true" className="size-5 shrink-0 text-muted" />
      <div className="min-w-0 flex-1 basis-40">
        <p className="break-words font-medium">{device.name}</p>
        <p className="text-xs text-muted">{device.platform === 'macos' ? 'macOS' : device.platform === 'windows' ? 'Windows' : 'Linux'}
          {isCurrent && <span className="ml-2 font-semibold text-primary">{t('This device')}</span>}
        </p>
        <p className="mt-1 text-xs text-muted">{device.revokedAt ? t('Revoked on {value0}', { value0: date(device.revokedAt) }) : t('Last active {value0}', { value0: date(device.lastSeenAt) })}</p>
      </div>
      {device.revokedAt ? <span className="text-sm text-muted">{t('Revoked')}</span>
        : <Button variant="outline" disabled={busy || disabled} aria-label={t('Revoke access for {value0}', { value0: device.name })} onClick={() => { setError(''); setTarget({ ...device, current: isCurrent }) }}>{t('Revoke access')}</Button>}
    </li>
  }
  return <section aria-labelledby="devices-heading" className="space-y-4 border-t border-line pt-5">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h4 id="devices-heading" className="font-semibold">{t('Devices')}</h4>
        <p className="mt-1 text-sm text-muted">{t('Manage devices with access to your Heyday account.')}</p>
      </div>
      <Button variant="outline" disabled={busy || disabled} onClick={() => { setOpen(true); void load() }}>{open ? t('Refresh devices') : t('Manage devices')}</Button>
    </div>
    {disabled && <p role="status" className="text-sm text-muted">{t('Connect and refresh your account status to manage devices.')}</p>}
    {open && <>
      {busy && <p role="status" className="text-sm text-muted">{t('Updating devices…')}</p>}
      {loaded && !current && <form onSubmit={event => { event.preventDefault(); void register() }} className="space-y-3 rounded-xl border border-line bg-soft p-4">
        <p className="text-sm">{t('Register this computer to identify its account access. This does not enable cloud sync.')}</p>
        <label htmlFor="cloud-device-name" className="block text-sm font-medium">{t('Device name')}</label>
        <div className="flex flex-wrap gap-3">
          <input id="cloud-device-name" value={name} onChange={event => setName(event.target.value)} maxLength={80} required readOnly={!!savedName} disabled={busy || disabled} className="min-w-0 flex-1 basis-48 rounded-lg border border-line bg-card px-3 py-2 text-sm" />
          <Button type="submit" disabled={busy || disabled || !name.trim()}>{t('Register this device')}</Button>
        </div>
        {savedName && <p className="text-xs text-muted">{t('Reuses the name saved for this installation.')}</p>}
      </form>}
      {current && <ul className="space-y-3" aria-label={t('Current device')}>{row(current, true)}</ul>}
      {devices.some(device => device.id !== current?.id) && <ul className="space-y-3" aria-label={t('Other devices')}>{devices.filter(device => device.id !== current?.id).map(device => row(device))}</ul>}
      {loaded && devices.length === 0 && !current && <p className="text-sm text-muted">{t('No devices registered yet.')}</p>}
      {cursor && <Button variant="outline" disabled={busy || disabled} onClick={() => void load(cursor)}>{t('Load more devices')}</Button>}
      {error && !target && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
    </>}
    <Dialog open={!!target} onOpenChange={value => { if (!value && !busy) { setTarget(null); setError('') } }}>
      <DialogContent showCloseButton={!busy} onPointerDownOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy) event.preventDefault() }}>
        <DialogHeader>
          <DialogTitle>{t('Revoke device access?')}</DialogTitle>
          <DialogDescription>{target?.current
            ? t('This will sign you out on this computer and remove its saved encryption key. Your local finances will stay available.')
            : t('This ends account access for this device. Financial data on that device is not erased.')}</DialogDescription>
        </DialogHeader>
        <p className="break-words font-medium">{target?.name}</p>
        {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={() => { setTarget(null); setError('') }}>{t('Cancel')}</Button>
          <Button variant="destructive" disabled={busy || disabled} onClick={() => void revoke()}>{busy ? t('Revoking…') : t('Revoke access')}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
}
