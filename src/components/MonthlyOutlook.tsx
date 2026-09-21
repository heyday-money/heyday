import { useState } from 'react'
import { AccountOutlook } from './AccountOutlook'
import { CashflowPlanner } from './CashflowPlanner'
import { Button } from './ui/button'

export function MonthlyOutlook() {
  const [view, setView] = useState<'planner' | 'accounts'>('planner')
  return <><div className="mb-5 flex gap-2" role="group" aria-label="Outlook view">
    <Button variant={view === 'planner' ? 'default' : 'outline'} aria-pressed={view === 'planner'} onClick={() => setView('planner')}>Cashflow Planner</Button>
    <Button variant={view === 'accounts' ? 'default' : 'outline'} aria-pressed={view === 'accounts'} onClick={() => setView('accounts')}>Account-Based Outlook</Button>
  </div>{view === 'planner' ? <CashflowPlanner /> : <AccountOutlook />}</>
}
