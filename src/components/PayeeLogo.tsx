import { t as translate, useLanguage } from "../lib/i18n"
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { desktopAvailable, listTransactionOptions, type TransactionOption } from '../lib/desktop'
import { LogoImage, useLogoAsset } from './LogoImage'
const Context = createContext<TransactionOption[]>([])
export function PayeeLogoProvider({ children }: { children: ReactNode }) {
  useLanguage()

  const [payees, setPayees] = useState<TransactionOption[]>([])
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true, request = 0
    const load = async () => {
      const current = ++request
      try { const data = await listTransactionOptions(); if (active && current === request) setPayees(data.payees) }
      catch { /* Names from financial snapshots and initials remain available. */ }
    }
    void load()
    window.addEventListener('transaction-options-changed', load); window.addEventListener('focus', load)
    return () => { active = false; window.removeEventListener('transaction-options-changed', load); window.removeEventListener('focus', load) }
  }, [])
  return <Context.Provider value={payees}>{children}</Context.Provider>
}
export function PayeeLogo({ id, name, assetId, className }: { id?: string | null; name: string; assetId?: string | null; className?: string }) {
  useLanguage()

  const payees = useContext(Context)
  const payee = payees.find(p => p.id === id)
  const source = useLogoAsset(assetId === undefined ? payee?.logo_asset_id : assetId)
  return <LogoImage src={source} name={payee?.name ?? name} className={className} />
}
export function PayeeLabel({ id, name }: { id?: string | null; name: string }) {
  useLanguage()

  return <span className="inline-flex min-w-0 items-center gap-2"><PayeeLogo id={id} name={name} /><span className="min-w-0 break-words">{name}</span></span>
}
