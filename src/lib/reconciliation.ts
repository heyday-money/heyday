import { invoke } from '@tauri-apps/api/core'
import type { TransactionType } from './desktop'

export type VerificationStatus = 'uncleared' | 'cleared' | 'reconciled'
export interface VerificationEntry {
  transaction_id: string; date: string; description: string; type: TransactionType; amount: string
  account_id: string; destination_account_id: string | null; status: VerificationStatus
  reconciliation_id: string | null; balance_change: string
}
export interface ReconciliationHistory {
  id: string; completed_at: string; confirmed_balance: string; opening_balance: string; needs_review: boolean
}
export interface ReconciliationHistoryEntry {
  reconciliation_id: string; transaction_id: string; date: string; description: string; type: TransactionType; balance_change: string
}
export interface ReconciliationSnapshot {
  account_id: string; name: string; account_type: 'bank' | 'wallet' | 'credit_card'; is_archived: boolean; currency: string
  opening_balance: string; working_balance: string; cleared_balance: string; uncleared_changes: string
  entries: VerificationEntry[]; history: ReconciliationHistory[]; history_entries: ReconciliationHistoryEntry[]; token: string
}
export const getReconciliation = (accountId: string) => invoke<ReconciliationSnapshot>('get_reconciliation', { accountId })
export async function setVerification(accountId: string, entries: VerificationEntry[], status: 'uncleared' | 'cleared', confirmUnlock = false) {
  await invoke('set_transaction_verification', { input: { account_id: accountId, entries: entries.map(entry => ({ transaction_id: entry.transaction_id, expected_status: entry.status })), status, confirm_unlock: confirmUnlock } })
  window.dispatchEvent(new Event('verification-changed'))
}
export async function finishReconciliation(snapshot: ReconciliationSnapshot, postedBalance: string, confirmOpeningBalance: boolean) {
  const result = await invoke<ReconciliationSnapshot>('finish_reconciliation', { input: { account_id: snapshot.account_id, posted_balance: postedBalance, expected_token: snapshot.token, confirm_opening_balance: confirmOpeningBalance } })
  window.dispatchEvent(new Event('verification-changed'))
  return result
}
