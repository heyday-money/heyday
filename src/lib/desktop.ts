import { invoke, isTauri } from '@tauri-apps/api/core'

export interface Settings {
  currency: string | null
  period_start_day: number
}

export const desktopAvailable = isTauri()

export function getSettings(): Promise<Settings> {
  return invoke<Settings>('get_settings')
}

export function updatePeriod(day: number): Promise<Settings> {
  return invoke<Settings>('update_period', { day })
}

export function updateCurrency(currency: string): Promise<Settings> {
  return invoke<Settings>('update_currency', { currency })
}

export type AccountType = 'cash' | 'bank' | 'wallet' | 'credit_card' | 'loan' | 'investment'
export const loanTypes = [
  { value: 'mortgage', label: 'Mortgage' },
  { value: 'auto_loan', label: 'Auto Loan' },
  { value: 'student_loan', label: 'Student Loan' },
  { value: 'personal_loan', label: 'Personal Loan' },
  { value: 'medical_debt', label: 'Medical Debt' },
  { value: 'other_debt', label: 'Other Debt' },
] as const
export type LoanType = typeof loanTypes[number]['value']
export interface Account {
  id: string
  name: string
  type: AccountType
  loan_type: LoanType | null
  current_balance: string
  opening_balance: string
  institution: string | null
  last_four: string | null
  notes: string | null
  credit_limit: string | null
  statement_day: number | null
  payment_due_day: number | null
  interest_rate_ten_thousandths: number | null
  monthly_installment: string | null
}
export type NewAccount = Omit<Account, 'id' | 'current_balance'> & { currency: string }

export function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>('list_accounts')
}
export type AccountUpdate = Omit<NewAccount, 'opening_balance'> & { id: string }
export async function updateAccount(input: AccountUpdate): Promise<Account> {
  const account = await invoke<Account>('update_account', { input })
  window.dispatchEvent(new Event('accounts-changed'))
  return account
}
export async function createAccount(input: NewAccount): Promise<Account> {
  const account = await invoke<Account>('create_account', { input })
  window.dispatchEvent(new Event('accounts-changed'))
  return account
}

export type IncomeType = 'salary' | 'variable' | 'investment' | 'other'
export interface Income {
  id: string
  name: string
  destination_account_id: string
  destination_account_name: string
  type: IncomeType
  estimated_amount: string
  deductions_total: string
  recurrence_frequency: 'monthly'
  recurrence_day_of_month: number
  is_auto_create_transaction: boolean
  is_active: boolean
  created_at: string
  updated_at: string
}
export type NewIncome = Omit<Income, 'id' | 'destination_account_name' | 'created_at' | 'updated_at' | 'deductions_total'> & { currency: string; deductions?: IncomeDeductionInput[] }

export function listIncomes(): Promise<Income[]> {
  return invoke<Income[]>('list_incomes')
}
export async function createIncome(input: NewIncome): Promise<Income> {
  const income = await invoke<Income>('create_income', { input })
  window.dispatchEvent(new Event('incomes-changed'))
  return income
}
export interface IncomeDeduction { id: string; income_id: string; name: string; description: string; amount: string; debt_account_id?: string | null; debt_account_name?: string | null }
export type IncomeDeductionInput = Omit<IncomeDeduction, 'income_id' | 'id' | 'debt_account_name'> & { id: string | null }
export const listIncomeDeductions = (incomeId: string) => invoke<IncomeDeduction[]>('list_income_deductions', { incomeId })
export async function saveSalaryDeductions(input: { income_id: string; currency: string; deductions: IncomeDeductionInput[] }) {
  const income = await invoke<Income>('save_salary_deductions', { input })
  window.dispatchEvent(new Event('incomes-changed'))
  return income
}

export type TransactionType = 'income' | 'expense' | 'transfer' | 'repayment'
export interface Transaction {
  has_reconciliation_history?: boolean
  id: string
  type: TransactionType
  account_id: string
  account_name: string
  destination_account_id: string | null
  destination_account_name: string | null
  amount: string
  date: string
  description: string
  payee_id: string | null
  payee_name: string | null
  category_id: string | null
  category_name: string | null
}
export type NewTransaction = Omit<Transaction, 'id' | 'account_name' | 'destination_account_name' | 'payee_name' | 'category_name' | 'has_reconciliation_history'> & { currency: string; cleared_account_ids?: string[] }
export function listTransactions(): Promise<Transaction[]> { return invoke('list_transactions') }
export async function createTransaction(input: NewTransaction): Promise<Transaction> {
  const result = await invoke<Transaction>('create_transaction', { input })
  window.dispatchEvent(new Event('transactions-changed'))
  window.dispatchEvent(new Event('accounts-changed'))
  return result
}
export async function deleteTransaction(id: string, confirmReconciled = false): Promise<void> {
  await invoke('delete_transaction', { id, confirmReconciled })
  window.dispatchEvent(new Event('transactions-changed'))
  window.dispatchEvent(new Event('accounts-changed'))
}

