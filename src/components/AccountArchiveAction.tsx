import { useState } from 'react'
import { toast } from 'sonner'
import { t as translate, useLanguage } from '../lib/i18n'
import { setAccountArchived, type Account } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'

export function AccountArchiveAction({ account, currency }: { account: Account; currency: string }) {
  useLanguage()
  const [archiveTarget, setArchiveTarget] = useState<Account | null>(null)
  const [archiveBusy, setArchiveBusy] = useState(false)
  const [archiveError, setArchiveError] = useState<string | null>(null)
  function chooseArchive(account: Account) { setArchiveError(null); setArchiveTarget(account) }
  async function saveArchive() {
    if (!archiveTarget || archiveBusy) return
    setArchiveBusy(true); setArchiveError(null)
    try {
      await setAccountArchived(archiveTarget.id, !archiveTarget.is_archived)
      toast.success(archiveTarget.is_archived ? translate("Account restored") : translate("Account archived"))
      setArchiveTarget(null)
    } catch (error) { setArchiveError(String(error)) }
    finally { setArchiveBusy(false) }
  }
  return <>
    <Button variant="outline" aria-label={translate("{value0} account {value1}", { value0: account.is_archived ? translate("Restore") : translate("Archive"), value1: account.name })} onClick={() => chooseArchive(account)}>{account.is_archived ? translate("Restore account") : translate("Archive account")}</Button>
    {archiveTarget && currency && <Dialog open onOpenChange={open => { if (!open && !archiveBusy) setArchiveTarget(null) }}>
      <DialogContent onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (archiveBusy) event.preventDefault() }} showCloseButton={!archiveBusy}>
        <DialogHeader><DialogTitle>{archiveTarget.is_archived ? translate("Restore") : translate("Archive")} {archiveTarget.name}?</DialogTitle><DialogDescription>
          {archiveTarget.is_archived ? translate("Restoring includes this balance in Net Worth and makes the account available for new records. Linked income and payment schedules may generate forecasts again.") : translate("Archiving excludes this balance from Net Worth, sidebar totals and generated forecasts, and prevents selecting this account for new records. This does not pay off debt or close the account with its provider.")}
        </DialogDescription></DialogHeader>
        <p className="text-sm">{['credit_card', 'loan'].includes(archiveTarget.type) ? translate("Outstanding balance (negative means credit)") : translate("Current balance")}: <strong>{formatAmount(archiveTarget.current_balance, currency)}</strong></p>
        <p className="text-sm text-muted">{translate("Balances, transactions, references, reconciliation history and shared credit-limit membership remain intact. Archived card balances still count toward shared availability and remain in Credit Cards Outlook. Explicit planner entries remain; review manual rows for duplicates.")}</p>
        {archiveTarget.type === 'loan' && <p className="text-sm text-muted">{translate("Mark as paid off is a separate action on the loan overview and requires zero balance.")}</p>}
        {archiveError && <p role="alert" className="text-sm text-red-600">{translate(archiveError)}</p>}
        <DialogFooter><Button variant="outline" disabled={archiveBusy} onClick={() => setArchiveTarget(null)}>{translate("Cancel")}</Button><Button disabled={archiveBusy} onClick={() => void saveArchive()}>{archiveBusy ? translate("Saving…") : archiveTarget.is_archived ? translate("Restore account") : translate("Archive account")}</Button></DialogFooter>
      </DialogContent>
    </Dialog>}
  </>
}
