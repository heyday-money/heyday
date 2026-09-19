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

export type AccountType = 'cash' | 'bank' | 'credit_card' | 'loan' | 'investment'
export interface Account {
  id: string
  name: string
  type: AccountType
  opening_balance: string
  institution: string | null
  last_four: string | null
  notes: string | null
  credit_limit: string | null
  statement_day: number | null
  payment_due_day: number | null
  interest_rate_bps: number | null
}
export type NewAccount = Omit<Account, 'id'> & { currency: string }

export function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>('list_accounts')
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
  recurrence_frequency: 'monthly'
  recurrence_day_of_month: number
  is_auto_create_transaction: boolean
  is_active: boolean
  created_at: string
  updated_at: string
}
export type NewIncome = Omit<Income, 'id' | 'destination_account_name' | 'created_at' | 'updated_at'> & { currency: string }

export function listIncomes(): Promise<Income[]> {
  return invoke<Income[]>('list_incomes')
}
export function createIncome(input: NewIncome): Promise<Income> {
  return invoke<Income>('create_income', { input })
}
