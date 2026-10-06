import { invoke } from '@tauri-apps/api/core'
export interface PayrollDeduction {
  id: string; debt_account_id: string | null; amount: string; interest: string; fee: string; contract_id: string | null
}
export interface SalaryPayment {
  id: string; income_id: string; occurrence: string; date: string; gross: string; net: string; breakdown: string
}
export interface PayrollBreakdown extends PayrollDeduction { name: string; debt_account_name: string | null; principal: string }
export const listSalaryPayments = (incomeId: string | null, paymentId: string | null = null, accountId: string | null = null) => invoke<SalaryPayment[]>('list_salary_payments', { incomeId, paymentId, accountId })
function changed() { for (const name of ['accounts-changed', 'transactions-changed', 'incomes-changed', 'plans-changed']) window.dispatchEvent(new Event(name)) }
export async function recordSalaryPayment(input: { income_id: string; destination_account_id: string; occurrence: string; date: string; gross: string; currency: string; deductions: PayrollDeduction[]; confirmed: boolean }) {
  await invoke('record_salary_payment', { input }); changed()
}
export async function deleteSalaryPayment(id: string) { await invoke('delete_salary_payment', { id, confirmed: true }); changed() }