export type TransactionOptionKind = 'payee' | 'category'
export interface TransactionOption { id: string; name: string; is_archived: boolean }
export interface TransactionOptions { payees: TransactionOption[]; categories: TransactionOption[] }
export interface SaveTransactionOption { kind: TransactionOptionKind; id: string | null; name: string; is_archived: boolean }
export function listTransactionOptions(): Promise<TransactionOptions> { return invoke('list_transaction_options') }
export async function saveTransactionOption(input: SaveTransactionOption): Promise<TransactionOption> {
  const option = await invoke<TransactionOption>('save_transaction_option', { input })
  window.dispatchEvent(new Event('transaction-options-changed'))
  return option
}

export interface PaymentPlan {
  id: string
  name: string
  type: 'expense' | 'repayment'
  account_id: string
  account_name: string
  destination_account_id: string | null
  destination_account_name: string | null
  category_id: string | null
  category_name: string | null
  amount: string
  date: string
}
export type SavePaymentPlan = Omit<PaymentPlan, 'id' | 'account_name' | 'destination_account_name' | 'category_name'> & { id: string | null; currency: string }
export interface FinancialData {
  settings: Settings
  accounts: Account[]
  incomes: Income[]
  transactions: Transaction[]
  plans: PaymentPlan[]
  installments: Installment[]
  subscriptions: Subscription[]
  categories: TransactionOption[]
}
export function getFinancialData(): Promise<FinancialData> { return invoke('get_financial_data') }
export async function savePaymentPlan(input: SavePaymentPlan): Promise<PaymentPlan> {
  const plan = await invoke<PaymentPlan>('save_payment_plan', { input })
  window.dispatchEvent(new Event('plans-changed'))
  return plan
}
export async function deletePaymentPlan(id: string): Promise<void> {
  await invoke('delete_payment_plan', { id })
  window.dispatchEvent(new Event('plans-changed'))
}

export interface Installment {
  id: string
  name: string
  account_id: string
  account_name: string
  debt_account_id: string
  debt_account_name: string
  debt_account_type: AccountType
  monthly_amount: string
  installment_count: number
  interest_rate_millis: string | null
  first_due_date: string
  purchase_kind: 'existing_purchase' | 'new_purchase'
  purchase_transaction_id: string | null
}
export type SaveInstallment = Omit<Installment, 'id' | 'account_name' | 'debt_account_name' | 'debt_account_type' | 'purchase_kind' | 'purchase_transaction_id'> & { id: string | null; currency: string; purchase: { amount: string; date: string } | null }
export async function saveInstallment(input: SaveInstallment): Promise<void> {
  await invoke('save_installment', { input })
  if (input.purchase) {
    window.dispatchEvent(new Event('accounts-changed'))
    window.dispatchEvent(new Event('transactions-changed'))
  }
  window.dispatchEvent(new Event('plans-changed'))
}
export async function deleteInstallment(id: string): Promise<void> {
  await invoke('delete_installment', { id })
  window.dispatchEvent(new Event('plans-changed'))
}

export interface Subscription {
  id: string
  name: string
  account_id: string
  account_name: string
  account_type: AccountType
  category_id: string | null
  category_name: string | null
  amount: string
  frequency: 'monthly' | 'yearly'
  first_billing_date: string
  end_date: string | null
  is_active: boolean
}
export type SaveSubscription = Omit<Subscription, 'id' | 'account_name' | 'account_type' | 'category_name'> & { id: string | null; currency: string }
export async function saveSubscription(input: SaveSubscription): Promise<void> {
  await invoke('save_subscription', { input })
  window.dispatchEvent(new Event('plans-changed'))
}
export async function deleteSubscription(id: string): Promise<void> {
  await invoke('delete_subscription', { id })
  window.dispatchEvent(new Event('plans-changed'))
}

export function clearAllData(confirmation: string): Promise<void> {
  return invoke<void>('clear_all_data', { confirmation })
}
