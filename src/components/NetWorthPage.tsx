import { Link } from '@tanstack/react-router'
import { desktopAvailable } from '../lib/desktop'
import { accountValue, dateKey, netWorth } from '../lib/financial'
import { formatAmount } from '../lib/money'
import { useFinancialData } from '../lib/useFinancialData'
import { Button } from './ui/button'

const types = [ ['cash', 'Cash'], ['bank', 'Bank'], ['wallet', 'Wallets'], ['credit_card', 'Credit cards'], ['loan', 'Loans'], ['investment', 'Investments'] ] as const
export function NetWorthPage() {
  const { data, error, loading, today, reload } = useFinancialData()
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">Open the desktop app to see your net worth.</p>
  if (error) return <div role="alert">Could not load net worth. <Button variant="outline" onClick={reload}>Retry net worth</Button></div>
  if (!data) return <p role="status">Loading net worth…</p>
  const currency = data.settings.currency
  if (!currency) return <p>Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to see your net worth.</p>
  const totals = netWorth(data.accounts)
  const money = (value: bigint) => formatAmount(value.toString(), currency)
  return <div aria-busy={loading}>
    <div className="mb-6"><h2 className="text-2xl font-semibold">Your overall financial position.</h2><p className="mt-2 text-sm">Current assets minus liabilities · {dateKey(today)} · Active accounts</p></div>
    <dl className="grid grid-cols-1 gap-4 sm:grid-cols-3">
      {[['Net worth', totals.total], ['Assets', totals.assets], ['Liabilities', totals.liabilities]].map(([label, amount]) => <div key={String(label)} className="min-w-0 rounded-2xl border border-line bg-card p-6"><dt className="text-sm text-muted">{label}</dt><dd className="mt-3 break-words text-2xl font-semibold tabular-nums">{money(amount as bigint)}</dd></div>)}
    </dl>
    <p className="mt-4 text-xs">Based on manual opening balances and recorded transactions. Investment values are entered manually. Overdrafts count as liabilities; card and loan credits count as assets. Expected income and payment plans are excluded.</p>
    {!data.accounts.length ? <div className="mt-6 rounded-2xl border border-line bg-card p-8"><h3 className="font-semibold">No accounts yet</h3><Link to="/accounts" className="mt-3 inline-block text-brand">Add your first account →</Link></div> : <div className="mt-6 space-y-5">{types.map(([type, label]) => {
      const accounts = data.accounts.filter(account => account.type === type)
      if (!accounts.length) return null
      const total = accounts.reduce((sum, account) => sum + accountValue(account), 0n)
      return <section key={type} aria-label={`${label} breakdown`} className="rounded-2xl border border-line bg-card p-5"><div className="mb-3 flex flex-wrap justify-between gap-3"><h3 className="font-semibold">{label}</h3><span className="font-semibold tabular-nums">{money(total)}</span></div><ul className="divide-y divide-line">{accounts.map(account => {
        const value = accountValue(account)
        return <li key={account.id} className="flex flex-wrap justify-between gap-3 py-3"><Link to="/accounts" className="min-w-0 break-words hover:text-brand">{account.name}</Link><div className="text-right"><span className={`tabular-nums ${value < 0n ? 'text-red-700 dark:text-red-400' : value > 0n ? 'text-green-700 dark:text-green-400' : ''}`}>{money(value)}</span><span className="ml-2 text-xs text-muted">{value < 0n ? 'Liability' : value > 0n ? 'Asset' : 'Zero balance'}</span></div></li>
      })}</ul></section>
    })}</div>}
    <p className="mt-5 text-xs">Historical net worth charts will be available when balance snapshots are supported.</p>
  </div>
}
