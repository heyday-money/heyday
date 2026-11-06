import { getLanguage, t } from './i18n'
import type { LogoChange } from './logos'
import type { CardBillingData } from './card-billing'
import { invoke, isTauri } from '@tauri-apps/api/core'

export interface Settings {
  currency: string | null
  period_start_day: number
}

export const desktopAvailable = isTauri()

export interface CloudSubscription {
  entitlement: { canSync: boolean; reason: string; accessUntil: string | null }
  billing?: { enabled: boolean; subscriptions: { status: string; periodEnd: string | null; cancelAt: string | null }[] } | null
}
export interface CloudStatus { account: { id: string; email: string } | null; online: boolean; deviceRegistered?: boolean; deviceError?: string | null; subscription?: CloudSubscription | null }
let pendingCloudAccount: Promise<CloudStatus> | undefined
export function cloudAccount(): Promise<CloudStatus> {
  // Share overlapping mount checks (including React StrictMode) without caching
  // credentials or preventing an explicit retry after the request settles.
  return pendingCloudAccount ??= invoke<CloudStatus>('cloud_account')
    .finally(() => { pendingCloudAccount = undefined })
}
export function cloudSignIn(): Promise<CloudStatus> { return invoke('cloud_sign_in', { language: getLanguage() }) }
export function cloudCancelSignIn(): Promise<void> { return invoke('cloud_cancel_sign_in') }
export function cloudSignOut(forgetDevice = false): Promise<boolean> { return invoke('cloud_sign_out', { forgetDevice }) }
export function cloudManageSubscription(): Promise<void> { return invoke('cloud_manage_subscription') }
export interface CloudDevice {
  id: string; name: string; platform: 'macos' | 'windows' | 'linux';
  createdAt: string; lastSeenAt: string; revokedAt: string | null; current: boolean
}
export interface CloudDeviceView { currentDevice: CloudDevice | null; savedName: string | null; page: { devices: CloudDevice[]; nextCursor: string | null } }
export function cloudDevices(cursor: string | null = null): Promise<CloudDeviceView> { return invoke('cloud_devices', { cursor }) }
export function cloudRegisterDevice(name: string): Promise<CloudDevice> { return invoke('cloud_register_device', { name }) }
export function cloudRevokeDevice(deviceId: string): Promise<{ current: boolean; credentialsCleared: boolean }> { return invoke('cloud_revoke_device', { deviceId }) }
export interface EncryptionStatus { state: 'not_set' | 'pending' | 'locked' | 'unlocked' | 'changed' | 'rotating'; rememberEncryption?: boolean; historyVerified?: boolean; rotationPending?: boolean; unverifiedHistory?: boolean }
export function cloudEncryptionStatus(): Promise<EncryptionStatus> { return invoke('cloud_encryption_status') }
export function cloudPrepareEncryption(): Promise<{ recoveryKey: string }> { return invoke('cloud_prepare_encryption') }
export function cloudConfirmEncryption(recoveryKey: string, saved: boolean, remember: boolean): Promise<EncryptionStatus> { return invoke('cloud_confirm_encryption', { recoveryKey, saved, remember }) }
export function cloudRecoverEncryption(recoveryKey: string, remember: boolean, historyCode?: string, allowUnverifiedHistory?: boolean): Promise<EncryptionStatus> { return invoke('cloud_recover_encryption', { recoveryKey, remember, ...(historyCode ? { historyCode } : {}), ...(allowUnverifiedHistory ? { allowUnverifiedHistory: true } : {}) }) }
export function cloudPrepareKeyRotation(): Promise<{ recoveryKey: string }> { return invoke('cloud_prepare_key_rotation') }
export function cloudConfirmKeyRotation(recoveryKey: string, saved: boolean): Promise<EncryptionStatus> { return invoke('cloud_confirm_key_rotation', { recoveryKey, saved }) }
export function cloudCancelKeyRotation(): Promise<EncryptionStatus> { return invoke('cloud_cancel_key_rotation') }
export function cloudHistoryVerificationCode(): Promise<string> { return invoke('cloud_history_verification_code') }
export function cloudRememberEncryption(remember: boolean): Promise<EncryptionStatus> { return invoke('cloud_remember_encryption', { remember }) }
export function cloudLockEncryption(): Promise<void> { return invoke('cloud_lock_encryption') }
export function cloudCancelEncryption(): Promise<void> { return invoke('cloud_cancel_encryption') }
export function cloudSaveRecoveryKey(): Promise<boolean> { return invoke('cloud_save_recovery_key') }

