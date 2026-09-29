import { expect, test } from '@playwright/test'

test('mark paid off validates zero, preserves failed confirmation, and retains history access', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const accounts = () => [{ id: 'loan', name: 'Finished loan', type: 'loan', loan_type: 'personal_loan', current_balance: '0', opening_balance: '10000', monthly_installment: '100', paid_off_on: localStorage.getItem('paid-off') }, { id: 'owed', name: 'Unpaid loan', type: 'loan', loan_type: 'mortgage', current_balance: '100', opening_balance: '100', monthly_installment: '100' }].map(a => ({ ...a, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null }))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: any) => {
      if (command === 'plugin:app|version') return '0.0.1-alpha.5'
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 25 }
      if (command === 'list_accounts') return accounts().filter(a => args?.includePaidOff || !a.paid_off_on)
      if (command === 'list_institutions') return { institutions: [], accounts: [] }
      if (command === 'list_card_limit_groups') return { groups: [], cards: [] }
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'mark_loan_paid_off') {
        if (sessionStorage.getItem('fail-payoff')) throw 'Balance changed. Record the final repayment first.'
        if (sessionStorage.getItem('hold-payoff')) await new Promise<void>(resolve => window.addEventListener('release-payoff', () => resolve(), { once: true }))
        localStorage.setItem('paid-off', '2026-09-29'); return
      }
      if (command === 'get_loan_account') return { account: accounts().find(a => a.id === args.accountId), currency: 'THB', paid_off_on: args.accountId === 'loan' ? localStorage.getItem('paid-off') : null, facility: null, contracts: [], transactions: [], payment_parts: [] }
      throw new Error(command)
    } } })
  })
  await page.goto('/#/accounts')
  await expect(page.getByRole('button', { name: /Mark .*paid off/ })).toHaveCount(0)
  const unpaid = page.getByRole('row').filter({ has: page.getByRole('rowheader', { name: 'Unpaid loan', exact: true }) })
  await unpaid.getByRole('link', { name: 'Overview, contracts & transactions' }).click()
  await page.getByRole('button', { name: 'Mark as paid off', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Mark loan as paid off?', exact: true })
  await expect(dialog.getByRole('button', { name: 'Mark as paid off', exact: true })).toBeDisabled()
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('link', { name: '← Accounts', exact: true }).click()
  await page.getByRole('button', { name: 'Cards', exact: true }).click()
  await expect(page.getByRole('button', { name: /Mark .*paid off/ })).toHaveCount(0)
  const card = page.getByRole('listitem').filter({ has: page.getByRole('heading', { name: 'Finished loan', exact: true }) })
  await card.getByRole('link', { name: 'Overview, contracts & transactions' }).click()
  await page.getByRole('button', { name: 'Mark as paid off', exact: true }).click()
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('button', { name: 'Mark as paid off', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-payoff', '1'))
  await dialog.getByRole('button', { name: 'Mark as paid off', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Balance changed')
  await page.evaluate(() => { sessionStorage.removeItem('fail-payoff'); sessionStorage.setItem('hold-payoff', '1') })
  await dialog.getByRole('button', { name: 'Mark as paid off', exact: true }).click()
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled()
  await page.keyboard.press('Escape')
  await expect(dialog).toBeVisible()
  await page.evaluate(() => window.dispatchEvent(new Event('release-payoff')))
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('Paid off 2026-09-29', { exact: true })).toBeVisible()
  await page.getByRole('link', { name: '← Accounts', exact: true }).click()
  await expect(page.getByRole('tab', { name: 'Loans (1)', exact: true })).toBeVisible()
  await page.reload()
  await page.getByText('Paid-off loans (1)', { exact: true }).click()
  await expect(page.getByText('Paid off 2026-09-29', { exact: true })).toBeVisible()
  const closed = page.getByRole('row').filter({ has: page.getByRole('rowheader', { name: 'Finished loan', exact: true }) })
  await expect(closed.getByRole('button', { name: 'Edit account Finished loan' })).toHaveCount(0)
  await closed.getByRole('link', { name: 'Overview, contracts & transactions' }).click()
  await expect(page.getByText('Paid off 2026-09-29', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Mark as paid off', exact: true })).toHaveCount(0)
  await page.getByRole('tab', { name: 'Transactions', exact: true }).click()
  await expect(page.getByRole('table', { name: 'Loan transactions', exact: true })).toBeVisible()
})
