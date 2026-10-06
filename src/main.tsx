import { t as translate, useLanguage } from "./lib/i18n"
import { CardBillingPage } from './components/CardBillingPage'
import { LoanAccountPage } from './components/LoanAccountPage'
import { AccountTransactionsPage } from './components/AccountTransactionsPage'
import { AccountDetailsPage } from './components/AccountDetailsPage'
import { SubscriptionsPage } from './components/SubscriptionsPage'
import { InstallmentsPage } from './components/InstallmentsPage'
import { MonthlyOutlook } from './components/MonthlyOutlook'
import { NetWorthPage } from './components/NetWorthPage'
import React from 'react'
import ReactDOM from 'react-dom/client'
import { createHashHistory, createRootRoute, createRoute, createRouter, RouterProvider } from '@tanstack/react-router'
import { AppLayout, HomePage, AccountsPage, IncomePage, SettingsPage } from './pages'
import './styles.css'
import { TransactionsPage } from './components/TransactionsPage'

const rootRoute = createRootRoute({
  component: AppLayout,
  notFoundComponent: () => <section className="rounded-[22px] border border-line bg-card p-[27px]"><h2>{translate("Page not found")}</h2><a href="#/">{translate("Return home")}</a></section>,
})
const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts/$accountId/details', component: AccountDetailsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts/$accountId/billing', component: CardBillingPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts/$accountId/loans', component: LoanAccountPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/', component: HomePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/outlook', validateSearch: (search: Record<string, unknown>): { view?: 'planner' | 'accounts' | 'expenses' | 'cards' } => ({ view: ['planner', 'accounts', 'expenses', 'cards'].includes(String(search.view)) ? search.view as 'planner' | 'accounts' | 'expenses' | 'cards' : undefined }), component: MonthlyOutlook }),
  createRoute({ getParentRoute: () => rootRoute, path: '/net-worth', component: NetWorthPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts', component: AccountsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts/$accountId', validateSearch: (search: Record<string, unknown>): { reconcile?: boolean } => ({ reconcile: search.reconcile === true || search.reconcile === 'true' ? true : undefined }), component: AccountTransactionsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/income', component: IncomePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/transactions', validateSearch: (search: Record<string, unknown>): { account?: string } => ({ account: typeof search.account === 'string' ? search.account : undefined }), component: TransactionsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/installments', component: InstallmentsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/subscriptions', component: SubscriptionsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: SettingsPage }),
])
const router = createRouter({ routeTree, history: createHashHistory() })

declare module '@tanstack/react-router' {
  interface Register { router: typeof router }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode><RouterProvider router={router} /></React.StrictMode>,
)
