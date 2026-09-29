import { AccountOutlook } from './AccountOutlook'
import { CashflowPlanner } from './CashflowPlanner'
import { ExpensesOutlook } from './ExpensesOutlook'
import { CreditCardsOutlook } from './CreditCardsOutlook'
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs'

export function MonthlyOutlook() {
  return <Tabs defaultValue="planner">
    <div className="max-w-full overflow-x-auto"><TabsList aria-label="Outlook view" className="min-w-max"><TabsTrigger value="planner">Cashflow Planner</TabsTrigger><TabsTrigger value="expenses">Expenses</TabsTrigger><TabsTrigger value="cards">Credit Cards</TabsTrigger><TabsTrigger value="accounts">Account-Based Outlook</TabsTrigger></TabsList></div>
    <TabsContent value="planner"><CashflowPlanner /></TabsContent>
    <TabsContent value="expenses"><ExpensesOutlook /></TabsContent>
    <TabsContent value="cards"><CreditCardsOutlook /></TabsContent>
    <TabsContent value="accounts"><AccountOutlook /></TabsContent>
  </Tabs>
}
