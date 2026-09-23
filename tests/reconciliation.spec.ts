import { test, expect } from '@playwright/test'
import type { Account, NewTransaction, Transaction } from '../src/lib/desktop'
import type { ReconciliationSnapshot, ReconciliationHistory, ReconciliationHistoryEntry, VerificationEntry } from '../src/lib/reconciliation'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const base = { loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_millis: null }
    type State = { accounts: Account[]; transactions: Transaction[]; statuses: Record<string, VerificationEntry['status']>; history: (ReconciliationHistory & { account: string })[]; evidence: ReconciliationHistoryEntry[] }
    const transaction = (id: string, type: Transaction['type'], amount: string, description: string, destination: string | null = null): Transaction => ({ id, type, account_id: 'bank', account_name: 'Everyday bank', destination_account_id: destination, destination_account_name: destination ? 'Credit card' : null, amount, description, date: '2026-01-01', payee_id: null, payee_name: null, category_id: null, category_name: null })
    const initial: State = { accounts: [{ ...base, id: 'bank', name: 'Everyday bank', type: 'bank', opening_balance: '10000', current_balance: '6500' }, { ...base, id: 'card', name: 'Credit card', type: 'credit_card', opening_balance: '2000', current_balance: '1500' }, { ...base, id: 'wallet', name: 'TrueMoney', type: 'wallet', opening_balance: '5000', current_balance: '5000' }], transactions: [transaction('groceries', 'expense', '1000', 'Groceries'), transaction('pending', 'expense', '2000', 'Pending purchase'), transaction('transfer', 'repayment', '500', 'Card payment', 'card')], statuses: {}, history: [], evidence: [] }
    const get = (): State => JSON.parse(localStorage.getItem('reconciliation-test') ?? JSON.stringify(initial))
    const put = (state: State) => localStorage.setItem('reconciliation-test', JSON.stringify(state))
    const snapshot = (state: State, accountId: string): ReconciliationSnapshot => {
      const account = state.accounts.find(a => a.id === accountId)!
      const entries: VerificationEntry[] = state.transactions.filter(t => t.account_id === accountId || t.destination_account_id === accountId).map(t => {
        const inflow = BigInt(t.amount) * (t.destination_account_id === accountId || t.type === 'income' ? 1n : -1n)
        const change = account.type === 'credit_card' ? -inflow : inflow
        return { transaction_id: t.id, date: t.date, description: t.description, type: t.type, amount: t.amount, account_id: t.account_id, destination_account_id: t.destination_account_id, status: state.statuses[`${accountId}:${t.id}`] ?? 'uncleared', reconciliation_id: null, balance_change: change.toString() }
      })
      const uncleared = entries.filter(e => e.status === 'uncleared').reduce((n, e) => n + BigInt(e.balance_change), 0n)
      const history = state.history.filter(h => h.account === accountId)
      return { account_id: accountId, name: account.name, account_type: account.type as 'bank' | 'wallet' | 'credit_card', is_archived: false, currency: 'THB', opening_balance: account.opening_balance, working_balance: account.current_balance, cleared_balance: (BigInt(account.current_balance) - uncleared).toString(), uncleared_changes: uncleared.toString(), entries, history, history_entries: state.evidence.filter(e => history.some(h => h.id === e.reconciliation_id)), token: JSON.stringify(state) }
    }
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { accountId?: string; id?: string; confirmReconciled?: boolean; input?: Record<string, unknown> }) => {
      const state = get()
      const input = args?.input ?? {}
      const accountId = String(input.account_id ?? args?.accountId ?? '')
      switch (command) {
        case 'plugin:app|version': return '0.1.0'
        case 'get_settings': return { currency: 'THB', period_start_day: 1 }
        case 'list_accounts': return state.accounts
        case 'update_account': {
          if ('opening_balance' in input || 'current_balance' in input) throw 'Balances must not be editable.'
          const account = state.accounts.find(account => account.id === input.id)!
          Object.assign(account, input)
          state.transactions.forEach(transaction => {
            if (transaction.account_id === account.id) transaction.account_name = account.name
            if (transaction.destination_account_id === account.id) transaction.destination_account_name = account.name
          })
          put(state); return account
        }

        case 'list_transaction_options': return { payees: [], categories: [] }
        case 'list_transactions': return state.transactions.map(t => ({ ...t, has_reconciliation_history: state.evidence.some(e => e.transaction_id === t.id) }))
        case 'get_reconciliation': return snapshot(state, accountId)
        case 'set_transaction_verification': {
          for (const entry of input.entries as { transaction_id: string; expected_status: string }[]) {
            const key = `${accountId}:${entry.transaction_id}`
            if ((state.statuses[key] ?? 'uncleared') !== entry.expected_status) throw 'Verification changed. Refresh and review.'
            if (state.statuses[key] === 'reconciled') {
              if (!input.confirm_unlock) throw 'Confirm unlocking.'
              state.history.filter(h => h.account === accountId).forEach(h => { h.needs_review = true })
            }
            state.statuses[key] = input.status as VerificationEntry['status']
          }
          put(state); return
        }
        case 'finish_reconciliation': {
          if (sessionStorage.getItem('fail-reconciliation')) throw 'Account activity or verification changed. Refresh and review the new comparison before finishing.'
          const current = snapshot(state, accountId)
          if (current.token !== input.expected_token || current.cleared_balance !== input.posted_balance) throw 'Stale comparison'
          if (!current.history.length && !input.confirm_opening_balance) throw 'Confirm opening balance'
          const id = String(state.history.length + 1)
          state.history.unshift({ id, account: accountId, completed_at: '2026-09-23T06:00:00Z', confirmed_balance: String(input.posted_balance), opening_balance: current.opening_balance, needs_review: false })
          current.entries.filter(e => e.status === 'cleared').forEach(e => {
            state.statuses[`${accountId}:${e.transaction_id}`] = 'reconciled'
            state.evidence.push({ ...e, reconciliation_id: id })
          })
          put(state); return snapshot(state, accountId)
        }
        case 'create_transaction': {
          const newEntry = input as unknown as NewTransaction
          const id = `new-${state.transactions.length}`
          const account = state.accounts.find(a => a.id === newEntry.account_id)!
          const destination = state.accounts.find(a => a.id === newEntry.destination_account_id)
          const entry: Transaction = { ...newEntry, id, account_name: account.name, destination_account_name: destination?.name ?? null, payee_name: null, category_name: null }
          state.transactions.push(entry)
          for (const account of state.accounts.filter(a => a.id === entry.account_id || a.id === entry.destination_account_id)) {
            const inflow = BigInt(entry.amount) * (entry.type === 'income' || account.id === entry.destination_account_id ? 1n : -1n)
            account.current_balance = (BigInt(account.current_balance) + inflow * (account.type === 'credit_card' ? -1n : 1n)).toString()
          }
          for (const id of newEntry.cleared_account_ids ?? []) state.statuses[`${id}:${entry.id}`] = 'cleared'
          put(state); return entry
        }
        case 'delete_transaction': {
          const related = state.evidence.filter(e => e.transaction_id === args.id)
          if (related.length && !args.confirmReconciled) throw 'This transaction has reconciliation history. Confirm deletion.'
          related.forEach(e => { state.history.find(h => h.id === e.reconciliation_id)!.needs_review = true })
          for (const account of state.accounts) {
            const entry = snapshot(state, account.id).entries.find(e => e.transaction_id === args.id)
            if (entry) account.current_balance = (BigInt(account.current_balance) - BigInt(entry.balance_change)).toString()
          }
          state.transactions = state.transactions.filter(t => t.id !== args.id)
          put(state); return
        }
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
})

