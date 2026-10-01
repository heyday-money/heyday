import { setLanguage, t as translate, useLanguage } from '../lib/i18n'
import { NativeSelect } from './ui/native-select'
export function LanguagePicker() {
  const language = useLanguage()
  return <div className="mb-6 border-b border-line pb-6">
    <label htmlFor="interface-language" className="text-base font-semibold">{translate('Language')}</label>
    <p className="mt-1 text-xs text-muted">{translate('Applies immediately on this device. Your records and currency stay unchanged.')}</p>
    <div className="mt-3 w-full sm:w-60"><NativeSelect id="interface-language" className="w-full" value={language} onChange={event => setLanguage(event.target.value === 'th' ? 'th' : 'en')}>
      <option value="en" lang="en">English</option><option value="th" lang="th">ไทย</option>
    </NativeSelect></div>
  </div>
}
