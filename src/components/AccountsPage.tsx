import { t as translate, useLanguage } from "../lib/i18n"
import { AccountActions } from './AccountActions'
import { AccountsTable } from './AccountsTable'
import { AccountLabel } from './InstitutionLogo'
import { useCardLimits } from '../lib/card-limits'
import { SharedCreditLimits, CardCredit } from './SharedCreditLimits'
import { useEffect, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { LayoutGrid, Table2, Plus } from 'lucide-react'
import { desktopAvailable, getSettings, listAccounts, loanTypes, type Account, type AccountType, type Settings } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { netWorth } from '../lib/financial'
import { interestRateText } from '../lib/installments'
import { AccountFormDialog, accountTypes as types } from './AccountFormDialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from './ui/tabs'
import { Button } from './ui/button'

export function AccountsPage() {
  useLanguage()

  const limits = useCardLimits()
  const [archivedAccounts, setArchivedAccounts] = useState<Account[]>([])
  const [closedLoans, setClosedLoans] = useState<Account[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [editor, setEditor] = useState<{ account?: Account; type: AccountType } | null>(null)
  const [selectedType, setSelectedType] = useState<AccountType | 'all'>('all')
  const [view, setView] = useState<'table' | 'cards'>(() => {
    try { return localStorage.getItem('accounts-view') === 'cards' ? 'cards' : 'table' } catch { return 'table' }
  })
  function changeView(next: 'table' | 'cards') {
    setView(next)
    try { localStorage.setItem('accounts-view', next) } catch { /* View preferences are optional. */ }
  }
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts(true, true)]).then(([settings, accounts]) => {
      if (active) { setArchivedAccounts(accounts.filter(account => account.is_archived && !account.paid_off_on)); setSettings(settings); setAccounts(accounts.filter(account => !account.paid_off_on && !account.is_archived)); setClosedLoans(accounts.filter(account => !!account.paid_off_on)) }
    }).catch(() => { if (active) setLoadError(true) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [attempt])
  useEffect(() => {
    const refresh = () => setAttempt(value => value + 1)
    window.addEventListener('accounts-changed', refresh)
    return () => window.removeEventListener('accounts-changed', refresh)
  }, [])
  const currency = settings?.currency
  return <>
    {editor && currency && <AccountFormDialog account={editor.account} initialType={editor.type} currency={currency} onClose={() => setEditor(null)} onSaved={account => {
      setAccounts(current => [...current.filter(item => item.id !== account.id), account])
      if (selectedType !== 'all') setSelectedType(account.type)
    }} />}
    <div className="mb-6 flex items-start justify-between gap-4">
      <div><h2 className="text-2xl font-semibold">{translate("A place for every account.")}</h2><p className="mt-2 text-[14px]">{translate("Your cash, savings, investments, and debts.")}</p></div>
      <Button onClick={() => setEditor({ type: selectedType === 'all' ? 'cash' : selectedType })} type="button" disabled={!currency || loading || loadError} size="lg"><Plus size={17} />{translate("Add account")}</Button>
    </div>
    {currency && (selectedType === 'all' || selectedType === 'credit_card') && <SharedCreditLimits currency={currency} limits={limits} />}
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage your local accounts.")}</p>
      : loading ? <p role="status">{translate("Loading accounts…")}</p>
      : loadError ? <div role="alert"><p>{translate("Could not load accounts.")}</p><button className="mt-3 text-brand" onClick={() => setAttempt(value => value + 1)}>{translate("Try again")}</button></div>
      : !currency ? <div className="rounded-2xl border border-line bg-card p-7"><h3 className="font-semibold">{translate("Choose your currency first")}</h3><p className="mt-2">{translate("Set the currency for all your accounts before adding a balance.")}</p><Link to="/settings" className="mt-4 inline-block text-brand">{translate("Go to Settings →")}</Link></div>
      : <>
        <div className="mb-6">
          <h3 className="text-sm font-semibold">{translate("Summary by account type")}</h3>
          <p className="mt-1 text-xs text-muted">{translate("Current balances for active accounts. Overdrafts count as liabilities; card and loan credits count as assets.")}</p>
          <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {types.map(type => {
              const group = accounts.filter(account => account.type === type.value)
              const totals = netWorth(group)
              const Icon = type.icon
              return <div key={type.value} role="group" aria-label={translate("{value0} summary", { value0: type.groupLabel })} className="min-w-0 rounded-2xl border border-line bg-card p-5">
                <div className="flex items-center gap-2"><Icon size={18} className="shrink-0 text-brand" /><h4 className="font-semibold">{type.groupLabel}</h4><span className="ml-auto text-xs text-muted">{group.length} {group.length === 1 ? translate("account") : translate("accounts")}</span></div>
                <dl className="mt-4">
                  <dt className="text-xs text-muted">{translate("Net balance")}</dt>
                  <dd className={`mt-1 break-words text-xl font-semibold tabular-nums ${totals.total < 0n ? 'text-red-700 dark:text-red-400' : totals.total > 0n ? 'text-green-700 dark:text-green-400' : 'text-ink'}`}>{formatAmount(totals.total.toString(), currency)}</dd>
                  <div className="mt-3 grid grid-cols-2 gap-3 border-t border-line pt-3">
                    <div className="min-w-0"><dt className="text-xs text-muted">{translate("Assets")}</dt><dd className="mt-1 break-words text-sm tabular-nums">{formatAmount(totals.assets.toString(), currency)}</dd></div>
                    <div className="min-w-0"><dt className="text-xs text-muted">{translate("Liabilities")}</dt><dd className="mt-1 break-words text-sm tabular-nums">{formatAmount(totals.liabilities.toString(), currency)}</dd></div>
                  </div>
                </dl>
              </div>
            })}
          </div>
        </div>
        <div role="group" aria-label={translate("Account view")} className="mb-3 flex justify-end gap-2">
          <Button type="button" variant={view === 'table' ? 'default' : 'outline'} size="sm" aria-pressed={view === 'table'} onClick={() => changeView('table')}><Table2 size={16} />{translate("Table")}</Button>
          <Button type="button" variant={view === 'cards' ? 'default' : 'outline'} size="sm" aria-pressed={view === 'cards'} onClick={() => changeView('cards')}><LayoutGrid size={16} />{translate("Cards")}</Button>
        </div>
        <Tabs value={selectedType} onValueChange={value => setSelectedType(value as AccountType | 'all')}>
          <div className="min-w-0 overflow-x-auto p-1">
            <TabsList aria-label={translate("Account types")} className="w-max min-w-full">
              <TabsTrigger value="all" className="min-w-max shrink-0 whitespace-nowrap">{translate("All (")}{accounts.length})</TabsTrigger>
              {types.map(item => <TabsTrigger key={item.value} value={item.value} className="min-w-max shrink-0 whitespace-nowrap">{item.groupLabel} ({accounts.filter(account => account.type === item.value).length})</TabsTrigger>)}
            </TabsList>
          </div>
          {(['all', ...types.map(item => item.value)] as const).map(tab => {
            const groups = types.filter(item => tab === 'all' ? accounts.some(account => account.type === item.value) : item.value === tab)
            const count = accounts.filter(account => tab === 'all' || account.type === tab).length
            return <TabsContent key={tab} value={tab}>
              {!count ? <div className="rounded-2xl border border-line bg-card p-12 text-center"><h3 className="font-semibold">{tab === 'all' ? translate("No accounts yet") : translate("No {value0} accounts yet", { value0: types.find(item => item.value === tab)!.label.toLowerCase() })}</h3><p className="mt-2 text-[14px]">{translate("Use Add account to")}{" "}{tab === 'all' ? translate("start building your overview") : translate("add one to this group")}.</p></div>
                : view === 'table' ? <AccountsTable sortBalance={tab === 'bank' || tab === 'credit_card'} showLoanDetails={tab === 'loan'} accounts={groups.flatMap(group => accounts.filter(account => account.type === group.value))} currency={currency} onEdit={account => setEditor({ account, type: account.type })} />
                : <div className="space-y-6">{groups.map(group => <section key={group.value} aria-label={translate("{value0} accounts", { value0: group.groupLabel })}>
                  <h3 className="mb-3 text-sm font-semibold">{group.groupLabel} ({accounts.filter(account => account.type === group.value).length})</h3>
                  <ul className="grid grid-cols-2 gap-4 max-[900px]:grid-cols-1" aria-label={translate("{value0} accounts", { value0: group.groupLabel })}>{accounts.filter(account => account.type === group.value).map(account => {
            const definition = types.find(item => item.value === account.type)!
            const liability = ['credit_card', 'loan'].includes(account.type)
            return <li key={account.id} className="min-w-0 rounded-2xl border border-line bg-card p-6">
              <div className="flex items-center gap-3"><h4 className="min-w-0 flex-1 break-words font-semibold"><Link to="/accounts/$accountId/details" params={{ accountId: account.id }} className="text-brand hover:underline"><AccountLabel id={account.id} name={account.name} /></Link></h4><AccountActions account={account} onEdit={account => setEditor({ account, type: account.type })} /></div>
              <p className="mt-3 text-[12px]">{account.type === 'loan' ? loanTypes.find(item => item.value === account.loan_type)?.label ?? translate("Loan (unclassified)") : definition.label} · {liability ? translate("Liability") : translate("Asset")}{account.last_four ? translate(" · •••• {value0}", { value0: account.last_four }) : ''}</p>

              <p className="mt-4 break-words text-[28px] font-semibold tabular-nums text-ink">{formatAmount(account.current_balance ?? account.opening_balance, currency)}</p>
              <p className="mt-1 text-[12px]">{liability ? translate("Outstanding Balance") : translate("Current balance")}</p>
              {account.type === 'loan' && <p className="mt-2 text-[13px]">{translate("Initial Loan Amount:")}{" "}{account.initial_loan_amount != null ? formatAmount(account.initial_loan_amount, currency) : translate("Not set")}</p>}
              {account.type === 'loan' && <p className="mt-2 text-[13px]">{translate("Monthly installment:")}{" "}{account.monthly_installment != null ? formatAmount(account.monthly_installment, currency) : translate("Not set")}</p>}
              {account.type === 'credit_card' && <CardCredit id={account.id} individualLimit={account.credit_limit} balance={account.current_balance ?? account.opening_balance} currency={currency} limits={limits} />}
              {account.statement_day !== null && <p className="mt-2 text-[13px]">{translate("Statement day:")}{" "}{account.statement_day}</p>}
              {account.payment_due_day !== null && <p className="mt-2 text-[13px]">{translate("Payment due day:")}{" "}{account.payment_due_day}</p>}
              {account.interest_rate_ten_thousandths !== null && <p className="mt-2 text-[13px]">{translate("Annual interest rate:")}{" "}{interestRateText(String(account.interest_rate_ten_thousandths), 4)}%</p>}
              {account.notes && <p className="mt-3 break-words whitespace-pre-wrap text-[13px]">{account.notes}</p>}
            </li>
                  })}</ul>
                </section>)}</div>}
            </TabsContent>
          })}
        </Tabs>
        {!!archivedAccounts.length && <details className="mt-6 rounded-2xl border border-line p-4"><summary className="cursor-pointer font-semibold">{translate("Archived accounts (")}{archivedAccounts.length})</summary><div className="mt-4"><AccountsTable accounts={archivedAccounts} currency={currency} onEdit={() => {}} /></div></details>}
        {!!closedLoans.length && <details className="mt-6 rounded-2xl border border-line p-4"><summary className="cursor-pointer font-semibold">{translate("Paid-off loans (")}{closedLoans.length})</summary><div className="mt-4"><AccountsTable accounts={closedLoans} currency={currency} onEdit={() => {}} /></div></details>}
      </>}
  </>
}