test('keyboard verification, bulk clearing, pending balances, locking, history, and corrections', async ({ page }) => {
  await page.goto('/#/accounts/bank')
  const table = page.getByRole('table', { name: 'Account transaction verification' })
  const working = page.getByText('Working balance', { exact: true }).locator('..')
  await expect(working).toContainText('65.00 THB')
  const grocery = page.getByRole('button', { name: /Uncleared: Groceries/ })
  await grocery.focus(); await page.keyboard.press('Space')
  await expect(page.getByRole('button', { name: /Cleared: Groceries/ })).toHaveAttribute('aria-pressed', 'true')
  await expect(working).toContainText('65.00 THB')
  await page.getByLabel('Verification status').selectOption('uncleared')
  await expect(table.locator('tbody tr')).toHaveCount(2)
  await page.getByLabel('Select Card payment 2026-01-01').check()
  await page.getByRole('button', { name: 'Clear selected (1)', exact: true }).click()
  await expect(table.locator('tbody tr')).toHaveCount(1)
  await page.getByLabel('Verification status').selectOption('')
  await page.getByRole('button', { name: 'Reconcile', exact: true }).click()
  await page.getByLabel('Bank posted balance (THB)').fill('84.99')
  await expect(page.getByRole('button', { name: 'Finish reconciliation', exact: true })).toBeDisabled()
  await page.getByLabel('Bank posted balance (THB)').fill('85')
  await expect(page.getByText('Difference: 0.00 THB · Matched')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Finish reconciliation', exact: true })).toBeDisabled()
  await page.getByLabel(/I confirm the opening balance/).check()
  await page.getByRole('button', { name: 'Finish reconciliation', exact: true }).click()
  await expect(page.getByRole('button', { name: /Unlock reconciled/ })).toHaveCount(2)
  await expect(page.getByRole('button', { name: /Uncleared: Pending purchase/ })).toBeVisible()
  await expect(working).toContainText('65.00 THB')
  await page.reload()
  await expect(page.getByRole('button', { name: /Unlock reconciled/ })).toHaveCount(2)
  await page.getByLabel('Hide reconciled', { exact: true }).check()
  await expect(table.locator('tbody tr')).toHaveCount(1)
  await page.getByLabel('Hide reconciled', { exact: true }).uncheck()
  await page.getByRole('button', { name: /Unlock reconciled Groceries/ }).click()
  const dialog = page.getByRole('dialog', { name: 'Unlock reconciled transaction?' })
  await expect(dialog.getByRole('button', { name: 'Unlock transaction', exact: true })).toBeDisabled()
  await dialog.getByLabel(/I confirm this correction/).check()
  await dialog.getByRole('button', { name: 'Unlock transaction', exact: true }).click()
  await expect(page.getByRole('button', { name: /Cleared: Groceries/ })).toBeVisible()
  await expect(page.getByRole('region', { name: 'Reconciliation history' })).toContainText('Needs review')
  await expect(working).toContainText('65.00 THB')
  // The other transfer leg was never inferred as cleared or reconciled.
  await page.goto('/#/accounts/card')
  await expect(page.getByRole('button', { name: /Uncleared: Card payment/ })).toBeVisible()
  await page.goto('/#/transactions')
  await page.getByRole('button', { name: 'Delete Card payment', exact: true }).click()
  const deletion = page.getByRole('dialog', { name: 'Delete transaction?' })
  await expect(deletion.getByRole('button', { name: 'Delete transaction', exact: true })).toBeDisabled()
  await deletion.getByLabel(/I confirm deleting this reconciled entry/).check()
  await deletion.getByRole('button', { name: 'Delete transaction', exact: true }).click()
  await expect(deletion).not.toBeVisible()
  await page.goto('/#/accounts/bank')
  await expect(working).toContainText('70.00 THB')
  await page.getByRole('region', { name: 'Reconciliation history' }).locator('summary').click()
  await expect(page.getByRole('region', { name: 'Reconciliation history' })).toContainText('Card payment')
})

test('record missing cleared entries during reconciliation and handle failed completion without losing the comparison', async ({ page }) => {
  await page.goto('/#/accounts/bank')
  await page.getByRole('button', { name: 'Reconcile', exact: true }).click()
  await page.getByLabel('Bank posted balance (THB)').fill('95')
  await page.getByRole('button', { name: 'Record missing entry', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Add transaction', exact: true })
  await expect(dialog.getByLabel('Account', { exact: true })).toHaveValue('bank')
  await expect(dialog.getByLabel('Cleared', { exact: true })).not.toBeChecked()
  await dialog.getByLabel('Amount (THB)', { exact: true }).fill('5')
  await dialog.getByLabel('Description (optional)').fill('Bank fee')
  await dialog.getByLabel('Cleared', { exact: true }).check()
  await dialog.getByRole('button', { name: 'Save transaction', exact: true }).click()
  await expect(page.getByLabel('Bank posted balance (THB)')).toHaveValue('95')
  await expect(page.getByText('Difference: 0.00 THB · Matched')).toBeVisible()
  await page.setViewportSize({ width: 760, height: 760 })
  expect(await page.locator('main').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.screenshot({ path: 'test-results/reconciliation.png', fullPage: true })
  await page.getByLabel(/I confirm the opening balance/).check()
  await page.evaluate(() => sessionStorage.setItem('fail-reconciliation', 'true'))
  await page.getByRole('button', { name: 'Finish reconciliation', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('changed')
  await expect(page.getByLabel('Bank posted balance (THB)')).toHaveValue('95')
  await expect(page.getByRole('button', { name: /Cleared: Bank fee/ })).toBeVisible()
  await expect(page.getByText('No completed reconciliations.')).toBeVisible()
  await page.evaluate(() => sessionStorage.removeItem('fail-reconciliation'))
  await page.getByRole('button', { name: 'Finish reconciliation', exact: true }).click()
  await expect(page.getByRole('button', { name: /Unlock reconciled Bank fee/ })).toBeVisible()
})

test('credit card reconciliation distinguishes owed balances from overpayments and clears transfer sides separately on entry', async ({ page }) => {
  await page.goto('/#/accounts/card')
  await page.getByRole('button', { name: /Uncleared: Card payment/ }).click()
  await page.getByRole('button', { name: 'Record missing entry', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Add transaction', exact: true })
  await dialog.getByLabel('Amount (THB)', { exact: true }).fill('30')
  await dialog.getByLabel('Transaction type').selectOption('income')
  await dialog.getByLabel('Cleared', { exact: true }).check()
  await dialog.getByRole('button', { name: 'Save transaction', exact: true }).click()
  await expect(page.getByText('Cleared balance', { exact: true }).locator('..')).toContainText('15.00 THB overpayment / credit')
  await page.getByRole('button', { name: 'Reconcile', exact: true }).click()
  await page.getByLabel('Bank posted balance (THB)').fill('15')
  await page.getByLabel(/I confirm the opening balance/).check()
  await expect(page.getByRole('button', { name: 'Finish reconciliation', exact: true })).toBeDisabled()
  await page.getByLabel('Posted card balance represents').selectOption('credit')
  await expect(page.getByText('Difference: 0.00 THB · Matched')).toBeVisible()
  await page.getByRole('button', { name: 'Finish reconciliation', exact: true }).click()
  await expect(page.getByRole('button', { name: /Unlock reconciled/ })).toHaveCount(2)
  await page.getByRole('button', { name: 'Record missing entry', exact: true }).click()
  await dialog.getByLabel('Amount (THB)', { exact: true }).fill('1')
  await dialog.getByLabel('Transaction type').selectOption('transfer')
  await dialog.getByLabel('From account', { exact: true }).selectOption('bank')
  await dialog.getByLabel('To account', { exact: true }).selectOption('card')
  await dialog.getByLabel('Cleared in from account').check()
  await expect(dialog.getByLabel('Cleared in to account')).not.toBeChecked()
  await dialog.getByRole('button', { name: 'Save transaction', exact: true }).click()
  await expect(page.getByRole('button', { name: /Uncleared: transfer/ })).toBeVisible()
  await page.goto('/#/accounts/bank')
  await expect(page.getByRole('button', { name: /Cleared: transfer/ })).toBeVisible()
})


test('digital wallet account links, cleared entry, and reconciliation use wallet balances', async ({ page }) => {
  await page.goto('/#/accounts')
  const wallet = page.getByRole('listitem').filter({ has: page.getByRole('heading', { name: 'TrueMoney', exact: true }) })
  await wallet.getByRole('link', { name: 'Transactions & reconciliation' }).click()
  await expect(page.getByRole('heading', { name: 'TrueMoney', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Record missing entry', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Add transaction', exact: true })
  await expect(dialog.getByLabel('Account', { exact: true })).toHaveValue('wallet')
  await dialog.getByLabel('Amount (THB)', { exact: true }).fill('5')
  await dialog.getByLabel('Description (optional)').fill('Wallet purchase')
  await dialog.getByLabel('Cleared', { exact: true }).check()
  await dialog.getByRole('button', { name: 'Save transaction', exact: true }).click()
  const working = page.getByText('Working balance', { exact: true }).locator('..')
  await expect(working).toContainText('45.00 THB')
  await page.getByRole('button', { name: 'Reconcile', exact: true }).click()
  await page.getByLabel('Wallet posted balance (THB)').fill('45')
  await expect(page.getByLabel('Posted card balance represents')).toHaveCount(0)
  await page.getByLabel(/I confirm the opening balance/).check()
  await page.getByRole('button', { name: 'Finish reconciliation', exact: true }).click()
  await expect(page.getByRole('button', { name: /Unlock reconciled Wallet purchase/ })).toBeVisible()
  await expect(working).toContainText('45.00 THB')
  await page.reload()
  await expect(page.getByRole('button', { name: /Unlock reconciled Wallet purchase/ })).toBeVisible()
})


test('Transactions starts reconciliation for the selected account independently of history filters', async ({ page }) => {
  await page.goto('/#/transactions')
  await expect(page.getByRole('button', { name: 'Reconcile account', exact: true })).toBeDisabled()
  await page.getByLabel('Filter by account', { exact: true }).selectOption('bank')
  await page.getByLabel('Filter by transaction type', { exact: true }).selectOption('income')
  await expect(page.getByText('No matching transactions', { exact: true })).toBeVisible()
  await page.getByRole('link', { name: 'Reconcile account', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Everyday bank', exact: true })).toBeVisible()
  await expect(page.getByLabel('Bank posted balance (THB)')).toBeVisible()
  await expect(page.getByRole('table', { name: 'Account transaction verification' }).locator('tbody tr')).toHaveCount(3)
  await page.goto('/#/transactions')
  await page.getByLabel('Filter by account', { exact: true }).selectOption('wallet')
  await page.getByRole('link', { name: 'Reconcile account', exact: true }).click()
  await expect(page.getByLabel('Wallet posted balance (THB)')).toBeVisible()
})


test('account page edits account details in place and preserves the reconciliation draft', async ({ page }) => {
  await page.goto('/#/accounts/bank?reconcile=true')
  await page.getByLabel('Bank posted balance (THB)').fill('100')
  await page.getByRole('button', { name: 'Edit account', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Edit account', exact: true })
  await expect(dialog.getByLabel('Account name', { exact: true })).toHaveValue('Everyday bank')
  await expect(dialog.getByLabel('Opening balance (THB)')).toHaveValue('100.00')
  await expect(dialog.getByLabel('Opening balance (THB)')).toHaveAttribute('readonly', '')
  await dialog.getByLabel('Account name', { exact: true }).fill('Main bank')
  await dialog.getByLabel('Last four digits (optional)').fill('0019')
  await dialog.getByLabel('Institution (optional)').fill('My bank')
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(dialog.getByText('Discard your unsaved account?')).toBeVisible()
  await dialog.getByRole('button', { name: 'Keep editing', exact: true }).click()
  await dialog.getByRole('button', { name: 'Save account', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(page.getByRole('heading', { name: 'Main bank', exact: true })).toBeVisible()
  await expect(page.getByLabel('Bank posted balance (THB)')).toHaveValue('100')
  await expect(page.getByText('Working balance', { exact: true }).locator('..')).toContainText('65.00 THB')
  await page.reload()
  await page.getByRole('button', { name: 'Edit account', exact: true }).click()
  await expect(dialog.getByLabel('Last four digits (optional)')).toHaveValue('0019')
  await expect(dialog.getByLabel('Institution (optional)')).toHaveValue('My bank')
})
