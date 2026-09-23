import { useEffect, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { Pencil, Plus } from 'lucide-react'
import { desktopAvailable, getSettings, listAccounts, loanTypes, type Account, type AccountType, type Settings } from '../lib/desktop'
import { formatAmount } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { AccountFormDialog, accountTypes as types } from './AccountFormDialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from './ui/tabs'
import { Button } from './ui/button'

export function AccountsPage() {
  const [accounts, setAccounts] = useState<Account[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [loadError, setLoadError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [editor, setEditor] = useState<{ account?: Account; type: AccountType } | null>(null)
  const [selectedType, setSelectedType] = useState<AccountType | 'all'>('all')
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true
    setLoading(true); setLoadError(false)
    Promise.all([getSettings(), listAccounts()]).then(([settings, accounts]) => {
      if (active) { setSettings(settings); setAccounts(accounts) }
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
      <div><h2 className="text-[25px] font-[650]">A place for every account.</h2><p className="mt-2 text-[14px]">Your cash, savings, investments, and debts.</p></div>
      <Button onClick={() => setEditor({ type: selectedType === 'all' ? 'cash' : selectedType })} type="button" disabled={!currency || loading || loadError} size="lg"><Plus size={17} />Add account</Button>
    </div>
    {!desktopAvailable ? <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your local accounts.</p>
      : loading ? <p role="status">Loading accounts…</p>
      : loadError ? <div role="alert"><p>Could not load accounts.</p><button className="mt-3 text-brand" onClick={() => setAttempt(value => value + 1)}>Try again</button></div>
      : !currency ? <div className="rounded-[22px] border border-line bg-card p-7"><h3 className="font-semibold">Choose your currency first</h3><p className="mt-2">Set the currency for all your accounts before adding a balance.</p><Link to="/settings" className="mt-4 inline-block text-brand">Go to Settings →</Link></div>
      : <>

        <Tabs value={selectedType} onValueChange={value => setSelectedType(value as AccountType | 'all')}>
          <div className="min-w-0 overflow-x-auto p-1">
            <TabsList aria-label="Account types" className="w-max min-w-full">
              <TabsTrigger value="all" className="min-w-max shrink-0 whitespace-nowrap">All ({accounts.length})</TabsTrigger>
              {types.map(item => <TabsTrigger key={item.value} value={item.value} className="min-w-max shrink-0 whitespace-nowrap">{item.groupLabel} ({accounts.filter(account => account.type === item.value).length})</TabsTrigger>)}
            </TabsList>
          </div>
          {(['all', ...types.map(item => item.value)] as const).map(tab => {
            const groups = types.filter(item => tab === 'all' ? accounts.some(account => account.type === item.value) : item.value === tab)
            const count = accounts.filter(account => tab === 'all' || account.type === tab).length
            return <TabsContent key={tab} value={tab}>
              {!count ? <div className="rounded-[22px] border border-line bg-card p-12 text-center"><h3 className="font-semibold">{tab === 'all' ? 'No accounts yet' : `No ${types.find(item => item.value === tab)!.label.toLowerCase()} accounts yet`}</h3><p className="mt-2 text-[14px]">Use Add account to {tab === 'all' ? 'start building your overview' : 'add one to this group'}.</p></div>
                : <div className="space-y-6">{groups.map(group => <section key={group.value} aria-label={`${group.groupLabel} accounts`}>
                  <h3 className="mb-3 text-sm font-semibold">{group.groupLabel} ({accounts.filter(account => account.type === group.value).length})</h3>
                  <ul className="grid grid-cols-2 gap-4 max-[900px]:grid-cols-1" aria-label={`${group.groupLabel} accounts`}>{accounts.filter(account => account.type === group.value).map(account => {
            const definition = types.find(item => item.value === account.type)!
            const Icon = definition.icon
            const liability = ['credit_card', 'loan'].includes(account.type)
            return <li key={account.id} className="min-w-0 rounded-[22px] border border-line bg-card p-6">
              <div className="flex items-center gap-3"><Icon className="shrink-0 text-brand" size={22} /><h4 className="min-w-0 flex-1 break-words font-semibold">{account.name}</h4><Button type="button" variant="ghost" size="icon" aria-label={`Edit account ${account.name}`} onClick={() => setEditor({ account, type: account.type })}><Pencil size={16} /></Button></div>
              <p className="mt-3 text-[12px]">{account.type === 'loan' ? loanTypes.find(item => item.value === account.loan_type)?.label ?? 'Loan (unclassified)' : definition.label} · {liability ? 'Liability' : 'Asset'}{account.last_four ? ` · •••• ${account.last_four}` : ''}</p>
              {account.institution && <p className="mt-1 break-words text-[13px]">{account.institution}</p>}
              <p className="mt-4 break-words text-[23px] font-semibold text-ink">{formatAmount(account.current_balance ?? account.opening_balance, currency)}</p>
              <p className="mt-1 text-[12px]">{liability ? 'Current amount owed' : 'Current balance'}</p>
              {account.credit_limit !== null && <p className="mt-3 text-[13px]">Credit limit: {formatAmount(account.credit_limit, currency)}</p>}
              {account.statement_day !== null && <p className="mt-2 text-[13px]">Statement day: {account.statement_day}</p>}
              {account.payment_due_day !== null && <p className="mt-2 text-[13px]">Payment due day: {account.payment_due_day}</p>}
              {account.interest_rate_millis !== null && <p className="mt-2 text-[13px]">Annual interest rate: {interestRateText(String(account.interest_rate_millis))}%</p>}
              {['bank', 'wallet', 'credit_card'].includes(account.type) && <Link className="mt-4 inline-block text-sm font-medium text-brand" to="/accounts/$accountId" params={{ accountId: account.id }}>Transactions & reconciliation</Link>}
              {account.notes && <p className="mt-3 break-words whitespace-pre-wrap text-[13px]">{account.notes}</p>}
            </li>
                  })}</ul>
                </section>)}</div>}
            </TabsContent>
          })}
        </Tabs>
      </>}
  </>
}
