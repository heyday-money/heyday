import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ChevronDown, KeyRound, LoaderCircle, ShieldCheck, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'
import { cloudCancelKeyRotation, cloudConfirmKeyRotation, cloudHistoryVerificationCode, cloudPrepareKeyRotation, cloudCancelEncryption, cloudConfirmEncryption, cloudEncryptionStatus, cloudLockEncryption, cloudPrepareEncryption, cloudRecoverEncryption, cloudRememberEncryption, cloudSaveRecoveryKey, type EncryptionStatus } from '../lib/desktop'
import { t, useLanguage } from '../lib/i18n'
import { useCloudSync } from './CloudSyncProvider'

export function CloudEncryption({ disabled, online, registered = false, onBusyChange, onSignedOut, children }: {
  disabled: boolean; online: boolean; registered?: boolean; onBusyChange: (busy: boolean) => void; onSignedOut: (notice: string) => void
  children: (state: EncryptionStatus['state'] | null, prepare: () => void) => ReactNode
}) {
  useLanguage()
  const sync = useCloudSync()
  const [status, setStatus] = useState<EncryptionStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [dialog, setDialog] = useState<'setup' | 'resume' | 'recover' | 'remove' | 'rotate' | 'rotate-resume' | 'history' | null>(null)
  const [generated, setGenerated] = useState('')
  const [entered, setEntered] = useState('')
  const [historyCode, setHistoryCode] = useState('')
  const [allowUnverified, setAllowUnverified] = useState(false)
  const [saved, setSaved] = useState(false)
  const [remember, setRemember] = useState(true)
  const mounted = useRef(true)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  useEffect(() => {
    mounted.current = true
    return () => { mounted.current = false; clearTimeout(timer.current); void cloudCancelEncryption().catch(() => {}) }
  }, [])
  useEffect(() => { if (registered && online) void refresh() }, [registered, online])
  useEffect(() => { if (sync.error === 'Encryption changed on another device. Restore with the latest recovery key.' || sync.error === 'Verify cloud history with a code from a trusted device before syncing.') void refresh() }, [sync.error])
  function working(value: boolean) { setBusy(value); onBusyChange(value) }
  function clearSecrets() { setGenerated(''); setEntered(''); setHistoryCode(''); setAllowUnverified(false); setSaved(false); setRemember(true); clearTimeout(timer.current) }
  function close() { clearSecrets(); setDialog(null); setError(''); void cloudCancelEncryption().catch(() => {}) }
  function failure(error: unknown) {
    if (!mounted.current) return
    const message = typeof error === 'string' ? error : 'Could not update encryption. Please try again.'
    if (message === 'Your sign-in has expired or device access was revoked. Please sign in again.') { clearSecrets(); onSignedOut(message) }
    else setError(message)
  }
  async function run(action: () => Promise<void>) {
    working(true); setError('')
    try { await action() } catch (error) {
      failure(error)
      if (dialog === 'rotate' || dialog === 'rotate-resume') {
        try { const next = await cloudEncryptionStatus(); if (mounted.current) setStatus(next) } catch { /* Keep the action's error and saved retry. */ }
      }
    }
    finally { if (mounted.current) working(false) }
  }
  async function refresh() {
    await run(async () => { const next = await cloudEncryptionStatus(); if (mounted.current) setStatus(next) })
  }
  async function begin(rotate = false) {
    await run(async () => {
      const prepared = await (rotate ? cloudPrepareKeyRotation() : cloudPrepareEncryption())
      if (!mounted.current) { await cloudCancelEncryption(); return }
      clearSecrets(); setGenerated(prepared.recoveryKey); setDialog(rotate ? 'rotate' : 'setup')
      timer.current = setTimeout(() => {
        clearSecrets(); setDialog(null); setError('Encryption setup expired or was cancelled. Start setup again.')
        void cloudCancelEncryption().catch(() => {})
      }, 600_000)
    })
  }
  function prepare() {
    if (blocked || !online) return
    if (status?.rotationPending) { clearSecrets(); setError(''); setDialog('rotate-resume') }
    else if (status?.state === 'not_set') void begin()
    else if (status?.state === 'locked' || status?.state === 'changed' || status?.state === 'pending' || status?.historyVerified === false) {
      clearSecrets(); setError(''); setDialog(status.state === 'pending' ? 'resume' : 'recover')
    } else void refresh()
  }
  async function finish() {
    await run(async () => {
      if (dialog === 'remove') {
        // Pause durably before removing the key. If key-store access fails, leave
        // sync paused and keep this confirmation available for a retry.
        if (!await sync.act('pause')) throw 'Could not pause sync. Try again before removing the encryption key.'
        await cloudLockEncryption()
        if (mounted.current) { setStatus({ state: 'locked' }); close(); toast.success(t('Saved encryption key removed. Sync is paused.')) }
        return
      }
      const next = rotating ? await cloudConfirmKeyRotation(entered, saved) : dialog === 'recover' ? await cloudRecoverEncryption(entered, remember, historyCode, allowUnverified) : await cloudConfirmEncryption(entered, saved, remember)
      if (mounted.current) { setStatus(next); close(); toast.success(t('Encryption keys are ready on this device.')) }
      // Recovery does not enable a new connection or undo an intentional pause.
      // An existing enabled connection may retry now that its key is available.
      if (sync.status?.enabled) await sync.act('now')
      else await sync.act('status')
    })
  }
  const needsHistory = status?.historyVerified === false && !status.unverifiedHistory
  const rotating = dialog === 'rotate' || dialog === 'rotate-resume'
  const blocked = busy || disabled
  return <div className="space-y-4">
    {children(status?.rotationPending ? 'rotating' : status?.state === 'unlocked' && needsHistory ? 'locked' : status?.state ?? null, prepare)}
    {busy && <p role="status" className="text-sm text-muted">{t('Updating encryption…')}</p>}
    {status?.state === 'not_set' && <div className="space-y-3 rounded-xl border border-line bg-soft p-4">
      <p className="text-sm">{t('Set up encryption and keep your recovery key somewhere safe. Support cannot recover it.')}</p>
    </div>}
    {status?.state === 'pending' && <div className="space-y-3 rounded-xl border border-line bg-soft p-4">
      <p className="text-sm">{t('Setup was interrupted. Use the recovery key you already saved to finish without replacing your encryption key.')}</p>
    </div>}
    {status?.state === 'locked' && <div className="space-y-3 rounded-xl border border-line bg-soft p-4">
      <p className="text-sm">{t('This device needs your recovery key before it can sync. Your cloud data is still encrypted and retained.')}</p>
    </div>}
    {status?.unverifiedHistory && <div role="status" className="space-y-2 rounded-xl border border-amber-500/30 bg-amber-500/5 p-4 text-sm"><p className="font-medium">{t('History recovered without independent verification')}</p><p>{t('The recovery key unlocked your data, but older cloud history could not be checked against a trusted device. Future verified checkpoints are saved on this device to detect rollback.')}</p><Button variant="outline" size="sm" disabled={blocked || !online} onClick={() => { clearSecrets(); setDialog('recover') }}>{t('Verify recovered history')}</Button></div>}
    {status?.state === 'changed' && <p role="status" className="rounded-xl border border-amber-500/30 bg-amber-500/5 p-4 text-sm">{t('Encryption changed on another device. Restore with the latest recovery key.')}</p>}
    {status?.rotationPending && <div className="space-y-3 rounded-xl border border-line p-4"><p className="text-sm">{t('A recovery-key replacement is pending. Use the new key you saved to finish, or check whether it can be cancelled.')}</p><div className="flex flex-wrap gap-2"><Button variant="outline" disabled={blocked || !online} onClick={() => void run(async () => { const next = await cloudCancelKeyRotation(); if (mounted.current) setStatus(next) })}>{t('Cancel pending replacement')}</Button></div></div>}
    <details className="group rounded-xl border border-line px-4">
      <summary className="flex cursor-pointer list-none items-center gap-2 py-3 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden"><ShieldCheck aria-hidden="true" className="size-4 text-muted" />{t('Advanced security')}<ChevronDown aria-hidden="true" className="ml-auto size-4 text-muted transition-transform group-open:rotate-180" /></summary>
      <div className="space-y-4 border-t border-line py-4">
      <p className="text-sm text-muted">{t('Encryption is automatic. Use Pause sync to stop sending and receiving changes while keeping this device ready.')}</p>
      <Button size="sm" variant="outline" disabled={blocked || !online} onClick={() => void refresh()}>{t('Check encryption status')}</Button>
      {status?.state === 'unlocked' && <div className="space-y-3">
      <div className="space-y-3 border-b border-line pb-4">
        <p className="text-sm text-muted">{t('Replace your recovery key to rotate encryption for future changes. Other devices will need the new key. Existing history stays readable; copies already held by another device cannot be erased.')}</p>
        <Button variant="outline" disabled={blocked || !online || !!status.rotationPending || needsHistory} onClick={() => void begin(true)}><KeyRound aria-hidden="true" className="size-4" />{t('Replace recovery key')}</Button>
        <p className="text-sm text-muted">{t('Verify a new device using a code copied directly from this trusted device. This checks cloud history before any downloaded financial records are applied.')}</p>
        <Button variant="outline" disabled={blocked || !online || !!status.rotationPending || needsHistory} onClick={() => void run(async () => { const code = await cloudHistoryVerificationCode(); if (mounted.current) { clearSecrets(); setHistoryCode(code); setDialog('history') } })}>{t('Verify another device')}</Button>
      </div>
      <label className="flex items-start gap-3 text-sm"><input type="checkbox" checked={!!status.rememberEncryption} disabled={blocked || !online} onChange={event => { const remember = event.target.checked; void run(async () => { const next = await cloudRememberEncryption(remember); if (mounted.current) setStatus(next) }) }} className="mt-1" /><span>{t('Remember encryption on this device')}</span></label>
      <p className="text-xs text-muted">{t('Keeps your encryption key in the system secure store after sign-out. Only this account can use it after signing in again.')}</p>
      <div className="space-y-3 border-t border-line pt-4">
      <p className="text-sm text-muted">{t('Remove the saved key only if you want this device to require your recovery key again. This also pauses sync.')}</p>
      <Button variant="outline" className="text-rose-700 hover:text-rose-800 dark:text-rose-300" disabled={blocked} onClick={() => { setError(''); setDialog('remove') }}>
        <Trash2 aria-hidden="true" className="size-4 shrink-0" />
        {t('Remove saved encryption key')}
      </Button>
      </div></div>}
      </div>
    </details>
    {error && !dialog && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
    <Dialog open={dialog !== null} onOpenChange={value => { if (!value && !busy) close() }}>
      <DialogContent showCloseButton={!busy} onPointerDownOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy) event.preventDefault() }}>
        <DialogHeader>
          <DialogTitle>{dialog === 'history' ? t('History verification code') : rotating ? t('Replace recovery key and rotate encryption') : dialog === 'remove' ? t('Remove saved encryption key?') : dialog === 'recover' ? t('Restore encryption key') : t('Save your recovery key')}</DialogTitle>
          <DialogDescription>{dialog === 'history' ? t('Transfer this code directly to your new device. It pins the history verified now and contains no encryption key. Generate a new code if the recovery key changes.') : rotating ? t('Save and re-enter the new key. It replaces the old recovery key and rotates encryption for future changes. Other devices must restore with the new key. Support cannot recover it.') : dialog === 'remove'
            ? t('Sync will pause and the saved encryption key will be removed from this device. You will need your recovery key to sync again. Your local finances and encrypted cloud data remain available.')
            : t('Keep this key private and separate from this computer. Anyone with the key and encrypted data can read that data. Support cannot recover a lost key.')}</DialogDescription>
        </DialogHeader>
        {dialog === 'history' && status?.unverifiedHistory && <p className="text-sm text-amber-800 dark:text-amber-200">{t('This device recovered without independent verification. The new device will keep the same history notice.')}</p>}
        {dialog === 'history' && <textarea aria-label={t('History verification code')} readOnly value={historyCode} spellCheck={false} className="min-h-40 w-full select-text break-all rounded-lg border border-line bg-soft p-3 font-mono text-xs" />}
        {generated && <div className="space-y-3">
          <label htmlFor="generated-recovery" className="text-sm font-medium">{t('Your recovery key')}</label>
          <textarea id="generated-recovery" readOnly value={generated} spellCheck={false} className="min-h-24 w-full select-text break-all rounded-lg border border-line bg-soft p-3 font-mono text-sm" />
          <Button variant="outline" disabled={busy} onClick={() => void run(async () => { if (await cloudSaveRecoveryKey()) toast.success(t('Recovery key saved. Store the file somewhere safe.')) })}>{t('Save recovery key file')}</Button>
          <p className="text-xs text-muted">{t('You can also save this key in your password manager. This screen expires after ten minutes.')}</p>
        </div>}
        {dialog !== 'remove' && dialog !== 'history' && <>
          <label htmlFor="recovery-entry" className="text-sm font-medium">{dialog === 'setup' || rotating ? t('Re-enter your saved recovery key') : t('Recovery key')}</label>
          <input id="recovery-entry" type="password" autoComplete="off" spellCheck={false} maxLength={128} value={entered} onChange={event => setEntered(event.target.value)} disabled={busy} className="w-full rounded-lg border border-line bg-card px-3 py-2 font-mono text-sm" />
          {dialog === 'recover' && status?.historyVerified === false && <div className="space-y-2"><label htmlFor="history-code" className="text-sm font-medium">{t('History verification code')}</label><textarea id="history-code" value={historyCode} onChange={event => setHistoryCode(event.target.value)} disabled={busy} autoComplete="off" spellCheck={false} maxLength={4096} className="min-h-24 w-full rounded-lg border border-line bg-card p-3 font-mono text-xs" /><p className="text-xs text-muted">{t('On a trusted device, open Advanced security and choose Verify another device. A recovery key alone cannot prove that the server returned the latest history. Keep a trusted device available until verification completes.')}</p>{needsHistory && <label className="flex items-start gap-3 rounded-lg border border-amber-500/30 p-3 text-sm"><input type="checkbox" checked={allowUnverified} onChange={event => setAllowUnverified(event.target.checked)} disabled={busy || !!historyCode.trim()} className="mt-1" /><span>{t('I no longer have a trusted device or verification code. I understand that the server could return older or incomplete history, and I want to recover without independent history verification.')}</span></label>}</div>}
          {dialog !== 'recover' && <label className="flex items-start gap-3 text-sm"><input type="checkbox" checked={saved} disabled={busy} onChange={event => setSaved(event.target.checked)} className="mt-1" /><span>{t('I saved my recovery key and understand that support cannot recover my encrypted data.')}</span></label>}
          {!rotating && <><label className="flex items-start gap-3 text-sm"><input type="checkbox" checked={remember} disabled={busy} onChange={event => setRemember(event.target.checked)} className="mt-1" /><span>{t('Remember encryption on this device')}</span></label>
          <p className="text-xs text-muted">{t('Keeps your encryption key in the system secure store after sign-out. Only this account can use it after signing in again.')}</p></>}
        </>}
        {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={close}>{t('Cancel')}</Button>
          {dialog !== 'history' && <Button variant={dialog === 'remove' ? 'destructive' : 'default'} disabled={blocked || (dialog !== 'remove' && (!online || !entered.trim() || (dialog === 'recover' && needsHistory && !historyCode.trim() && !allowUnverified) || (dialog !== 'recover' && !saved)))} onClick={() => void finish()}>
            {busy ? <LoaderCircle aria-hidden="true" className="size-4 shrink-0 animate-spin" />
              : dialog === 'remove' ? <Trash2 aria-hidden="true" className="size-4 shrink-0" />
              : dialog === 'recover' ? <KeyRound aria-hidden="true" className="size-4 shrink-0" /> : null}
            {rotating ? t('Replace key and rotate encryption') : dialog === 'remove' ? t('Remove saved encryption key') : dialog === 'recover' ? t('Restore encryption key') : t('Confirm encryption setup')}
          </Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
}
