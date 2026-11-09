import { useEffect, useState } from 'react'
import { getSettings, type SyncMetadata } from '../lib/desktop'
import { t } from '../lib/i18n'
import { formatAmount } from '../lib/money'

export function metadataTitle(kind: string) {
  const labels: Record<string, string> = {
    logo: t('Custom logo'), institution: t('Institution'), payee: t('Payee'), provider: t('Subscription provider'),
    'account-logo': t('Cash account logo'), 'account-link': t('Account institution'), facility: t('Loan facility'), contract: t('Loan contract'), installment: t('Installment'),
    subscription: t('Subscription'), 'payment-plan': t('Payment plan'), 'planner-item': t('Planner item'),
    'planner-amount': t('Planner amount'), 'debt-amount': t('Debt forecast'), 'installment-amount': t('Installment forecast'),
    cycle: t('Cycle status'), opening: t('Opening cash'), settings: t('Financial settings'), 'card-limit': t('Shared credit limit'),
    statement: t('Card statement'), 'default-history': t('Selective Default'), payoff: t('Loan payoff'),
    reconciliation: t('Reconciliation history'), verification: t('Transaction verification'),
  }
  return labels[kind] ?? t('Financial record')
}
const monetary = new Set(['amount', 'monthly_amount', 'schedule_amount', 'credit_limit', 'principal', 'basis_principal', 'target', 'minimum', 'confirmed_balance', 'opening_balance', 'balance_change'])
export function MetadataDetails({ value, accounts }: { value: SyncMetadata; accounts: Record<string, string> }) {
  const [currency, setCurrency] = useState<string | null>(null)
  useEffect(() => { let active = true; void getSettings().then(s => { if (active) setCurrency(s.currency) }).catch(() => {}); return () => { active = false } }, [])
  const labels: Record<string, string> = {
    name: t('Name'), account_id: t('Account'), receiving_account_id: t('Receiving account'), payment_account_id: t('Payment account'), debt_account_id: t('Debt account'), destination_account_id: t('Destination account'),
    amount: t('Amount'), monthly_amount: t('Monthly amount'), schedule_amount: t('Scheduled amount'), credit_limit: t('Credit limit'), principal: t('Principal'), basis_principal: t('Principal before recorded repayments'),
    target: t('Payment target'), minimum: t('Minimum payment'), confirmed_balance: t('Confirmed balance'), opening_balance: t('Opening balance'), balance_change: t('Balance change'),
    date: t('Date'), month: t('Forecast cycle'), start_date: t('Start date'), end_date: t('End date'), due_date: t('Due date'), first_due_date: t('First due date'), first_billing_date: t('First billing date'), borrowing_date: t('Borrowing date'), schedule_start: t('Schedule start'), schedule_end: t('Schedule end'), start_month: t('Start cycle'), end_month: t('End cycle'), completed_at: t('Completed at'), paid_off_on: t('Paid off on'),
    description: t('Description'), notes: t('Notes'), card_name: t('Card name'), status: t('Status'), type: t('Type'), mode: t('Payment mode'), currency: t('Currency'), period_start_day: t('Period start day'),
    is_archived: t('Archived'), is_active: t('Active'), needs_review: t('Needs review'), frequency: t('Frequency'), installment_count: t('Number of installments'), interest_rate: t('Annual interest'), interest_rate_millis: t('Annual interest'),
    managed_via: t('Managed through'), management_url: t('Management link'), logo_mode: t('Logo'), short_name: t('Short name'), bank_code: t('Bank code'), swift_code: t('SWIFT code'), borrowing_kind: t('Borrowing type'), purchase_kind: t('Purchase type'),
    institution_id: t('Institution reference'), provider_id: t('Provider reference'), category_id: t('Category reference'), transaction_category_id: t('Category reference'), transaction_id: t('Transaction reference'), reconciliation_id: t('Reconciliation reference'), installment_id: t('Installment reference'), borrowing_transaction_id: t('Borrowing transaction'), purchase_transaction_id: t('Purchase transaction'), item_id: t('Planner item reference'),
  }
  const values: Record<string, string> = { cleared: t('Cleared'), uncleared: t('Uncleared'), reconciled: t('Reconciled'), monthly: t('Monthly'), yearly: t('Yearly'), tracking: t('Tracking'), forecast: t('Forecast'), complete: t('Complete'), full: t('Full balance'), minimum: t('Minimum payment'), custom: t('Custom'), default: t('Default'), none: t('None'), new: t('New borrowing'), existing: t('Already recorded borrowing'), income: t('Income'), expense: t('Expense'), transfer: t('Transfer'), repayment: t('Repayment') }
  const amountCurrency = ['planner-item', 'planner-amount', 'debt-amount', 'installment-amount', 'opening'].includes(value.kind) ? 'THB' : currency
  function rows(fields: Record<string, string | null>) {
    return <dl className="space-y-2">{Object.entries(fields).filter(([key]) => labels[key]).map(([key, raw]) => {
      const detail = raw === null ? '—' : monetary.has(key) ? amountCurrency ? formatAmount(raw, amountCurrency) : t('Exact minor units: {amount}', { amount: raw }) : ['is_archived', 'is_active', 'needs_review'].includes(key) ? raw === '1' ? t('Yes') : t('No') : key.endsWith('account_id') ? accounts[raw] ?? t('Account unavailable') : key === 'interest_rate' ? `${Number(raw) / 10000}%` : key === 'interest_rate_millis' ? `${Number(raw) / 1000}%` : values[raw] ?? raw
      return <div key={key}><dt className="text-xs text-muted">{labels[key]}</dt><dd className="break-words tabular-nums">{detail}</dd></div>
    })}</dl>
  }
  const children: Record<string, string> = { members: t('Cards'), entries: t('Recorded entries'), installments: t('Included installments'), allocations: t('Allocated payments'), plan: t('Payment plan'), intervals: t('Default periods') }
  return <div className="mt-3 space-y-3 text-sm"><p className="font-medium">{metadataTitle(value.kind)}</p>{value.kind === 'logo' && value.fields.data && <img src={`data:image/png;base64,${value.fields.data}`} width={Number(value.fields.width)} height={Number(value.fields.height)} alt={t('Custom logo')} className="size-16 object-contain" />}{rows(value.fields)}{Object.entries(value.children).map(([kind, records]) => <details key={kind} className="border-t border-line pt-2"><summary className="cursor-pointer">{children[kind] ?? t('Related records')} ({records.length})</summary><div className="max-h-72 space-y-3 overflow-auto pt-2">{records.map((record, i) => <div key={i} className="border-b border-line pb-2">{rows(record)}</div>)}</div></details>)}</div>
}