export interface SyncCategory { name: string; icon: string; isArchived: boolean }
export type SyncAccount = Omit<Account, 'id' | 'current_balance' | 'paid_off_on' | 'payroll_linked'> & { currency: string }
export interface SyncLedgerEntry { id: string; type: TransactionType; account_id: string; destination_account_id: string | null; amount: string; date: string; description: string; payee: { id: string; name: string } | null; category_id: string | null; income_source: { id: string; name: string } | null; loan_account_id: string | null; salary_role: string | null; loan_component: string | null }
export interface SyncPayroll { id: string; income_id: string; occurrence: string; date: string; gross: string; net: string; breakdown: import('./salary-payments').PayrollBreakdown[]; principal_entries: { deduction_id: string; transaction_id: string }[] }
export interface SyncIncome { currency: string; name: string; type: IncomeType; destination_account_id: string; estimated_amount: string; recurrence_frequency: string; recurrence_day_of_month: number; is_active: boolean; deductions: { id: string; name: string; description: string; amount: string; debt_account_id: string | null }[] }
export interface SyncLedgerGroup { currency: string; protected: boolean; entries: SyncLedgerEntry[]; payroll?: SyncPayroll; loan?: { id: string; loan_account_id: string; contract_id: string | null; parts: { transaction_id: string; component: string; amount: string }[] }; links?: { borrowing_contract_id: string | null; installment_id: string | null } }
export interface SyncPlannerOverride { source_type: 'income' | 'deduction'; source_id: string; month: string; currency: 'THB'; amount: string }
export interface SyncMetadata { kind: string; fields: Record<string, string | null>; children: Record<string, Record<string, string | null>[]> }
export type SyncDetails = SyncCategory | SyncAccount | SyncLedgerGroup | SyncIncome | SyncPlannerOverride | SyncMetadata
export interface SyncConflict { entity?: 'category' | 'account' | 'transaction' | 'income' | 'income-plan' | 'metadata'; localDetails?: SyncDetails | null; remoteDetails?: SyncDetails | null; recordId: string; local: SyncCategory | null; remote: SyncCategory | null; localRevision: string; remoteVersion: string }
export interface CloudSyncStatus { fullEnabled: boolean; plannerEnabled: boolean; incomeEnabled: boolean; financialEnabled: boolean; bound: boolean; enabled: boolean; bootstrapped: boolean; pending: number; conflictCount: number; lastSuccess: string | null; conflicts: SyncConflict[] }
let pendingSyncStatus: Promise<CloudSyncStatus> | undefined
export function cloudSyncStatus(): Promise<CloudSyncStatus> { return pendingSyncStatus ??= invoke<CloudSyncStatus>('cloud_sync_status').finally(() => { pendingSyncStatus = undefined }) }
export function cloudEnableSync(): Promise<CloudSyncStatus> { return invoke('cloud_enable_sync', { confirmed: true }) }
export function cloudSyncNow(): Promise<CloudSyncStatus> { return invoke('cloud_sync_now') }
export function cloudPauseSync(): Promise<CloudSyncStatus> { return invoke('cloud_pause_sync') }
export function cloudResolveSync(conflict: SyncConflict, choice: 'local' | 'remote' | 'rename', name?: string): Promise<CloudSyncStatus> { return invoke('cloud_resolve_sync', { recordId: conflict.recordId, localRevision: conflict.localRevision, remoteVersion: conflict.remoteVersion, choice, name: name ?? null }) }

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
  { value: 'mortgage', get label() { return t('Mortgage') } },
  { value: 'auto_loan', get label() { return t('Auto Loan') } },
  { value: 'student_loan', get label() { return t('Student Loan') } },
  { value: 'personal_loan', get label() { return t('Personal Loan') } },
  { value: 'medical_debt', get label() { return t('Medical Debt') } },
  { value: 'other_debt', get label() { return t('Other Debt') } },
] as const
export type LoanType = typeof loanTypes[number]['value']
export interface Account {
  payroll_linked?: boolean
  is_archived?: boolean
  paid_off_on?: string | null
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
  initial_loan_amount?: string | null
}
import type { LoanContract, LoanFacility } from './loans'
export type NewAccount = Omit<Account, 'payroll_linked' | 'id' | 'current_balance' | 'paid_off_on' | 'is_archived'> & { currency: string; revolving_credit_limit?: string | null; institution_id?: string | null }

