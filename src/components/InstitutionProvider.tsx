import { t as translate, useLanguage } from "../lib/i18n"
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { desktopAvailable } from '../lib/desktop'
import { listInstitutions, type InstitutionDirectory } from '../lib/institutions'
const empty: InstitutionDirectory = { institutions: [], accounts: [] }
const Context = createContext({ ...empty, loading: true, error: false, reload: () => {} })
export const useInstitutions = () => useContext(Context)
export function InstitutionProvider({ children }: { children: ReactNode }) {
  useLanguage()

  const [data, setData] = useState(empty)
  const [loading, setLoading] = useState(desktopAvailable)
  const [error, setError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const reload = useCallback(() => setAttempt(n => n + 1), [])
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true, request = 0
    const load = async () => {
      const current = ++request
      try {
        const value = await listInstitutions()
        if (active && current === request) { setData(value); setError(false) }
      } catch { if (active && current === request) setError(true) }
      finally { if (active && current === request) setLoading(false) }
    }
    void load()
    window.addEventListener('accounts-changed', load)
    window.addEventListener('focus', load)
    return () => { active = false; window.removeEventListener('accounts-changed', load); window.removeEventListener('focus', load) }
  }, [attempt])
  return <Context.Provider value={{ ...data, loading, error, reload }}>{children}</Context.Provider>
}
