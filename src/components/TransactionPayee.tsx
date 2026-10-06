import type { Transaction } from '../lib/desktop'
import { t, useLanguage } from '../lib/i18n'
import { AccountLabel } from './InstitutionLogo'
import { PayeeLabel } from './PayeeLogo'

export function TransactionPayee({ transaction }: { transaction: Transaction }) {
  useLanguage()
  if (transaction.payee_id) return <PayeeLabel id={transaction.payee_id} name={transaction.payee_name || t('Payee')} />
  if (transaction.loan_account_id) return <AccountLabel id={transaction.loan_account_id} name={transaction.loan_account_name || t('Loan account')} />
  return <>{transaction.type === 'expense' ? t('No payee') : '—'}</>
}
