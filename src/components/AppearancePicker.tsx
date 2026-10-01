import { Check } from 'lucide-react'
import { appearances, type Appearance } from '../lib/appearance'

const labels: Record<Appearance, string> = { light: 'Light', dark: 'Dark', heyday: 'Heyday' }
const descriptions: Record<Appearance, string> = {
  light: 'White & charcoal', dark: 'Neutral & focused', heyday: 'Purple & yellow',
}

export function AppearancePicker({ value, onChange }: {
  value: Appearance
  onChange: (value: Appearance) => void
}) {
  return <fieldset className="@container border-b border-line pb-6">
    <legend className="text-base font-semibold">Appearance</legend>
    <p className="mt-1 text-xs">Choose your look. Applies immediately and stays saved on this device.</p>
    <div className="mt-4 grid grid-cols-1 gap-3 @min-[600px]:grid-cols-3">
      {appearances.map(appearance => <label key={appearance} className="relative block min-w-0 cursor-pointer">
        <input type="radio" name="appearance" value={appearance} checked={value === appearance}
          onChange={() => onChange(appearance)} aria-label={labels[appearance]}
          className="peer absolute inset-0 z-10 m-0 size-full cursor-pointer opacity-0" />
        <div className="rounded-xl border border-line p-2 transition-colors hover:border-brand/50 peer-checked:border-brand peer-checked:ring-1 peer-checked:ring-brand peer-focus-visible:outline-2 peer-focus-visible:outline-offset-4 peer-focus-visible:outline-brand">
          <div data-theme={appearance} aria-hidden="true" className="flex h-32 overflow-hidden rounded-lg border border-line bg-page text-ink">
            <div className="flex w-8 shrink-0 flex-col items-center gap-3 border-r border-line bg-card py-3">
              <div className="size-3 rounded bg-primary" />
              <div className="size-3 rounded bg-soft ring-1 ring-line" />
              <div className="size-3 rounded bg-soft ring-1 ring-line" />
              <div className="mt-auto size-2 rounded-full bg-accent" />
            </div>
            <div className="min-w-0 flex-1 p-3">
              <div className="h-1.5 w-12 rounded bg-ink/70" />
              <div className="mt-3 rounded-md border border-line bg-card p-2">
                <div className="h-1 w-8 rounded bg-muted/40" />
                <div className="mt-1 text-[13px] font-semibold tabular-nums">24,850.00</div>
              </div>
              <div className="mt-2 flex justify-between border-b border-line pb-1"><div className="h-1 w-10 rounded bg-muted/30" /><div className="h-1 w-5 rounded bg-muted/30" /></div>
              <div className="mt-2 h-3 w-10 rounded bg-primary" />
            </div>
          </div>
          <div className="flex items-center justify-between gap-2 px-1 pt-3 pb-1">
            <span><span className="block text-sm font-medium">{labels[appearance]}</span><span className="mt-0.5 block text-xs text-muted">{descriptions[appearance]}</span></span>
            <span className={`flex size-5 shrink-0 items-center justify-center rounded-full border ${value === appearance ? 'border-primary bg-primary text-primary-foreground' : 'border-line'}`} aria-hidden="true">
              {value === appearance && <Check className="size-3" />}
            </span>
          </div>
        </div>
      </label>)}
    </div>
  </fieldset>
}
