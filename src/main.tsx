import React from 'react'
import ReactDOM from 'react-dom/client'
import { createHashHistory, createRootRoute, createRoute, createRouter, RouterProvider } from '@tanstack/react-router'
import { AppLayout, HomePage, AccountsPage, IncomePage, SettingsPage } from './pages'
import './styles.css'

const rootRoute = createRootRoute({
  component: AppLayout,
  notFoundComponent: () => <section className="rounded-[22px] border border-line bg-card p-[27px]"><h2>Page not found</h2><a href="#/">Return home</a></section>,
})
const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: '/', component: HomePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/accounts', component: AccountsPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/income', component: IncomePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: SettingsPage }),
])
const router = createRouter({ routeTree, history: createHashHistory() })

declare module '@tanstack/react-router' {
  interface Register { router: typeof router }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode><RouterProvider router={router} /></React.StrictMode>,
)
