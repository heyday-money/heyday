import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { t, useLanguage } from '../lib/i18n'
import { loanTotals, type LoanSnapshot } from '../lib/loans'
import { formatAmount } from '../lib/money'
import { LoanEditor } from './LoanEditor'
import { Button } from './ui/button'

export function LoanAccountSummary({ data, onSaved }: { data: LoanSnapshot; onSaved: () => void }) {
  useLanguage()
  const [editing, setEditing] = useState(false)
  const totals = loanTotals(data)
  const hasBorrowings = !!data.facility || data.contracts.length > 0
  const editable = !data.account.is_archived && !data.paid_off_on
  if (!hasBorrowings && (data.account.loan_type !== 'personal_loan' || !editable)) return null
  const figures: [string, bigint][] = [
    [t('Assigned outstanding principal'), totals.principal],
    [t('Unassigned debt / credit'), totals.unassigned],
    ...(data.facility ? [[t('Credit Limit'), BigInt(data.facility.credit_limit)], [t('Estimated available credit'), totals.available!]] as [string, bigint][] : []),
  ]
  return <section className="space-y-4 rounded-2xl border border-line bg-card p-6">
    <h3 className="font-semibold">{t('Revolving credit / cash card')}</h3>
    {hasBorrowings && <>
      <dl className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">{figures.map(([label, value]) => <div key={label}>
        <dt className="text-sm text-muted">{label}</dt>
        <dd className="mt-1 break-words font-semibold tabular-nums">{data.currency ? formatAmount(value.toString(), data.currency) : '—'}</dd>
      </div>)}</dl>
      <p className="text-sm">{t('Available credit uses current debt, including unassigned debt, conservatively. Your lender may apply other charges or holds. Allocate only outstanding principal already included in this account; never count the same borrowing twice.')}</p>
      <p className="text-sm">{t('Contract schedules replace this account’s monthly installment and legacy loan schedule forecasts. Saved cycle overrides remain on the account row. Review manual plans and payroll deductions for duplicates. Interest and fees paid with contract repayments are cash expenses; accrued interest is not calculated automatically.')}</p>
    </>}
    <div className="flex flex-wrap gap-3">
      {editable && data.account.loan_type === 'personal_loan' && <Button variant="outline" onClick={() => setEditing(true)}>{data.facility ? t('Edit shared credit limit') : t('Set up revolving credit / cash card')}</Button>}
      {hasBorrowings && <Button asChild variant="outline"><Link to="/accounts/$accountId/loans" params={{ accountId: data.account.id }}>{t('Borrowings & schedules')}</Link></Button>}
    </div>
    {editing && data.currency && <LoanEditor editor={{ kind: 'facility' }} data={data} accounts={[]} close={() => setEditing(false)} saved={() => { setEditing(false); onSaved() }} />}
  </section>
}
