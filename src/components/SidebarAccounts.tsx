import { useEffect, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { desktopAvailable, getSettings, listAccounts, type Account } from '../lib/desktop'
import { formatAmount } from '../lib/money'

const groups = [
  ['cash', 'Cash'], ['bank', 'Bank'], ['credit_card', 'Credit cards'],
  ['loan', 'Loans'], ['investment', 'Investments'],
] as const

export function accountDisplayBalance(account: Pick<Account, 'type' | 'opening_balance'> & Partial<Pick<Account, 'current_balance'>>) {
  const balance = BigInt(account.current_balance ?? account.opening_balance)
  return ['credit_card', 'loan'].includes(account.type) ? -balance : balance
}

export function SidebarAccounts() {
  const [accounts, setAccounts] = useState<Account[]>([])
  const [currency, setCurrency] = useState<string | null>(null)
  const [error, setError] = useState(false)
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    let request = 0
    const load = async () => {
      const current = ++request
      try {
        const [settings, accounts] = await Promise.all([getSettings(), listAccounts()])
        if (active && current === request) { setCurrency(settings.currency); setAccounts(accounts); setError(false) }
      } catch { if (active && current === request) setError(true) }
    }
    void load()
    window.addEventListener('accounts-changed', load)
    return () => { active = false; window.removeEventListener('accounts-changed', load) }
  }, [])
  if (!desktopAvailable) return null
  if (error) return <p className="px-2 text-[11px]">Could not load account balances.</p>
  if (!currency || !accounts.length) return null
  return <div className="min-w-0 space-y-3 px-1 pb-3" aria-label="Sidebar accounts">
    {groups.map(([type, label]) => {
      const items = accounts.filter(account => account.type === type)
      if (!items.length) return null
      return <section key={type} aria-label={label}>
        <h2 className="px-1 py-1 text-[10px] font-semibold tracking-wide text-muted uppercase">{label}</h2>
        <ul className="space-y-0.5">
          {items.map(account => {
            const balance = accountDisplayBalance(account)
            const amount = formatAmount(balance.toString(), currency)
            return <li key={account.id}>
              <Link to="/accounts" title={`${account.name}: ${amount}`} aria-label={`${account.name}: ${amount}`}
                className="flex min-w-0 items-center justify-between gap-2 rounded-md px-1 py-1.5 text-[12px] hover:bg-soft">
                <span className="min-w-0 truncate">{account.name}</span>
                <span data-balance-sign={balance > 0n ? 'positive' : balance < 0n ? 'negative' : 'zero'} className={`max-w-[60%] shrink-0 truncate text-right text-[11px] font-medium tabular-nums ${balance > 0n ? 'text-green-700 dark:text-green-400' : balance < 0n ? 'text-red-600 dark:text-red-400' : 'text-muted'}`}>
                  ({amount.replace(` ${currency}`, '')})
                </span>
              </Link>
            </li>
          })}
        </ul>
      </section>
    })}
  </div>
}
