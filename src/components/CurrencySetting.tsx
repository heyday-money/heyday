import { useState } from 'react'
import { updateCurrency, type Settings } from '../lib/desktop'
import { currencies } from '../lib/money'
import { toast } from 'sonner'

export function CurrencySetting({ settings, onChange }: { settings: Settings; onChange: (settings: Settings) => void }) {
  const [currency, setCurrency] = useState(settings.currency ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  return <form className="my-5 border-b border-line pb-5" onSubmit={async event => {
    event.preventDefault(); setBusy(true); setError(null)
    try { onChange(await updateCurrency(currency)); toast.success('Currency saved.', { id: 'currency-saved' }) }
    catch (error) { setError(typeof error === 'string' ? error : 'Could not save currency. Please try again.') }
    finally { setBusy(false) }
  }}>
    <div className="flex items-center justify-between gap-5 text-[14px]">
      <label htmlFor="currency">Currency</label>
      <select id="currency" required disabled={busy} value={currency} onChange={event => { setCurrency(event.target.value); setError(null) }} className="rounded-[9px] border border-line bg-page px-3 py-2 text-ink">
        <option value="" disabled>Select currency</option>
        {currencies.map(code => <option key={code}>{code}</option>)}
      </select>
    </div>
    <p className="mt-3 text-[13px]">Used for all accounts and income. Once you add financial records, the currency is locked.</p>
    <button className="mt-4 rounded-[10px] bg-brand px-4 py-2.5 text-[12px] font-semibold text-white" disabled={busy || !currency || currency === settings.currency}>{busy ? 'Saving…' : 'Save currency'}</button>
    {error && <p className="mt-3 text-[14px]" role="alert">{error}</p>}
  </form>
}
