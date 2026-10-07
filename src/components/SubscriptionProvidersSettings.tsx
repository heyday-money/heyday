import { useRef, useState, type FormEvent } from 'react'
import { toast } from 'sonner'
import { t, useLanguage } from '../lib/i18n'
import { desktopAvailable } from '../lib/desktop'
import { providerIcon, saveSubscriptionProvider, useSubscriptionProviders, type SubscriptionProvider } from '../lib/subscription-providers'
import type { LogoChange } from '../lib/logos'
import { LogoImage, useLogoAsset } from './LogoImage'
import { LogoPicker } from './LogoPicker'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function ProviderLogo({ provider }: { provider: SubscriptionProvider }) {
  const asset = useLogoAsset(provider.logo_asset_id)
  return <LogoImage name={provider.name} src={provider.logo_mode === 'none' ? null : asset ?? providerIcon(provider.builtin_icon)} className="size-9 rounded-lg text-base" />
}
export function SubscriptionProvidersSettings() {
  useLanguage()
  const directory = useSubscriptionProviders()
  const [editing, setEditing] = useState<SubscriptionProvider | 'new' | null>(null)
  const [search, setSearch] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  async function toggle(provider: SubscriptionProvider) {
    if (busy) return
    setBusy(true); setError(null)
    try { await saveSubscriptionProvider({ id: provider.id, name: provider.name, is_archived: !provider.is_archived }); toast.success(t('Subscription provider saved.')) }
    catch (e) { setError(String(e)) } finally { setBusy(false) }
  }
  return <section className="min-w-0">
    <div className="flex flex-wrap items-center justify-between gap-3"><h3 className="text-lg font-semibold">{t('Subscription providers')}</h3><Button disabled={!desktopAvailable || busy || directory.loading || directory.error} onClick={() => setEditing('new')}>{t('Add provider')}</Button></div>
    <p className="my-3 text-sm">{t('Choose a provider when adding a subscription to reuse its name and icon. Provider icons update linked subscriptions; subscription names stay as entered. Archived providers remain on existing subscriptions.')}</p>
    <Input aria-label={t('Search providers')} placeholder={t('Search providers')} value={search} onChange={e => setSearch(e.target.value)} />
    {directory.loading && <p role="status">{t('Loading providers…')}</p>}
    {directory.error && <p role="alert">{t('Could not load subscription providers.')} <Button variant="outline" onClick={directory.reload}>{t('Retry')}</Button></p>}
    {!directory.error && <ul className="mt-3 divide-y divide-line">{directory.providers.filter(p => p.name.toLowerCase().includes(search.toLowerCase())).map(provider => <li key={provider.id} className="flex flex-wrap items-center gap-3 py-3">
      <ProviderLogo provider={provider} /><span className="min-w-0 flex-1 break-words text-sm font-medium">{provider.name}{provider.is_archived && t(' (archived)')}</span>
      <Button size="sm" variant="outline" disabled={busy} aria-label={t('Edit provider {value0}', { value0: provider.name })} onClick={() => setEditing(provider)}>{t('Edit')}</Button>
      <Button size="sm" variant="ghost" disabled={busy} aria-label={t('{value0} provider {value1}', { value0: provider.is_archived ? t('Restore') : t('Archive'), value1: provider.name })} onClick={() => void toggle(provider)}>{provider.is_archived ? t('Restore') : t('Archive')}</Button>
    </li>)}</ul>}
    {error && <p role="alert">{t(error)}</p>}
    {editing && <ProviderDialog provider={editing === 'new' ? undefined : editing} onClose={() => setEditing(null)} />}
  </section>
}
function ProviderDialog({ provider, onClose }: { provider?: SubscriptionProvider; onClose: () => void }) {
  const [name, setName] = useState(provider?.name ?? '')
  const [logo, setLogo] = useState<LogoChange>()
  const asset = useLogoAsset(provider?.logo_asset_id)
  const saved = provider?.logo_mode === 'none' ? null : asset ?? providerIcon(provider?.builtin_icon)
  const [busy, setBusy] = useState(false)
  const lock = useRef(false), imageLock = useRef(false)
  const [imageBusy, setImageBusy] = useState(false)
  const [discard, setDiscard] = useState(false)
  const [error, setError] = useState<string | null>(null)
  function close() { if (lock.current || imageLock.current) return; if (name !== (provider?.name ?? '') || logo !== undefined) setDiscard(true); else onClose() }
  async function save(e: FormEvent) {
    e.preventDefault(); if (lock.current || imageLock.current) return
    lock.current = true; setBusy(true); setError(null)
    try { await saveSubscriptionProvider({ id: provider?.id ?? null, name, is_archived: provider?.is_archived ?? false, logo_change: logo }); onClose(); toast.success(t('Subscription provider saved.')) }
    catch (e) { setError(String(e)) } finally { lock.current = false; setBusy(false) }
  }
  return <Dialog open onOpenChange={open => { if (!open) close() }}><DialogContent showCloseButton={!busy && !imageBusy} onInteractOutside={e => e.preventDefault()} className="max-h-[85dvh] overflow-y-auto">
    <DialogHeader><DialogTitle>{provider ? t('Edit provider') : t('Add provider')}</DialogTitle><DialogDescription>{t('Save a reusable provider name and optional icon. No price or billing schedule is created.')}</DialogDescription></DialogHeader>
    <form onSubmit={save} className="space-y-4"><FormField label={t('Provider name')}><Input required maxLength={100} value={name} disabled={busy} onChange={e => setName(e.target.value)} /></FormField>
      <LogoPicker name={name} value={logo} savedSource={saved} defaultSource={providerIcon(provider?.builtin_icon)} hasSavedLogo={!!provider?.logo_asset_id} disabled={busy} onChange={setLogo} onBusyChange={value => { imageLock.current = value; setImageBusy(value) }} />
      {error && <p role="alert">{t(error)}</p>}
      {discard ? <><p>{t('Discard unsaved changes?')}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{t('Keep editing')}</Button><Button type="button" variant="destructive" onClick={onClose}>{t('Discard changes')}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={busy || imageBusy} onClick={close}>{t('Cancel')}</Button><Button disabled={busy || imageBusy}>{busy ? t('Saving…') : t('Save provider')}</Button></DialogFooter>}
    </form>
  </DialogContent></Dialog>
}
