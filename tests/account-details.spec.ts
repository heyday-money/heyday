import { expect, test } from '@playwright/test'

test.use({ channel: 'chrome' })

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const account = (id: string, name: string, type = 'bank') => ({ id, name, type, currency: 'THB', opening_balance: '100000', current_balance: '123450', institution: null, last_four: null, notes: null, loan_type: type === 'loan' ? 'personal_loan' : null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null, monthly_installment: null, is_archived: id === 'bank' && !!sessionStorage.getItem('archived') })
    const accounts = () => [account('bank', 'Everyday'), account('other', 'Other savings'), account('loan', 'Home loan', 'loan'), account('empty', 'Empty wallet', 'cash')]
    const base = { account_id: 'bank', account_name: 'Everyday', destination_account_id: null, destination_account_name: null, type: 'expense', amount: '1234', date: '2026-10-20', payee_id: null, payee_name: null, category_id: null, category_name: null }
    const seed = [
      { ...base, id: 'unrelated', account_id: 'other', account_name: 'Other savings', description: 'Other account only', date: '2026-10-31', category_id: 'other-category', category_name: 'Other category' },
      { ...base, id: 'incoming', type: 'transfer', account_id: 'other', account_name: 'Other savings', destination_account_id: 'bank', destination_account_name: 'Everyday', description: 'Moved into Everyday', date: '2026-10-30' },
      { ...base, id: 'principal', type: 'repayment', destination_account_id: 'loan', destination_account_name: 'Home loan', loan_account_id: 'loan', loan_account_name: 'Home loan', description: 'Loan principal', date: '2026-10-29' },
      { ...base, id: 'interest', loan_account_id: 'loan', loan_account_name: 'Home loan', description: 'Loan interest', date: '2026-10-29', category_id: 'charges', category_name: 'Loan charges', has_reconciliation_history: true },
      { ...base, id: 'fee', account_id: 'other', account_name: 'Other savings', loan_account_id: 'loan', loan_account_name: 'Home loan', description: 'Imported loan fee', date: '2026-10-29', imported_linked_payment: true },
      { ...base, id: 'payroll', type: 'income', account_id: 'loan', account_name: 'Home loan', description: 'Payroll principal', date: '2026-10-28', salary_payment_role: 'principal', salary_payment_id: 'salary-1' },
      ...Array.from({ length: 11 }, (_, i) => ({ ...base, id: `expense-${i}`, description: `Everyday expense ${i}`, date: `2026-10-${String(20 - i).padStart(2, '0')}` })),
    ]
    if (!sessionStorage.getItem('detail-transactions')) sessionStorage.setItem('detail-transactions', JSON.stringify(seed))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: any = {}) => {
      switch (command) {
        case 'plugin:app|version': return 'test'
        case 'get_settings': return { currency: 'THB', period_start_day: 1 }
        case 'list_accounts': return accounts()
        case 'list_institutions': return { institutions: [], accounts: [] }
        case 'list_card_limit_groups': return { groups: [], cards: [] }
        case 'list_transactions':
          if (sessionStorage.getItem('fail-history')) throw 'History unavailable'
          return JSON.parse(sessionStorage.getItem('detail-transactions')!)
        case 'get_loan_account': return { account: accounts().find(a => a.id === args.accountId), currency: 'THB', facility: null, contracts: [], transactions: [], payment_parts: [] }
        case 'get_selective_default': return { periods: [], period_start_day: 1, linked_items: [] }
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
})

