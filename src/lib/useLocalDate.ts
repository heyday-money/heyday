import { useEffect, useState } from 'react'

// Read the OS clock through the WebView, using local calendar fields, never UTC.
export function useLocalDate() {
  const [today, setToday] = useState(() => new Date())
  useEffect(() => {
    const refresh = () => {
      const now = new Date()
      setToday(previous => previous.toDateString() === now.toDateString() && previous.getTimezoneOffset() === now.getTimezoneOffset() ? previous : now)
    }
    refresh()
    const timer = window.setInterval(refresh, 30_000)
    window.addEventListener('focus', refresh)
    document.addEventListener('visibilitychange', refresh)
    return () => {
      clearInterval(timer)
      window.removeEventListener('focus', refresh)
      document.removeEventListener('visibilitychange', refresh)
    }
  }, [])
  return today
}
