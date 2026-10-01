import { t as translate, useLanguage } from "../lib/i18n"
import { useSearch } from '@tanstack/react-router'
import { AccountOutlook } from './AccountOutlook'
import { CashflowPlanner } from './CashflowPlanner'
import { ExpensesOutlook } from './ExpensesOutlook'
import { CreditCardsOutlook } from './CreditCardsOutlook'
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs'

export function MonthlyOutlook() {
  useLanguage()

  const { view } = useSearch({ from: '/outlook' })
  return <Tabs defaultValue={view ?? 'planner'}>
    <div className="max-w-full overflow-x-auto"><TabsList aria-label={translate("Outlook view")} className="min-w-max"><TabsTrigger value="planner">{translate("Cashflow Planner")}</TabsTrigger><TabsTrigger value="expenses">{translate("Expenses")}</TabsTrigger><TabsTrigger value="cards">{translate("Credit Cards")}</TabsTrigger><TabsTrigger value="accounts">{translate("Account-Based Outlook")}</TabsTrigger></TabsList></div>
    <TabsContent value="planner"><CashflowPlanner /></TabsContent>
    <TabsContent value="expenses"><ExpensesOutlook /></TabsContent>
    <TabsContent value="cards"><CreditCardsOutlook /></TabsContent>
    <TabsContent value="accounts"><AccountOutlook /></TabsContent>
  </Tabs>
}
