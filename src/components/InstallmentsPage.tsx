import { t as translate, useLanguage } from "../lib/i18n"
import { Link } from '@tanstack/react-router'
import { desktopAvailable } from '../lib/desktop'
import { useFinancialData } from '../lib/useFinancialData'
import { InstallmentsTable } from './InstallmentsTable'
import { Button } from './ui/button'

export function InstallmentsPage() {
  useLanguage()

  const { data, loading, error, today, reload } = useFinancialData()
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage your local installment plans.")}</p>
  if (error) return <div role="alert">{translate("Could not load installment plans.")}{" "}<Button variant="outline" onClick={reload}>{translate("Retry installments")}</Button></div>
  if (!data) return <p role="status">{translate("Loading installment plans…")}</p>
  if (!data.settings.currency) return <p className="rounded-xl bg-soft p-5">{translate("Choose your currency in")}{" "}<Link to="/settings" className="text-brand">{translate("Settings")}</Link> {" "}{translate("to manage installment plans.")}</p>
  return <InstallmentsTable data={data} today={today} loading={loading} />
}
