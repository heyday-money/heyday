import { useRef, useState } from 'react'
import { toast } from 'sonner'
import { markLoanPaidOff, type Account } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { Button } from './ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

export function MarkLoanPaidOffDialog({ account, currency, onClose }: { account: Account; currency: string; onClose: () => void }) {
  const [busy, setBusy] = useState(false)
  const lock = useRef(false)
  const [error, setError] = useState<string | null>(null)
  const zero = BigInt(account.current_balance ?? account.opening_balance) === 0n
  async function confirm() {
    if (lock.current || !zero) return
    lock.current = true; setBusy(true); setError(null)
    try { await markLoanPaidOff(account.id); toast.success('Loan marked as paid off.'); onClose() }
    catch (error) { setError(error instanceof Error ? error.message : String(error)) }
    finally { lock.current = false; setBusy(false) }
  }
  return <Dialog open onOpenChange={open => { if (!open && !lock.current) onClose() }}>
    <DialogContent showCloseButton={!busy} onInteractOutside={event => event.preventDefault()} onEscapeKeyDown={event => { if (lock.current) event.preventDefault() }}>
      <DialogHeader><DialogTitle>Mark loan as paid off?</DialogTitle><DialogDescription>Archive “{account.name}” and stop its linked repayment forecasts. Transactions, contracts, and saved planner entries are preserved.</DialogDescription></DialogHeader>
      <p className="text-sm">Current balance: {formatAmount(account.current_balance ?? account.opening_balance, currency)}</p>
      {!zero && <p role="alert" className="text-sm">The balance must be zero. Record the final repayment or resolve any credit first.</p>}
      <p className="text-sm">Any remaining contract principal must be resolved. Remove linked payroll deductions in Income first, and review unrelated manual Outlook rows for duplicate payments.</p>
      <p className="text-xs text-muted">The loan stays available under Paid-off loans. Linked forecasts are excluded from this payday cycle onward; past entered figures remain. This does not close an account with your lender. A transaction correction that restores a balance will reopen the loan in the app.</p>
      {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{error}</p>}
      <DialogFooter><Button variant="outline" disabled={busy} onClick={onClose}>Cancel</Button><Button disabled={busy || !zero} onClick={() => void confirm()}>{busy ? 'Saving…' : 'Mark as paid off'}</Button></DialogFooter>
    </DialogContent>
  </Dialog>
}
