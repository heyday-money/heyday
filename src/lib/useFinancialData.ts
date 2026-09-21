import { useEffect, useState } from 'react'
import { desktopAvailable, getFinancialData, type FinancialData } from './desktop'
import { dateKey } from './financial'

export function useFinancialData() {
  const [data, setData] = useState<FinancialData | null>(null)
  const [loading, setLoading] = useState(desktopAvailable)
  const [error, setError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [today, setToday] = useState(() => new Date())
  useEffect(() => {
    if (!desktopAvailable) return
    let active = true, request = 0
    let lastDay = dateKey(new Date())
    const load = async () => {
      const current = ++request
      setLoading(true); setError(false)
      try {
        const value = await getFinancialData()
        if (active && current === request) { setData(value); setToday(new Date()) }
      } catch { if (active && current === request) setError(true) }
      finally { if (active && current === request) setLoading(false) }
    }
    const dayChanged = () => {
      const day = dateKey(new Date())
      if (day !== lastDay) { lastDay = day; void load() }
    }
    void load()
    const events = ['accounts-changed', 'transactions-changed', 'plans-changed', 'transaction-options-changed']
    events.forEach(event => window.addEventListener(event, load))
    window.addEventListener('focus', dayChanged)
    const timer = window.setInterval(dayChanged, 60000)
    return () => { active = false; events.forEach(event => window.removeEventListener(event, load)); window.removeEventListener('focus', dayChanged); clearInterval(timer) }
  }, [attempt])
  return { data, loading, error, today, reload: () => setAttempt(value => value + 1) }
}
