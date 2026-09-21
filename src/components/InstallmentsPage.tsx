import { Link } from '@tanstack/react-router'
import { desktopAvailable } from '../lib/desktop'
import { useFinancialData } from '../lib/useFinancialData'
import { InstallmentsTable } from './InstallmentsTable'
import { Button } from './ui/button'

export function InstallmentsPage() {
  const { data, loading, error, today, reload } = useFinancialData()
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">Open the desktop app to manage your local installment plans.</p>
  if (error) return <div role="alert">Could not load installment plans. <Button variant="outline" onClick={reload}>Retry installments</Button></div>
  if (!data) return <p role="status">Loading installment plans…</p>
  if (!data.settings.currency) return <p className="rounded-xl bg-soft p-5">Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to manage installment plans.</p>
  return <InstallmentsTable data={data} today={today} loading={loading} />
}