export function listAccounts(includePaidOff = false, includeArchived = false): Promise<Account[]> {
  return invoke<Account[]>('list_accounts', { includePaidOff, includeArchived })
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
export type UpdateIncome = Omit<NewIncome, 'deductions'> & { id: string }
export async function updateIncome(input: UpdateIncome): Promise<Income> {
  const income = await invoke<Income>('update_income', { input })
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
  imported_linked_payment?: boolean
  salary_payment_id?: string | null
  salary_payment_role?: string | null
  loan_account_id?: string | null
  loan_account_name?: string | null
  income_source_id?: string | null
  income_source_name?: string | null
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
  category_icon?: string | null
  category_name: string | null
}
export type NewTransaction = Omit<Transaction, 'imported_linked_payment' | 'loan_account_id' | 'loan_account_name' | 'category_icon' | 'id' | 'income_source_name' | 'account_name' | 'destination_account_name' | 'payee_name' | 'category_name' | 'has_reconciliation_history'> & { currency: string; cleared_account_ids?: string[] }
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
export interface TransactionOption { logo_asset_id?: string | null; icon?: string | null; id: string; name: string; is_archived: boolean }
export interface TransactionOptions { payees: TransactionOption[]; categories: TransactionOption[] }
export interface SaveTransactionOption { logo_change?: LogoChange; icon?: string | null; kind: TransactionOptionKind; id: string | null; name: string; is_archived: boolean }
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
  income_deductions?: IncomeDeduction[]
  selective_defaults?: import('./selective-defaults').SelectiveDefault[]
  card_billing?: CardBillingData
  loan_contracts?: LoanContract[]
  loan_facilities?: LoanFacility[]
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

export type SubscriptionPlatform = 'apple_app_store' | 'google_play' | 'website' | 'in_app' | 'other'
export interface Subscription {
  provider_id?: string | null
  provider_icon?: string | null
  logo_asset_id?: string | null
  managed_via?: SubscriptionPlatform | null
  management_url?: string | null
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
export type SaveSubscription = Omit<Subscription, 'id' | 'account_name' | 'account_type' | 'category_name' | 'logo_asset_id' | 'provider_icon'> & { id: string | null; currency: string; logo_change?: LogoChange }
export async function saveSubscription(input: SaveSubscription): Promise<void> {
  await invoke('save_subscription', { input })
  window.dispatchEvent(new Event('plans-changed'))
}
export const openSubscriptionManagement = (id: string) => invoke<void>('open_subscription_management', { id })
export async function deleteSubscription(id: string): Promise<void> {
  await invoke('delete_subscription', { id })
  window.dispatchEvent(new Event('plans-changed'))
}

export function clearAllData(confirmation: string): Promise<void> {
  return invoke<void>('clear_all_data', { confirmation })
}

export async function markLoanPaidOff(accountId: string): Promise<void> {
  await invoke('mark_loan_paid_off', { accountId })
  for (const event of ['accounts-changed', 'plans-changed', 'transactions-changed']) window.dispatchEvent(new Event(event))
}

export async function setAccountArchived(accountId: string, archived: boolean): Promise<void> {
  await invoke('set_account_archived', { accountId, archived })
  window.dispatchEvent(new Event('accounts-changed'))
}
