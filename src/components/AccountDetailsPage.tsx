import { SelectiveDefaultSettings } from './SelectiveDefault'
import { useEffect, useState } from 'react'
import { Link, useParams } from '@tanstack/react-router'
import { t as translate, useLanguage } from '../lib/i18n'
import { desktopAvailable, getSettings, listAccounts, loanTypes, type Account } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { useCardLimits } from '../lib/card-limits'
import { AccountLabel } from './InstitutionLogo'
import { AccountFormDialog, accountTypes } from './AccountFormDialog'
import { AccountArchiveAction } from './AccountArchiveAction'
import { CardCredit } from './SharedCreditLimits'
import { Button } from './ui/button'
import { getLoanAccount, type LoanSnapshot } from '../lib/loans'
import { LoanAccountSummary } from './LoanAccountSummary'
import { MarkLoanPaidOffDialog } from './MarkLoanPaidOffDialog'

export function AccountDetailsPage() {
  const { accountId } = useParams({ from: '/accounts/$accountId/details' })
  return <AccountDetails key={accountId} accountId={accountId} />
}

function AccountDetails({ accountId }: { accountId: string }) {
  useLanguage()
  const limits = useCardLimits()
  const [account, setAccount] = useState<Account | null>(null)
  const [currency, setCurrency] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [editing, setEditing] = useState(false)
  const [loan, setLoan] = useState<LoanSnapshot | null>(null)
  const [markingPaidOff, setMarkingPaidOff] = useState(false)
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setError(false)
    Promise.all([getSettings(), listAccounts(true, true)]).then(async ([settings, accounts]) => {
      const found = accounts.find(a => a.id === accountId) ?? null
      const snapshot = found?.type === 'loan' ? await getLoanAccount(accountId) : null
      if (active) {
        setCurrency(snapshot ? snapshot.currency : settings.currency)
        setAccount(snapshot ? { ...snapshot.account, paid_off_on: snapshot.paid_off_on ?? null } : found)
        setLoan(snapshot)
      }
    }).catch(() => { if (active) setError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [accountId, attempt])
  useEffect(() => {
    const refresh = () => setAttempt(n => n + 1)
    const events = ['accounts-changed', 'transactions-changed', 'plans-changed', 'focus']
    events.forEach(event => window.addEventListener(event, refresh))
    return () => events.forEach(event => window.removeEventListener(event, refresh))
  }, [])
  if (!desktopAvailable) return <p>{translate("Open the desktop app to manage your local accounts.")}</p>
  if (loading && !account) return <p role="status">{translate("Loading accounts…")}</p>
  if (error) return <div role="alert"><p>{translate("Could not load accounts.")}</p><Button variant="outline" onClick={() => setAttempt(n => n + 1)}>{translate("Try again")}</Button></div>
  if (!account || !currency) return <div><p>{translate("Account unavailable")}</p><Link to="/accounts" className="text-brand">{translate("← Accounts")}</Link></div>
  const debt = ['loan', 'credit_card'].includes(account.type)
  const editable = !account.is_archived && !account.paid_off_on
  const details = [
    [translate("Type"), account.type === 'loan' ? loanTypes.find(t => t.value === account.loan_type)?.label ?? translate("Loan (unclassified)") : accountTypes.find(t => t.value === account.type)?.label],
    [debt ? translate("Outstanding balance") : translate("Current balance"), formatAmount(account.current_balance, currency)],
    ...(account.last_four ? [[translate("Last four digits"), `•••• ${account.last_four}`]] : []),
    ...(account.type === 'loan' ? [
      [translate("Initial Loan Amount"), account.initial_loan_amount != null ? formatAmount(account.initial_loan_amount, currency) : translate("Not set")],
      [translate("Monthly installment"), account.monthly_installment != null ? formatAmount(account.monthly_installment, currency) : translate("Not set")],
    ] : []),
    ...(account.statement_day != null ? [[translate("Statement day"), String(account.statement_day)]] : []),
    ...(account.payment_due_day != null ? [[translate("Payment due day"), String(account.payment_due_day)]] : []),
    ...(account.interest_rate_ten_thousandths != null ? [[translate("Annual interest rate"), `${interestRateText(String(account.interest_rate_ten_thousandths), 4)}%`]] : []),
  ]
  return <section className="space-y-6">
    <nav aria-label={translate("Breadcrumb")} className="pb-2">
      <Link to="/accounts" className="text-brand">{translate("← Accounts")}</Link>
    </nav>
    <div className="flex flex-wrap items-center justify-between gap-4">
      <div><h2 className="text-2xl font-semibold"><AccountLabel id={account.id} name={account.name} iconSize="lg" /></h2>{(account.paid_off_on || account.is_archived) && <p className="mt-1 text-sm text-muted">{account.paid_off_on ? `${translate("Paid off")} ${account.paid_off_on}` : translate("Archived")}</p>}</div>
      <div className="flex flex-wrap gap-2">
        {editable && <Button variant="outline" disabled={loading} onClick={() => setEditing(true)} aria-label={translate("Edit account {value0}", { value0: account.name })}>{translate("Edit account")}</Button>}
        <Button asChild variant="outline"><Link to="/transactions" search={{ account: account.id }}>{translate("Transaction history")}</Link></Button>
      </div>
    </div>
    <div className="rounded-2xl border border-line bg-card p-6">
      <dl className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">{details.map(([label, value]) => <div key={label}><dt className="text-sm text-muted">{label}</dt><dd className="mt-1 break-words font-medium tabular-nums">{value}</dd></div>)}</dl>
      {account.type === 'credit_card' && <CardCredit id={account.id} individualLimit={account.credit_limit} balance={account.current_balance} currency={currency} limits={limits} />}
      {account.notes && <p className="mt-5 break-words whitespace-pre-wrap text-sm">{account.notes}</p>}
    </div>
    {loan && <LoanAccountSummary data={loan} onSaved={() => setAttempt(n => n + 1)} />}
    {account.type !== 'loan' && <nav aria-label={translate("Account management")} className="flex flex-wrap gap-3">
      {['bank', 'wallet', 'credit_card'].includes(account.type) && <Button asChild variant="outline"><Link to="/accounts/$accountId" params={{ accountId }}>{translate("Transactions & reconciliation")}</Link></Button>}
      {account.type === 'credit_card' && <Button asChild variant="outline"><Link to="/accounts/$accountId/billing" params={{ accountId }}>{translate("Billing & payments")}</Link></Button>}
    </nav>}
    {['loan', 'credit_card'].includes(account.type) && <SelectiveDefaultSettings account={account} />}
    {!account.paid_off_on && <section className="rounded-2xl border border-line p-6">
      <h3 className="mb-3 font-semibold">{translate("Account status")}</h3>
      <div className="flex flex-wrap gap-3">
      {account.type === 'loan' && editable && <Button variant="outline" disabled={loading} onClick={() => setMarkingPaidOff(true)}>{translate("Mark as paid off")}</Button>}
      <AccountArchiveAction account={account} currency={currency} />
      </div>
    </section>}
    {editing && <AccountFormDialog account={account} initialType={account.type} currency={currency} onClose={() => setEditing(false)} onSaved={() => setAttempt(n => n + 1)} />}
    {markingPaidOff && <MarkLoanPaidOffDialog account={account} currency={currency} onClose={() => setMarkingPaidOff(false)} />}
  </section>
}
