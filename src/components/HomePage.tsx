import { getLocale } from "../lib/i18n"
import { t as translate, useLanguage } from "../lib/i18n"
import { useEffect, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { ArrowUpRight, CalendarDays, Wallet, ArrowDownLeft, ArrowDownRight } from 'lucide-react'
import { desktopAvailable } from '../lib/desktop'
import { homeOverview } from '../lib/home'
import { dateKey } from '../lib/financial'
import { formatAmount } from '../lib/money'
import { useFinancialData } from '../lib/useFinancialData'
import { useLocalDate } from '../lib/useLocalDate'
import { AccountLabel } from './InstitutionLogo'
import { Button } from './ui/button'
import { Input } from './ui/input'

const transactionNames = { get income() { return translate("Income") }, get expense() { return translate("Expense") }, get transfer() { return translate("Transfer") }, get repayment() { return translate("Repayment") } }
function dateLabel(key: string) {
  const [year, month, day] = key.split('-').map(Number)
  const date = new Date(0)
  date.setFullYear(year, month - 1, day)
  return new Intl.DateTimeFormat(getLocale(), { day: 'numeric', month: 'short', year: 'numeric' }).format(date)
}

export function HomePage() {
  useLanguage()

  const { data, error, loading, reload } = useFinancialData()
  const today = useLocalDate()
  const [selectedMonth, setSelectedMonth] = useState('')
  useEffect(() => {
    if (!desktopAvailable) return
    const refresh = () => reload()
    window.addEventListener('focus', refresh)
    return () => window.removeEventListener('focus', refresh)
  }, [reload])
  const overview = data && !error ? homeOverview(data, selectedMonth, today) : null
  const currency = data?.settings.currency
  const money = (amount: bigint) => formatAmount(amount.toString(), currency!)
  const cycleLabel = overview ? `${dateLabel(overview.cycle.startKey)} – ${dateLabel(dateKey(overview.cycle.last))}` : null
  return <div className="@container min-w-0 space-y-6">
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div><p className="mb-2 text-xs font-medium text-brand">{translate("YOUR MONEY, YOUR HEYDAY")}</p>
        <h2 className="text-2xl font-semibold">{translate("A fresh start for your finances.")}</h2>
        <p className="mt-2 text-sm">{translate("A little clarity today. More freedom tomorrow.")}</p>
      </div>
      {overview && <div className="flex flex-wrap items-end gap-2">
        <div><label htmlFor="home-cycle" className="mb-2 block text-xs font-medium">{translate("Payday cycle")}</label>
          <Input id="home-cycle" type="month" min="0001-01" max="9999-11" value={overview.cycle.key} onChange={event => {
            const month = event.target.value
            if (/^\d{4}-(0[1-9]|1[0-2])$/.test(month) && month >= '0001-01' && month <= '9999-11') setSelectedMonth(month)
          }} />
        </div>
        <Button variant="outline" onClick={() => setSelectedMonth('')}>{translate("Current cycle")}</Button>
      </div>}
    </div>
    <section aria-label={translate("Your Heyday island")} className="grid min-h-[360px] grid-cols-1 @min-[650px]:grid-cols-2 items-center overflow-hidden rounded-2xl border border-line bg-card p-9 max-[1000px]:p-6">
      <div>
        <span className="inline-flex items-center gap-2 rounded-full bg-accent/20 px-3 py-1.5 text-xs font-medium text-ink">
          <CalendarDays className="size-3.5 shrink-0" />{cycleLabel ?? translate("Make yourself at home")}
        </span>
        <h2 className="mt-5 mb-3.5 text-[clamp(25px,3vw,39px)] leading-tight font-semibold tracking-tight">{translate("Your money.")}<br />{translate("All in one little world.")}</h2>
        <p className="max-w-[330px] text-sm">{translate("Bring your accounts and expected income together, and make room for what matters.")}</p>
        <div className="mt-6 flex flex-wrap gap-2.5">
          <Button asChild><Link to="/accounts">{translate("Explore accounts")}{" "}<ArrowUpRight size={17} /></Link></Button>
          <Button asChild variant="outline"><Link to="/income">{translate("Explore income")}</Link></Button>
        </div>
      </div>
      <img className="w-full max-w-none @min-[650px]:-ml-[4%] @min-[650px]:w-[115%] dark:brightness-[.82] dark:saturate-[.85]"
        src="/images/island.png" alt={translate("A peaceful floating island with a purple-roofed home, trees, and a pond")} />
    </section>
    {!desktopAvailable ? <p className="rounded-2xl border border-line bg-card p-6">{translate("Open the desktop app to see your financial overview.")}</p>
      : error ? <div role="alert" className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-line bg-card p-6"><p>{translate("Could not load your Home overview.")}</p><Button variant="outline" onClick={reload}>{translate("Retry overview")}</Button></div>
      : !overview ? <p role="status" className="rounded-2xl border border-line bg-card p-6">{translate("Loading your overview…")}</p>
      : !currency ? <section aria-label={translate("Get started")} className="rounded-2xl border border-line bg-card p-6"><h3 className="text-lg font-semibold">{translate("Choose your currency first")}</h3><p className="mt-2 text-sm">{translate("Set the currency, then add an account to start your financial overview.")}</p><Button asChild className="mt-4"><Link to="/settings">{translate("Choose currency")}</Link></Button></section>
      : <div className="space-y-6" aria-busy={loading}>
        {!data!.accounts.length && <section aria-label={translate("Get started")} className="rounded-2xl border border-line bg-card p-6"><h3 className="text-lg font-semibold">{translate("Add your first account")}</h3><p className="mt-2 text-sm">{translate("Enter an opening balance, record your first transaction, then add expected income.")}</p><Button asChild className="mt-4"><Link to="/accounts">{translate("Add an account")}</Link></Button></section>}
        <dl className="grid grid-cols-1 gap-4 @min-[760px]:grid-cols-3" aria-label={translate("Home financial summary")}>
          {([
            [translate("Available cash"), overview.cash, translate("Current balance · Cash, Bank & Wallet"), Wallet],
            [translate("Income received"), overview.incomeReceived, translate("Recorded income · selected cycle · all accounts"), ArrowDownLeft],
            [translate("Recorded spending"), overview.spending, translate("Purchases · selected cycle · all accounts"), ArrowDownRight],
          ] as const).map(([label, amount, description, Icon]) => <div key={label} className="min-w-0 rounded-2xl border border-line bg-card p-6">
            <dt className="flex items-center gap-2 text-sm text-muted"><Icon className="size-4" />{label}</dt>
            <dd className={`mt-3 break-words text-[28px] font-semibold tabular-nums ${amount < 0n ? 'text-red-700 dark:text-red-400' : 'text-ink'}`}>{money(amount)}</dd>
            <p className="mt-2 text-xs">{description}</p>
          </div>)}
        </dl>
        <p className="text-xs">{translate("Current cash includes opening balances and recorded activity. Spending includes card purchases; transfers and repayments are excluded. Cycle totals include recorded activity through")}{" "}{dateLabel(dateKey(today))}.</p>
        <section aria-labelledby="home-attention-title" className="rounded-2xl border border-line bg-card p-6">
          <div className="flex flex-wrap items-center justify-between gap-3"><h3 id="home-attention-title" className="text-lg font-semibold">{translate("Needs attention")}{" "}<span className="ml-1 text-sm font-normal text-muted">({overview.attention.length})</span></h3><span className="text-xs text-muted">{translate("Due dates in the next 14 days & items to review")}</span></div>
          {!overview.attention.length ? <p className="mt-3 text-sm">{translate("No saved bills or payment plans need attention right now.")}</p> : <ul className="mt-3 divide-y divide-line">
            {overview.attention.slice(0, 5).map(item => <li key={item.id} className="flex flex-wrap items-center justify-between gap-3 py-4">
              <div className="min-w-0 flex-1"><h4 className="break-words text-sm font-medium">{item.name}</h4><p className="mt-1 text-xs">{translate(item.title)} · {dateLabel(item.date)}</p><p className="mt-1 text-xs">{translate(item.detail)}</p>
                {item.amount !== undefined && <p className="mt-2 text-sm font-semibold text-ink">{item.kind === 'billing' ? translate("Remaining statement balance") : translate("Planned amount")}: {money(item.amount)}</p>}
              </div>
              {item.kind === 'billing' && item.accountId ? <Button asChild size="sm" variant="outline"><Link to="/accounts/$accountId/billing" params={{ accountId: item.accountId }}>{translate("Review bill")}</Link></Button> : <Button asChild size="sm" variant="outline"><Link to="/outlook" search={{ view: 'accounts' }}>{translate("Review plan")}</Link></Button>}
            </li>)}
          </ul>}
          {overview.attention.length > 5 && <p className="mt-2 text-xs">{translate("Showing the first five reminders. Review remaining bills in Accounts and plans in Outlook.")}</p>}
        </section>
        <section aria-labelledby="home-income-title" className="rounded-2xl border border-line bg-card p-6">
          <div className="flex flex-wrap items-center justify-between gap-3"><h3 id="home-income-title" className="text-lg font-semibold">{translate("Upcoming expected income")}</h3><Button asChild variant="outline" size="sm"><Link to="/income">{translate("View income")}</Link></Button></div>
          <p className="mt-2 text-xs">{translate("Next scheduled payment for each active source. Estimates only; record receipts separately.")}</p>
          {!overview.upcomingIncome.length ? <p className="mt-3 text-sm">{translate("No upcoming income sources with an available destination account.")}</p> : <ul className="mt-3 divide-y divide-line">
            {overview.upcomingIncome.slice(0, 5).map(({ source, date, amount }) => <li key={source.id} className="flex flex-wrap items-center justify-between gap-3 py-4">
              <div className="min-w-0"><h4 className="break-words text-sm font-medium">{source.name}</h4><p className="mt-1 text-xs">{dateLabel(date)} {" "}{translate("· To")}{" "}<AccountLabel id={source.destination_account_id} name={source.destination_account_name} /></p></div>
              <div className="text-right"><p className="font-semibold text-ink">{money(amount)}</p><p className="mt-1 text-xs">{source.type === 'salary' ? translate("Estimated net income") : translate("Estimated income")}</p></div>
            </li>)}
          </ul>}
          {overview.upcomingIncome.length > 5 && <p className="mt-2 text-xs">{translate("Showing the next five sources. View Income for all sources.")}</p>}
        </section>
        <section aria-labelledby="home-recent-title" className="rounded-2xl border border-line bg-card p-6">
          <div className="flex flex-wrap items-center justify-between gap-3"><h3 id="home-recent-title" className="text-lg font-semibold">{translate("Recent transactions")}</h3><Button asChild variant="outline" size="sm"><Link to="/transactions">{translate("View all transactions")}</Link></Button></div>
          <p className="mt-2 text-xs">{translate("Latest five entries across all recorded dates.")}</p>
          {!overview.recent.length ? <p className="mt-3 text-sm">{translate("No transactions yet. Use Add transaction to record your first entry.")}</p> : <ul className="mt-3 divide-y divide-line">
            {overview.recent.map(row => <li key={row.id} className="flex flex-wrap items-center justify-between gap-3 py-4">
              <div className="min-w-0 flex-1"><h4 className="break-words text-sm font-medium">{row.description || transactionNames[row.type]}</h4><p className="mt-1 text-xs">{dateLabel(row.date)} · {transactionNames[row.type]}</p><p className="mt-1 text-xs"><AccountLabel id={row.account_id} name={row.account_name} />{row.destination_account_id && <> → <AccountLabel id={row.destination_account_id} name={row.destination_account_name ?? translate("Destination account")} /></>}</p></div>
              <span className={`text-right font-medium tabular-nums ${row.type === 'income' ? 'text-green-700 dark:text-green-400' : row.type === 'expense' ? 'text-red-700 dark:text-red-400' : 'text-ink'}`}>{row.type === 'income' ? '+' : row.type === 'expense' ? '−' : ''}{money(BigInt(row.amount))}</span>
            </li>)}
          </ul>}
        </section>
      </div>}
  </div>
}
