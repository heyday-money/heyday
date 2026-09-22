import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { desktopAvailable } from '../lib/desktop'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

type UpdateInfo = { current_version: string; version: string | null; notes: string | null; can_install: boolean; message: string }

export function AppUpdates({ version, disabled }: { version: string; disabled: boolean }) {
  const [info, setInfo] = useState<UpdateInfo | null>(null)
  const [checking, setChecking] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [confirm, setConfirm] = useState(false)
  const [error, setError] = useState<string | null>(null)
  async function check() {
    setChecking(true); setError(null); setInfo(null)
    try { setInfo(await invoke<UpdateInfo>('check_app_update')) }
    catch (error) { setError(typeof error === 'string' ? error : 'Could not check for updates. Please try again.') }
    finally { setChecking(false) }
  }
  async function install() {
    if (!info?.version || installing) return
    setInstalling(true); setError(null)
    try { await invoke('install_app_update', { version: info.version }) }
    catch (error) {
      setError(typeof error === 'string' ? error : 'Could not install the update. Please try again.')
      setInstalling(false)
    }
  }
  return <section className="mt-6 rounded-[22px] border border-line bg-card p-[27px]" aria-labelledby="updates-title">
    <h3 id="updates-title" className="text-lg font-semibold">App Updates</h3>
    <p className="mt-2 text-sm text-muted">Installed version: {version}. Updates come from heyday-money/heyday on GitHub.</p>
    <Button className="mt-4" variant="outline" disabled={!desktopAvailable || disabled || checking || installing} onClick={check}>{checking ? 'Checking…' : 'Check for Updates'}</Button>
    {info && <div className="mt-4 space-y-3" role="status">
      {info.version && <p className="font-medium">Version {info.version} is available</p>}
      <p className="text-sm">{info.message}</p>
      {info.notes && <details><summary className="cursor-pointer text-sm font-medium">Release notes</summary><p className="mt-2 max-h-60 overflow-y-auto whitespace-pre-wrap break-words text-sm text-muted">{info.notes}</p></details>}
      {info.version && info.can_install && <Button disabled={disabled || installing} onClick={() => { setError(null); setConfirm(true) }}>Download and Install</Button>}
      {!info.can_install && info.version && <p className="select-text break-all text-xs text-muted">https://github.com/heyday-money/heyday/releases</p>}
    </div>}
    {error && !confirm && <p className="mt-3 text-sm text-red-600 dark:text-red-400" role="alert">{error}</p>}
    <Dialog open={confirm} onOpenChange={value => { if (!installing) setConfirm(value) }}>
      <DialogContent showCloseButton={!installing} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (installing) event.preventDefault() }}>
        <DialogHeader><DialogTitle>Install version {info?.version}?</DialogTitle><DialogDescription>Save your work before continuing. Heyday Money will download and verify the update, back up your local database, then install and restart. Your financial records stay in the application data folder.</DialogDescription></DialogHeader>
        {installing && <p role="status" className="text-sm">Downloading, verifying, and installing… The app will restart when finished.</p>}
        {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{error}</p>}
        <DialogFooter><Button variant="outline" disabled={installing} onClick={() => setConfirm(false)}>Cancel</Button><Button disabled={installing} onClick={install}>{installing ? 'Updating…' : 'Install and Restart'}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
}
