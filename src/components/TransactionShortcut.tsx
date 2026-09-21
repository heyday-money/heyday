import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { Plus } from 'lucide-react'
import { Button } from './ui/button'
import { AddTransactionDialog } from './AddTransactionDialog'
import { desktopAvailable } from '../lib/desktop'

export const TransactionShortcutContext = createContext({
  requested: false,
  setRequested: (_requested: boolean) => {},
})

export const isMac = /Mac|iPhone|iPad/.test(navigator.platform)
export const transactionShortcutLabel = isMac ? '⌘N' : 'Ctrl+N'

export function TransactionShortcut({ children }: { children: ReactNode }) {
  const [requested, setRequested] = useState(false)

  useEffect(() => {
    function keydown(event: KeyboardEvent) {
      if (!desktopAvailable || event.defaultPrevented || event.isComposing || event.key.toLowerCase() !== 'n'
        || event.altKey || event.shiftKey || (isMac ? !event.metaKey || event.ctrlKey : !event.ctrlKey || event.metaKey)) return
      event.preventDefault()
      // Preserve any open form, including unsaved account/income dialogs.
      if (event.repeat || document.querySelector('[role="dialog"], [role="alertdialog"]')) return
      setRequested(true)
    }
    window.addEventListener('keydown', keydown)
    return () => window.removeEventListener('keydown', keydown)
  }, [])

  return <TransactionShortcutContext.Provider value={{ requested, setRequested }}>{children}{requested && <AddTransactionDialog onClose={() => setRequested(false)} />}</TransactionShortcutContext.Provider>
}

export function AddTransactionButton() {
  const { setRequested } = useContext(TransactionShortcutContext)
  return <Button className="bg-accent text-accent-foreground hover:bg-accent/90" disabled={!desktopAvailable} onClick={() => setRequested(true)} title={`Add transaction (${transactionShortcutLabel})`} aria-keyshortcuts={isMac ? 'Meta+N' : 'Control+N'}>
    <Plus size={17} aria-hidden="true" />Add transaction
  </Button>
}
