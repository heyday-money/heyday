import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useRef, useState } from 'react'
import { desktopAvailable } from './desktop'
export interface CardLimitGroup { id: string; name: string; credit_limit: string }
export interface LimitCard { id: string; name: string; current_balance: string; is_archived: boolean; group_id: string | null }
export interface CardLimits { groups: CardLimitGroup[]; cards: LimitCard[] }
export function sharedCredit(data: CardLimits, accountId: string) {
  const id = data.cards.find(c => c.id === accountId)?.group_id
  const group = data.groups.find(g => g.id === id)
  if (!group) return null
  const balance = data.cards.filter(c => c.group_id === group.id).reduce((sum, c) => sum + BigInt(c.current_balance), 0n)
  return { ...group, balance, available: BigInt(group.credit_limit) - balance }
}
export function sharedCreditAfter(data: CardLimits, groupId: string, kind: string, source: string, destination: string, amount: bigint) {
  const group = data.groups.find(g => g.id === groupId)!
  const members = data.cards.filter(c => c.group_id === groupId)
  const current = members.reduce((sum, c) => sum + BigInt(c.current_balance), 0n)
  const sourceChange = members.some(c => c.id === source) ? (kind === 'income' ? -amount : amount) : 0n
  const destinationChange = (kind === 'transfer' || kind === 'repayment') && members.some(c => c.id === destination) ? -amount : 0n
  return BigInt(group.credit_limit) - current - sourceChange - destinationChange
}
export function useCardLimits() {
  const [data, setData] = useState<CardLimits>({ groups: [], cards: [] })
  const [loading, setLoading] = useState(desktopAvailable)
  const [error, setError] = useState(false)
  const sequence = useRef(0)
  const reload = useCallback(async () => {
    if (!desktopAvailable) return
    const request = ++sequence.current
    setLoading(true)
    try {
      const result = await invoke<CardLimits>('list_card_limit_groups')
      if (request === sequence.current) { setData(result); setError(false) }
    } catch { if (request === sequence.current) setError(true) }
    finally { if (request === sequence.current) setLoading(false) }
  }, [])
  useEffect(() => {
    void reload()
    window.addEventListener('accounts-changed', reload)
    window.addEventListener('focus', reload)
    return () => { sequence.current++; window.removeEventListener('accounts-changed', reload); window.removeEventListener('focus', reload) }
  }, [reload])
  return { data, loading, error, reload }
}
export async function saveCardLimitGroup(input: { id: string | null; name: string; credit_limit: string; currency: string; account_ids: string[] }) {
  await invoke('save_card_limit_group', { input })
  window.dispatchEvent(new Event('accounts-changed'))
}
export async function deleteCardLimitGroup(id: string) {
  await invoke('delete_card_limit_group', { id })
  window.dispatchEvent(new Event('accounts-changed'))
}
