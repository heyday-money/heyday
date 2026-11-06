import { useEffect, useState } from 'react'
import { Check, Cloud, CloudOff, Clock3, KeyRound, Pause, Play, RefreshCw, TriangleAlert } from 'lucide-react'
import { t, getLocale, useLanguage } from '../lib/i18n'
import { Button } from './ui/button'
import { useCloudSync } from './CloudSyncProvider'
import type { EncryptionStatus } from '../lib/desktop'
import { useCloudSession } from './CloudSessionProvider'

export function CloudSyncStatus({ disabled, connected, onEnable, keyState, onPrepare }: { disabled: boolean; connected: boolean; onEnable: () => void; keyState: EncryptionStatus['state'] | null; onPrepare: () => void }) {
  useLanguage()
  const { status, busy, error, verified, hasLocalChanges, act } = useCloudSync()
  const { syncAccess } = useCloudSession()
  const noAccess = syncAccess === 'blocked'
  const [browserOnline, setOnline] = useState(navigator.onLine)
  const online = connected && browserOnline
  useEffect(() => {
    const update = () => setOnline(navigator.onLine)
    window.addEventListener('online', update); window.addEventListener('offline', update)
    return () => { window.removeEventListener('online', update); window.removeEventListener('offline', update) }
  }, [])
  const pending = status?.pending ? (status.pending === 1 ? t('1 change waiting to sync') : t('{count} changes waiting to sync', { count: status.pending })) : hasLocalChanges ? t('Changes waiting to sync') : null
  const complete = status?.fullEnabled && status.plannerEnabled && status.incomeEnabled && status.financialEnabled
  const needsKey = keyState !== null && keyState !== 'unlocked'
  const state = busy ? 'syncing'
    : noAccess ? 'access-required'
    : needsKey ? 'key-required'
    : error ? 'error'
    : !status ? 'loading'
    : !status.bound ? 'disconnected'
    : !status.enabled ? 'paused'
    : !online ? 'offline'
    : status.conflictCount > 0 ? 'conflict'
    : pending ? 'pending'
    : !status.bootstrapped || !status.lastSuccess || !verified ? 'waiting'
    : 'synced'
  const summaries = {
    'access-required': { title: t('Sync is waiting for access'), description: t('Check your account subscription above. Your local changes are kept on this device.'), icon: Pause },
    'key-required': { title: keyState === 'rotating' ? t('Finish replacing your recovery key') : (keyState === 'locked' || keyState === 'changed') ? t('Restore your key to sync') : t('Finish setting up cloud sync'), description: keyState === 'rotating' ? t('Sync is waiting for the key replacement to finish or be cancelled.') : (keyState === 'locked' || keyState === 'changed') ? t('Enter your recovery key once to reconnect this device securely.') : t('Save your recovery key before enabling encrypted cloud sync.'), icon: KeyRound },
    syncing: { title: t('Updating cloud sync…'), description: !status?.bootstrapped ? t('Downloading and verifying cloud records before merging local changes.') : t('Checking the cloud and saving your latest changes.'), icon: RefreshCw },
    loading: { title: t('Checking sync status…'), description: t('Loading this device’s sync settings.'), icon: RefreshCw },
    error: { title: t('Sync needs attention'), description: t('Sync could not finish. See the details below and try again.'), icon: TriangleAlert },
    disconnected: { title: t('Cloud sync is not connected'), description: t('Enable sync to keep your financial data available across your devices.'), icon: Cloud },
    paused: { title: t('Cloud sync paused'), description: t('Resume sync to send and receive changes. Your cloud data is retained.'), icon: Pause },
    offline: { title: t('You’re offline'), description: t('Sync will resume when your connection returns.'), icon: CloudOff },
    conflict: { title: t('Review conflicting changes'), description: t('Choose which version to keep below to finish syncing.'), icon: TriangleAlert },
    pending: { title: pending ?? t('Changes waiting to sync'), description: t('Your changes will sync automatically when ready.'), icon: Clock3 },
    waiting: { title: t('Ready to sync'), description: t('The next sync will check that your data is up to date.'), icon: Cloud },
    synced: { title: complete ? t('All changes synced') : t('Selected data synced'), description: complete ? t('Your latest changes are saved to the cloud.') : t('Your enabled sync categories are up to date.'), icon: Check },
  }
  const { title, description, icon: Icon } = summaries[state]
  const spinning = state === 'syncing' || state === 'loading'
  const iconClass = state === 'synced' ? 'bg-emerald-600 text-white dark:bg-emerald-500 dark:text-emerald-950' : state === 'error' ? 'bg-rose-100 text-rose-700 dark:bg-rose-950 dark:text-rose-300' : state === 'pending' || state === 'conflict' ? 'bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300' : spinning ? 'bg-sky-100 text-sky-700 dark:bg-sky-950 dark:text-sky-300' : 'bg-soft text-muted'
  const blocked = disabled || busy

  return <div data-sync-state={state} className={`overflow-hidden rounded-2xl border ${state === 'synced' ? 'border-emerald-600/25 bg-emerald-500/5' : 'border-line bg-card'}`}>
    <div role="status" aria-label={t('Sync status')} aria-live="polite" aria-atomic="true" className="flex items-start gap-4 p-5">
      <span className={`flex size-12 shrink-0 items-center justify-center rounded-full ${iconClass}`}><Icon aria-hidden="true" strokeWidth={state === 'synced' ? 3 : 2} className={`size-6 ${spinning ? 'motion-safe:animate-spin' : ''}`} /></span>
      <div className="min-w-0 space-y-1">
        <p className="font-semibold text-ink">{title}</p>
        <p className="text-sm text-muted">{description}</p>
        {pending && state !== 'disconnected' && state !== 'pending' && <p className="pt-1 text-sm font-medium text-ink">{pending}</p>}
        {status?.lastSuccess && <p className="pt-1 text-xs text-muted">{t('Last synced')}: <time dateTime={status.lastSuccess}>{new Date(status.lastSuccess).toLocaleString(getLocale(), { dateStyle: 'medium', timeStyle: 'short' })}</time></p>}
      </div>
    </div>
    <div className="flex flex-wrap items-center gap-2 border-t border-line/60 px-5 py-3">
      {needsKey ? <Button size="sm" disabled={blocked || !online} onClick={onPrepare}><KeyRound aria-hidden="true" />{keyState === 'rotating' ? t('Finish key replacement') : (keyState === 'locked' || keyState === 'changed') ? t('Restore encryption key') : keyState === 'pending' ? t('Finish encryption setup') : t('Set up encryption')}</Button>
        : status && (status.enabled ? <>
        <Button size="sm" variant="outline" disabled={blocked} onClick={() => void act('pause')}><Pause aria-hidden="true" />{t('Pause sync')}</Button>
      </> : <Button size="sm" disabled={blocked || !online || noAccess} onClick={onEnable}><Play aria-hidden="true" />{status.bound ? t('Resume sync') : t('Enable cloud sync')}</Button>)}
      {status?.enabled && !needsKey
        ? <Button size="icon-sm" variant="ghost" className="ml-auto text-muted" disabled={blocked || !online || noAccess} title={t('Sync now')} aria-label={t('Sync now')} onClick={() => void act('now')}><RefreshCw aria-hidden="true" className="size-4" /></Button>
        : <Button size="icon-sm" variant="ghost" className="ml-auto text-muted" disabled={blocked} title={t('Refresh sync status')} aria-label={t('Refresh sync status')} onClick={() => void act('status')}><RefreshCw aria-hidden="true" className="size-4" /></Button>}
    </div>
  </div>
}
