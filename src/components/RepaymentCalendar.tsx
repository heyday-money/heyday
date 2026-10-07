import { useId, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import type { FinancialData } from '../lib/desktop'
import { dateKey, isDebt } from '../lib/financial'
import { getLocale, t, useLanguage } from '../lib/i18n'
import { formatAmount } from '../lib/money'
import { SubscriptionLogo } from './SubscriptionLogo'
import { NativeSelect } from './ui/native-select'
import { repaymentDeadlines, subscriptionBillings } from '../lib/repayment-calendar'
import { useInstitutions } from './InstitutionProvider'
import { InstitutionLogo } from './InstitutionLogo'
import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from './ui/tooltip'
import { Button } from './ui/button'

export function RepaymentCalendar({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  useLanguage()
  const directory = useInstitutions()
  const filterId = useId()
  const [filter, setFilter] = useState('all')
  const [selected, setSelected] = useState<Date | null>(null)
  const month = selected ?? new Date(today.getFullYear(), today.getMonth(), 1)
  const year = month.getFullYear(), monthIndex = month.getMonth()
  const deadlines = filter === 'subscriptions' ? [] : repaymentDeadlines(data.accounts, year, monthIndex)
  const billings = filter === 'repayments' ? [] : subscriptionBillings(data.subscriptions ?? [], data.accounts, year, monthIndex)
  const missing = data?.accounts.filter(a => isDebt(a) && !a.is_archived && !a.paid_off_on && a.payment_due_day == null) ?? []
  const start = (month.getDay() + 6) % 7
  const dayCount = new Date(year, monthIndex + 1, 0).getDate()
  const cells = Math.ceil((start + dayCount) / 7) * 7
  return <TooltipProvider><section aria-labelledby="repayment-calendar-title" className="min-w-0 rounded-2xl border border-line bg-card p-6">
    <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
      <h2 id="repayment-calendar-title" className="text-lg font-semibold">{t('Payment Calendar')}</h2>
      <div className="flex max-w-full flex-wrap items-center gap-2">
        <Button variant="outline" size="icon" aria-label={t('Previous month')} disabled={year <= 1900 && monthIndex === 0} onClick={() => setSelected(new Date(year, monthIndex - 1, 1))}><ChevronLeft size={16} /></Button>
        <span aria-live="polite" className="min-w-36 text-center font-semibold">{month.toLocaleDateString(getLocale(), { month: 'long', year: 'numeric' })}</span>
        <Button variant="outline" size="icon" aria-label={t('Next month')} disabled={year >= 9999 && monthIndex === 11} onClick={() => setSelected(new Date(year, monthIndex + 1, 1))}><ChevronRight size={16} /></Button>
        <Button variant="outline" onClick={() => setSelected(null)}>{t('Current month')}</Button>
      </div>
    </div>
    <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
      <p className="min-w-0 max-w-xl text-sm text-muted">{t('Account repayment deadlines and subscription billing dates. Scheduled reminders only; payments are not confirmed and balances stay unchanged.')}</p>
      <div className="flex items-center gap-3"><label htmlFor={filterId} className="shrink-0 text-sm font-medium">{t('Calendar type')}</label><NativeSelect id={filterId} value={filter} onChange={event => setFilter(event.target.value)}>
        <option value="all">{t('All')}</option><option value="repayments">{t('Repayments')}</option><option value="subscriptions">{t('Subscriptions')}</option>
      </NativeSelect></div>
    </div>
        <div role="region" aria-label={t('Monthly payment calendar')} tabIndex={0} className="overflow-x-auto rounded-xl border border-line" aria-busy={loading}>
          <table className="w-full min-w-[700px] table-fixed border-collapse bg-card text-sm">
            <caption className="sr-only">{t('Monthly payment calendar')}</caption>
            <thead><tr>{Array.from({ length: 7 }, (_, i) => <th scope="col" key={i} className="border-b border-line bg-soft p-3 text-left">{new Date(2026, 0, 5 + i).toLocaleDateString(getLocale(), { weekday: 'short' })}</th>)}</tr></thead>
            <tbody>{Array.from({ length: cells / 7 }, (_, week) => <tr key={week}>{Array.from({ length: 7 }, (_, column) => {
              const day = week * 7 + column - start + 1
              if (day < 1 || day > dayCount) return <td key={column} className="border border-line bg-soft/30" />
              const date = dateKey(new Date(year, monthIndex, day))
              const current = date === dateKey(today)
              return <td key={column} className={`h-28 border border-line p-2 align-top ${current ? 'bg-soft' : ''}`}>
                <time dateTime={date} aria-current={current ? 'date' : undefined} className={`inline-flex size-7 items-center justify-center rounded-full ${current ? 'bg-brand text-white font-semibold' : 'text-muted'}`}>{day}</time>
                <ul className="mt-2 flex flex-wrap gap-2">{deadlines.filter(d => d.date === date).map(({ account }) => <li key={`account:${account.id}`}>
                  <Tooltip><TooltipTrigger asChild>
                    <Link to="/accounts/$accountId/details" params={{ accountId: account.id }}
                      aria-label={`${account.name}${account.last_four ? ` · •${account.last_four}` : ''} · ${t('Payment due')}`}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-line bg-card hover:bg-soft focus-visible:outline-2 focus-visible:outline-brand">
                      <InstitutionLogo institution={directory.institutions.find(i => i.id === directory.accounts.find(a => a.id === account.id)?.institution_id)} name={account.name} className="size-8 rounded-md text-base" />
                    </Link>
                  </TooltipTrigger><TooltipContent>{account.name}{account.last_four ? ` · •${account.last_four}` : ''} · {t('Payment due')}</TooltipContent></Tooltip>
                </li>)}{billings.filter(b => b.date === date).map(({ subscription }) => <li key={`subscription:${subscription.id}`}>
                  <Tooltip><TooltipTrigger asChild>
                    <Link to="/subscriptions" aria-label={`${subscription.name} · ${t('Subscription billing')}`}
                      className="inline-flex size-10 items-center justify-center rounded-lg border border-brand/30 bg-soft hover:bg-brand/10 focus-visible:outline-2 focus-visible:outline-brand">
                      <SubscriptionLogo subscription={subscription} />
                    </Link>
                  </TooltipTrigger><TooltipContent><p>{subscription.name} · {t('Subscription billing')}</p>{data.settings.currency && <p>{formatAmount(subscription.amount, data.settings.currency)}</p>}<p>{subscription.account_name}</p></TooltipContent></Tooltip>
                </li>)}</ul>
              </td>
            })}</tr>)}</tbody>
          </table>
        </div>
        {!deadlines.length && !billings.length && <p className="mt-4 text-sm" role="status">{t('No scheduled events for this month and filter.')}</p>}
        {filter !== 'subscriptions' && !!missing.length && <div className="mt-4 rounded-xl border border-line p-4 text-sm"><p className="font-medium">{t('Accounts without a payment due day')}</p><ul className="mt-2 flex flex-wrap gap-3">{missing.map(a => <li key={a.id}><Link to="/accounts/$accountId/details" params={{ accountId: a.id }} className="text-brand hover:underline">{a.name}</Link></li>)}</ul></div>}
        <p className="mt-3 text-xs text-muted">{t('Dates beyond month-end use the last day. Weekends and holidays are not adjusted. Manage repayment due days in account details and billing schedules in Subscriptions.')}</p>
  </section></TooltipProvider>
}
