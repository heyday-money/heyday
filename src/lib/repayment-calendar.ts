import { subscriptionDates } from './subscriptions'
import type { Account, Subscription } from './desktop'
import { dateKey, isDebt, monthlyDate } from './financial'

// Recurring reminders only: a due day does not establish an unpaid bill or amount.
export function repaymentDeadlines(accounts: Account[], year: number, month: number) {
  return accounts.filter(account => isDebt(account) && !account.is_archived && !account.paid_off_on && account.payment_due_day != null)
    .map(account => ({ account, date: dateKey(monthlyDate(year, month, account.payment_due_day!)) }))
    .sort((a, b) => a.date.localeCompare(b.date) || a.account.name.localeCompare(b.account.name))
}


export function subscriptionBillings(subscriptions: Subscription[], accounts: Account[], year: number, month: number) {
  const available = new Set(accounts.filter(account => !account.is_archived).map(account => account.id))
  const from = dateKey(new Date(year, month, 1))
  const through = dateKey(new Date(year, month + 1, 0))
  return subscriptions.filter(subscription => subscription.is_active && available.has(subscription.account_id))
    .flatMap(subscription => subscriptionDates(subscription, from, through).map(date => ({ subscription, date })))
    .sort((a, b) => a.date.localeCompare(b.date) || a.subscription.name.localeCompare(b.subscription.name))
}
