import { invoke } from '@tauri-apps/api/core'
import type { Account, Transaction } from './desktop'
import { installmentSchedule } from './installments'
export interface LoanFacility { account_id: string; credit_limit: string }
export interface LoanContract {
 id: string; account_id: string; name: string; principal: string; remaining_principal: string
 borrowing_date: string; receiving_account_id: string; payment_account_id: string
 interest_rate: number; monthly_amount: string; installment_count: number; first_due_date: string
 schedule_end: string | null; notes: string; borrowing_kind: 'new' | 'existing'
 borrowing_transaction_id: string | null; needs_review: boolean; accounts_available: boolean
}
export interface LoanSnapshot {
 paid_off_on?: string | null
 account: Account; currency: string | null; facility: LoanFacility | null; contracts: LoanContract[]
 transactions: Transaction[]; payment_parts: { transaction_id: string; payment_id: string; contract_id: string; component: string; amount: string }[]
}
export type SaveLoanContract = Omit<LoanContract, 'id' | 'borrowing_transaction_id' | 'needs_review' | 'accounts_available'> & { id: string | null; currency: string; acknowledge_over_limit: boolean }
export interface LoanRepayment { contract_id: string; account_id: string; date: string; total: string; principal: string; interest: string; fee: string; currency: string }
export const getLoanAccount = (accountId: string) => invoke<LoanSnapshot>('get_loan_account', { accountId })
async function mutate(command: string, args: Record<string, unknown>) {
 await invoke(command,args)
 for (const event of ['accounts-changed','transactions-changed','plans-changed']) window.dispatchEvent(new Event(event))
}
export const saveLoanFacility = (accountId: string, creditLimit: string, currencyCode: string) => mutate('save_loan_facility',{accountId,creditLimit,currencyCode})
export const saveLoanContract = (input: SaveLoanContract) => mutate('save_loan_contract',{input})
export const recordLoanRepayment = (input: LoanRepayment) => mutate('record_loan_repayment',{input})
export const deleteLoanContract = (id: string) => mutate('delete_loan_contract',{id})
export function loanSchedule(contract: LoanContract) {
 if (contract.needs_review || !contract.accounts_available) return []
 return installmentSchedule(contract).filter(p => !contract.schedule_end || p.date <= contract.schedule_end)
}
export function loanTotals(data: LoanSnapshot) {
 const principal = data.contracts.reduce((n,c) => n+BigInt(c.remaining_principal),0n)
 const debt=BigInt(data.account.current_balance)
 const unassigned=debt-principal
 // Unassigned debt conservatively uses credit until the user classifies it.
 const used=debt>0n?debt:0n
 return {principal,unassigned,available:data.facility?BigInt(data.facility.credit_limit)-used:null}
}
