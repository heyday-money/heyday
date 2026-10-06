import type { Transaction } from './desktop'

// Loan recipients are account references, not synthetic records in the payee list.
// Keep explicit payees and loan accounts distinct even when their names match.
export function transactionPayeeKey(transaction: Transaction): string | null {
  if (transaction.payee_id) return transaction.payee_id
  return transaction.loan_account_id ? `loan:${transaction.loan_account_id}` : null
}
