import { useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { deleteAccount, type Account } from '../lib/desktop'
import { t as translate, useLanguage } from '../lib/i18n'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function AccountDeleteAction({ account, disabled = false }: { account: Account; disabled?: boolean }) {
  useLanguage()
  const navigate = useNavigate()
  const trigger = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  if (!account.is_archived || account.paid_off_on) return null
  async function remove() {
    if (busy || disabled) return
    setBusy(true); setError(null)
    try {
      await deleteAccount(account.id)
      setOpen(false)
      toast.success(translate("Account deleted"))
      await navigate({ to: '/accounts' })
    } catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  return <>
    <Button ref={trigger} variant="destructive" disabled={disabled || busy} aria-label={translate("Delete account {value0}", { value0: account.name })} onClick={() => { setError(null); setOpen(true) }}>{translate("Delete account")}</Button>
    {open && <Dialog open onOpenChange={value => { if (!busy) setOpen(value) }}>
      <DialogContent onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (busy) event.preventDefault() }} showCloseButton={!busy} onCloseAutoFocus={event => { event.preventDefault(); trigger.current?.focus() }}>
        <DialogHeader>
          <DialogTitle>{translate("Delete account {value0}?", { value0: account.name })}</DialogTitle>
          <DialogDescription>{translate("Permanently delete this archived account and its opening balance? This cannot be undone. Accounts with transaction records or linked records cannot be deleted.")}</DialogDescription>
        </DialogHeader>
        {error && <p role="alert" className="text-sm text-red-600">{translate(error)}</p>}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={() => setOpen(false)}>{translate("Cancel")}</Button>
          <Button variant="destructive" disabled={busy || disabled} onClick={() => void remove()}>{busy ? translate("Deleting…") : translate("Delete account")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>}
  </>
}
