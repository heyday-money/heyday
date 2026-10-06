import { t as translate, useLanguage } from "../lib/i18n"
import { Button } from './ui/button'
import { NativeSelect } from './ui/native-select'
import { useState } from 'react'
import { updateCurrency, type Settings } from '../lib/desktop'
import { currencies } from '../lib/money'
import { toast } from 'sonner'

export function CurrencySetting({ settings, onChange }: { settings: Settings; onChange: (settings: Settings) => void }) {
  useLanguage()

  const [currency, setCurrency] = useState(settings.currency ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  return <form className="my-5 border-b border-line pb-5" onSubmit={async event => {
    event.preventDefault(); setBusy(true); setError(null)
    try { onChange(await updateCurrency(currency)); toast.success(translate("Currency saved."), { id: 'currency-saved' }) }
    catch (error) { setError(typeof error === 'string' ? error : "Could not save currency. Please try again.") }
    finally { setBusy(false) }
  }}>
    <h3 className="text-base font-semibold">{translate("Currency")}</h3>
    <div className="mt-4 flex flex-wrap items-center justify-between gap-5 text-sm">
      <label htmlFor="currency">{translate("Currency")}</label>
      <div className="w-40 max-w-full"><NativeSelect id="currency" required disabled={busy} value={currency} onChange={event => { setCurrency(event.target.value); setError(null) }}>
        <option value="" disabled>{translate("Select currency")}</option>
        {currencies.map(code => <option key={code}>{code}</option>)}
      </NativeSelect></div>
    </div>
    <p className="mt-3 text-[13px]">{translate("Used for all accounts and income. Once you add financial records, the currency is locked.")}</p>
    <Button className="mt-4" disabled={busy || !currency || currency === settings.currency}>{busy ? translate("Saving…") : translate("Save currency")}</Button>
    {error && <p className="mt-3 text-[14px]" role="alert">{translate(error)}</p>}
  </form>
}