test('account history paginates, keeps filters scoped, and refreshes after synced changes', async ({ page }) => {
  await page.goto('/#/accounts/bank/details')
  const history = page.getByRole('region', { name: 'Transaction history', exact: true })
  const table = history.getByRole('table', { name: 'Transaction history', exact: true })
  await expect(table.locator('tbody tr')).toHaveCount(10)
  await expect(history.getByLabel('Filter by account')).toHaveCount(0)
  await expect(history).toContainText('Showing 1–10 of 14 transactions')
  await expect(table.getByRole('row').filter({ hasText: 'Moved into Everyday' })).toContainText('Transfer in')
  await expect(table.getByRole('row').filter({ hasText: 'Loan principal' })).toContainText('Transfer out')
  await expect(history.getByLabel('Filter by category').getByRole('option', { name: 'Other category', exact: true })).toHaveCount(0)
  await history.getByRole('button', { name: 'Next page', exact: true }).click()
  await expect(table.locator('tbody tr')).toHaveCount(4)
  await history.getByLabel('Filter by transaction type').selectOption('repayment')
  await expect(table.locator('tbody tr')).toHaveCount(1)
  await expect(history).toContainText('Showing 1–1 of 1 transactions')
  await history.getByRole('button', { name: 'Clear filters', exact: true }).click()
  await expect(history).toContainText('Showing 1–10 of 14 transactions')
  await expect(table).not.toContainText('Other account only')
  await page.screenshot({ path: 'test-results/account-detail-history.png', fullPage: true, animations: 'disabled' })
  await page.evaluate(() => {
    const rows = JSON.parse(sessionStorage.getItem('detail-transactions')!)
    rows.unshift({ ...rows.find((r: any) => r.id === 'expense-0'), id: 'synced', date: '2026-11-01', description: 'Arrived from another device' })
    sessionStorage.setItem('detail-transactions', JSON.stringify(rows))
    window.dispatchEvent(new Event('transactions-changed'))
  })
  await expect(table.getByRole('rowheader').first()).toHaveText('Arrived from another device')
  await expect(history).toContainText('Showing 1–10 of 15 transactions')
  await history.getByRole('link', { name: 'View all transactions', exact: true }).click()
  await expect(page).toHaveURL(/#\/transactions\?account=bank$/)
  await expect(page.getByLabel('Filter by account')).toHaveValue('bank')
  await expect(page.getByRole('table').locator('tbody tr')).toHaveCount(15)
})

test('loan history includes linked charges and payroll, preserving correction safeguards', async ({ page }) => {
  await page.goto('/#/accounts/loan/details')
  const history = page.getByRole('region', { name: 'Transaction history', exact: true })
  const table = history.getByRole('table')
  await expect(table.locator('tbody tr')).toHaveCount(4)
  await expect(table).toContainText('Loan principal')
  await expect(table).toContainText('Loan interest')
  await expect(table).toContainText('Imported loan fee')
  await expect(table).toContainText('Payroll repayment')
  await history.getByRole('button', { name: 'Delete Loan interest', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Delete transaction?', exact: true })
  await expect(dialog).toContainText('all principal, interest and fee components are reversed together')
  await expect(dialog.getByRole('button', { name: 'Delete transaction', exact: true })).toBeDisabled()
  await dialog.getByRole('checkbox').check()
  await expect(dialog.getByRole('button', { name: 'Delete transaction', exact: true })).toBeEnabled()
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await history.getByRole('button', { name: 'Delete Imported loan fee', exact: true }).click()
  await expect(dialog.getByRole('button', { name: 'Delete transaction', exact: true })).toBeDisabled()
  await expect(dialog).toContainText('Delete this linked payment on the device')
})

test('history retries independently, retains archived activity, and handles an empty account', async ({ page }) => {
  await page.addInitScript(() => { sessionStorage.setItem('archived', '1'); sessionStorage.setItem('fail-history', '1') })
  await page.goto('/#/accounts/bank/details')
  await expect(page.getByRole('heading', { name: 'Everyday', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Restore account Everyday', exact: true })).toBeVisible()
  const history = page.getByRole('region', { name: 'Transaction history', exact: true })
  await expect(history.getByRole('alert')).toContainText('Could not load transactions.')
  await page.evaluate(() => sessionStorage.removeItem('fail-history'))
  await history.getByRole('button', { name: 'Try again', exact: true }).click()
  await expect(history.getByRole('table').locator('tbody tr')).toHaveCount(10)
  await page.evaluate(() => { location.hash = '/accounts/empty/details' })
  await expect(page.getByRole('heading', { name: 'Empty wallet', exact: true })).toBeVisible()
  await expect(history.getByText('No transactions yet', { exact: true })).toBeVisible()
  await expect(history.getByRole('table')).toHaveCount(0)
})

test('Thai history fits a narrow window and keeps table scrolling within the page', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('heyday-language', 'th'))
  await page.setViewportSize({ width: 760, height: 650 })
  await page.goto('/#/accounts/bank/details')
  const history = page.getByRole('region', { name: 'ประวัติธุรกรรม', exact: true })
  await expect(history.getByRole('table').locator('tbody tr')).toHaveCount(10)
  await expect(history).toContainText('แสดงธุรกรรม 1–10 จากทั้งหมด 14 รายการ')
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await history.getByRole('button', { name: 'หน้าถัดไป', exact: true }).click()
  await expect(history).toContainText('แสดงธุรกรรม 11–14 จากทั้งหมด 14 รายการ')
  await page.screenshot({ path: 'test-results/account-detail-history-thai.png', fullPage: true, animations: 'disabled' })
})
