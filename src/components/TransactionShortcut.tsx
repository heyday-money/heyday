import { t as translate, useLanguage } from "../lib/i18n"
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react'
import { useParams } from '@tanstack/react-router'
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
  useLanguage()

  const accountId = useParams({ strict: false, select: params => params.accountId })
  const [entry, setEntry] = useState<{ accountId?: string } | null>(null)
  const requested = entry !== null
  // Capture the page context only when opening; later navigation cannot reset a draft.
  const setRequested = useCallback((next: boolean) => {
    setEntry(current => next ? current ?? { accountId } : null)
  }, [accountId])

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
  }, [setRequested])

  return <TransactionShortcutContext.Provider value={{ requested, setRequested }}>{children}{entry && <AddTransactionDialog initialAccountId={entry.accountId} onClose={() => setRequested(false)} />}</TransactionShortcutContext.Provider>
}

export function AddTransactionButton() {
  useLanguage()

  const { setRequested } = useContext(TransactionShortcutContext)
  return <Button className="bg-accent text-accent-foreground hover:bg-accent/90" disabled={!desktopAvailable} onClick={() => setRequested(true)} title={translate("Add transaction ({value0})", { value0: transactionShortcutLabel })} aria-keyshortcuts={isMac ? 'Meta+N' : 'Control+N'}>
    <Plus size={17} aria-hidden="true" />{translate("Add transaction")}</Button>
}
