import { invoke } from '@tauri-apps/api/core'

export interface SelectiveDefault { account_id: string; start_month: string; end_month: string | null }
export interface SelectiveDefaultStatus {
  periods: SelectiveDefault[]
  period_start_day: number
  linked_items: { kind: string; name: string }[]
}
export const getSelectiveDefault = (accountId: string) => invoke<SelectiveDefaultStatus>('get_selective_default', { accountId })
export async function saveSelectiveDefault(input: { account_id: string; action: 'exclude' | 'resume'; month: string; expected_periods: SelectiveDefault[]; confirmed: boolean }) {
  await invoke('save_selective_default', { input })
  for (const event of ['accounts-changed', 'plans-changed']) window.dispatchEvent(new Event(event))
}
export function exclusionFor(periods: SelectiveDefault[] | undefined, accountId: string | null | undefined, month: string) {
  return periods?.find(p => p.account_id === accountId && p.start_month <= month && (!p.end_month || month < p.end_month))
}
// Stable start-month keys use independently clamped payday boundaries.
export function cycleForDate(date: string, day: number) {
  const [year, month, d] = date.split('-').map(Number)
  const last = new Date(0); last.setFullYear(year, month, 0)
  if (d >= Math.min(day, last.getDate())) return date.slice(0, 7)
  return month === 1 ? `${String(year - 1).padStart(4, '0')}-12` : `${String(year).padStart(4, '0')}-${String(month - 1).padStart(2, '0')}`
}
export function excludedOnDate(periods: SelectiveDefault[] | undefined, accountId: string, date: string, day: number) {
  return !!exclusionFor(periods, accountId, cycleForDate(date, day))
}
export function includedCycleCount(periods: SelectiveDefault[] | undefined, accountId: string, from: string, to: string) {
  const index = (month: string) => Number(month.slice(0, 4)) * 12 + Number(month.slice(5)) - 1
  let count = index(to) - index(from)
  for (const p of periods ?? []) if (p.account_id === accountId) {
    count -= Math.max(0, Math.min(index(to), p.end_month ? index(p.end_month) : index(to)) - Math.max(index(from), index(p.start_month)))
  }
  return count
}
