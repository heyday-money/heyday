import { expect, test } from '@playwright/test'
import type { NewAccount, AccountUpdate, Account } from '../src/lib/desktop'
import { decimalToInteger, formatAmount, fractionDigits } from '../src/lib/money'

test('money conversion respects currency precision without floating point loss', () => {
  expect(decimalToInteger('90071992547409.93', 2)).toBe('9007199254740993')
  expect(decimalToInteger('-10.50', 2)).toBe('-1050')
  expect(decimalToInteger('0.001', fractionDigits('KWD'))).toBe('1')
  expect(decimalToInteger('500', fractionDigits('JPY'))).toBe('500')
  expect(() => decimalToInteger('1.01', fractionDigits('JPY'))).toThrow()
  expect(() => decimalToInteger('1e3', 2)).toThrow()
  expect(() => decimalToInteger('9223372036854775808', 0)).toThrow()
  expect(formatAmount('9007199254740993', 'USD').replace(/,/g, '')).toContain('90071992547409.93')
})

test('set currency, create all account types, and preserve them across reload', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string, args: { currency?: string; input?: NewAccount | AccountUpdate }) => {
        const settings = () => ({ currency: localStorage.getItem('test-currency'), period_start_day: 1 })
        const accounts = (): Account[] => JSON.parse(localStorage.getItem('test-accounts') ?? '[]')
        switch (command) {
          case 'plugin:app|version': return '0.1.0'
          case 'get_settings': return settings()
          case 'update_currency':
            if (accounts().length) throw 'Currency cannot change after accounts or income have been created.'
            localStorage.setItem('test-currency', args.currency!); return settings()
          case 'list_transaction_options': return { payees: [], categories: [] }
          case 'list_accounts': return accounts()
          case 'update_account': {
            if (sessionStorage.getItem('fail-account')) throw 'Could not save account. Please try again.'
            const { id, ...details } = args.input as AccountUpdate
            if ('opening_balance' in details || 'current_balance' in details) throw 'Balance must not be sent for editing.'
            const saved = accounts().find(account => account.id === id)!
            const account = { ...saved, ...details, id }
            localStorage.setItem('test-accounts', JSON.stringify(accounts().map(item => item.id === id ? account : item)))
            return account
          }
          case 'create_account': {
            if (sessionStorage.getItem('fail-account')) throw 'Could not save account. Please try again.'
            const account = { ...(args.input as NewAccount), id: crypto.randomUUID() }
            localStorage.setItem('test-accounts', JSON.stringify([...accounts(), account]))
            return account
          }
          default: throw new Error(`Unexpected command: ${command}`)
        }
      },
    } })
  })
  await page.goto('/#/accounts')
  await expect(page.getByRole('button', { name: 'Add account', exact: true })).toBeDisabled()
  await page.getByRole('link', { name: 'Go to Settings' }).click()
  await page.getByLabel('Currency', { exact: true }).selectOption('THB')
  await page.getByRole('button', { name: 'Save currency' }).click()
  await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Currency saved.' })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Accounts', exact: true }).click()
  await page.getByRole('button', { name: 'Expand sidebar' }).click()
  await page.getByRole('button', { name: 'Add account', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Add account', exact: true })
  await expect(dialog).toBeVisible()
  await expect(page.getByLabel('Account name', { exact: true })).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(dialog).not.toBeVisible()
  await expect(page.getByRole('button', { name: 'Add account', exact: true })).toBeFocused()
  await page.getByRole('button', { name: 'Add account', exact: true }).click()
  await page.getByLabel('Account name', { exact: true }).fill('Unsaved account')
  await page.mouse.click(5, 5)
  await expect(dialog).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByText('Discard your unsaved account?')).toBeVisible()
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await expect(page.getByLabel('Account name', { exact: true })).toHaveValue('Unsaved account')
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('button', { name: 'Discard changes' }).click()
  await expect(dialog).not.toBeVisible()
  const tabLabels = { cash: 'Cash', bank: 'Bank', wallet: 'Wallets', credit_card: 'Credit cards', loan: 'Loans', investment: 'Investments' }
  for (const type of ['cash', 'bank', 'wallet', 'credit_card', 'loan', 'investment'] as const) {
    await page.getByRole('tab', { name: `${tabLabels[type]} (0)`, exact: true }).click()
    await page.getByRole('button', { name: 'Add account', exact: true }).click()
    await expect(page.getByLabel('Account type', { exact: true })).toHaveValue(type)
    await page.getByLabel('Account name', { exact: true }).fill(`My ${type}`)
    await page.getByLabel(/^(Opening balance|Amount owed|Current value)/).fill('1234.56')
    await page.getByText('Optional details', { exact: true }).click()
    await page.getByLabel('Last four digits (optional)').fill('0123')
    if (type === 'credit_card') {
      await page.getByLabel(/^Credit limit/).fill('50000')
      await page.getByLabel('Statement day (optional)').selectOption('31')
      await page.getByLabel('Payment due day (optional)').selectOption('15')
      await page.setViewportSize({ width: 760, height: 560 })
      await expect.poll(async () => {
        const footer = await dialog.getByRole('button', { name: 'Save account', exact: true }).boundingBox()
        return footer ? footer.y + footer.height : Infinity
      }).toBeLessThan(560)
      await page.keyboard.press('Tab')
      expect(await dialog.evaluate(element => element.contains(document.activeElement))).toBe(true)
      await page.setViewportSize({ width: 1200, height: 800 })
      await page.screenshot({ path: 'test-results/account-dialog.png' })
    }
    if (type === 'loan') await page.getByLabel('Loan type', { exact: true }).selectOption('mortgage')
    else await expect(page.getByLabel('Loan type', { exact: true })).toHaveCount(0)
    if (type === 'loan') await page.getByLabel('Annual interest rate (%, optional)').fill('1.195')
    if (type === 'cash') {
      await page.evaluate(() => sessionStorage.setItem('fail-account', 'true'))
      await page.getByRole('button', { name: 'Save account', exact: true }).click()
      await expect(page.getByRole('alert')).toContainText('Could not save')
      await expect(page.getByLabel('Account name', { exact: true })).toHaveValue('My cash')
      await page.evaluate(() => sessionStorage.removeItem('fail-account'))
    }
    await page.getByRole('button', { name: 'Save account', exact: true }).click()
    await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Account added.' })).toBeVisible()
    await expect(dialog).not.toBeVisible()
    await expect(page.getByRole('heading', { name: `My ${type}`, exact: true })).toBeVisible()
    await expect(page.getByRole('tab', { name: `${tabLabels[type]} (1)`, exact: true })).toHaveAttribute('aria-selected', 'true')
    const sidebarAccount = page.getByRole('complementary').getByRole('link', { name: new RegExp(`^My ${type}:`) })
    await expect(sidebarAccount).toBeVisible()
    await expect(sidebarAccount.locator('[data-balance-sign]')).toHaveAttribute('data-balance-sign', ['credit_card', 'loan'].includes(type) ? 'negative' : 'positive')
  }
  await page.reload()
  await expect(page.getByRole('complementary').getByRole('region', { name: 'Credit cards', exact: true })).toBeVisible()
  await expect(page.getByRole('tab', { name: 'All (6)', exact: true })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('tabpanel').getByRole('listitem')).toHaveCount(6)
  await expect(page.getByRole('tabpanel').getByText(/•••• 0123/)).toHaveCount(6)
  await expect(page.getByRole('tabpanel').getByRole('region')).toHaveCount(6)
  await expect(page.getByText('Credit limit: 50,000.00 THB')).toBeVisible()
  await expect(page.getByText('Annual interest rate: 1.195%')).toBeVisible()
  await expect(page.getByText('Mortgage · Liability · •••• 0123', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Edit account My loan', exact: true }).click()
  const editDialog = page.getByRole('dialog', { name: 'Edit account', exact: true })
  await expect(page.getByLabel('Account name', { exact: true })).toHaveValue('My loan')
  await expect(page.getByLabel('Opening balance (THB)')).toHaveValue('1234.56')
  await expect(page.getByLabel('Opening balance (THB)')).toHaveAttribute('readonly', '')
  await expect(page.getByLabel('Account type', { exact: true })).toBeDisabled()
  await expect(page.getByLabel('Last four digits (optional)')).toHaveValue('0123')
  await expect(page.getByLabel('Annual interest rate (%, optional)')).toHaveValue('1.195')
  await page.getByLabel('Account name', { exact: true }).fill('Student loan')
  await page.getByLabel('Loan type', { exact: true }).selectOption('student_loan')
  await page.getByLabel('Last four digits (optional)').fill('0099')
  await page.getByLabel('Payment due day (optional)').selectOption('25')
  await page.getByLabel('Notes (optional)').fill('Updated account details')
  await page.keyboard.press('Escape')
  await expect(page.getByText('Discard your unsaved account?')).toBeVisible()
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-account', 'true'))
  await page.getByRole('button', { name: 'Save account', exact: true }).click()
  await expect(editDialog.getByRole('alert')).toContainText('Could not save')
  await expect(page.getByLabel('Account name', { exact: true })).toHaveValue('Student loan')
  await page.evaluate(() => sessionStorage.removeItem('fail-account'))
  await page.getByRole('button', { name: 'Save account', exact: true }).click()
  await expect(editDialog).not.toBeVisible()
  await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Account updated.' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Student loan', exact: true })).toBeVisible()
  await expect(page.getByRole('complementary').getByRole('link', { name: /^Student loan:/ })).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: 'Edit account Student loan', exact: true }).click()
  await expect(page.getByLabel('Opening balance (THB)')).toHaveValue('1234.56')
  await expect(page.getByLabel('Last four digits (optional)')).toHaveValue('0099')
  await expect(page.getByLabel('Loan type', { exact: true })).toHaveValue('student_loan')
  await expect(page.getByLabel('Notes (optional)')).toHaveValue('Updated account details')
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('tab', { name: 'All (6)', exact: true }).focus()
  await page.keyboard.press('ArrowRight')
  await expect(page.getByRole('tab', { name: 'Cash (1)', exact: true })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('tabpanel').getByRole('listitem')).toHaveCount(1)
  await page.setViewportSize({ width: 650, height: 700 })
  await page.keyboard.press('End')
  await expect(page.getByRole('tab', { name: 'Investments (1)', exact: true })).toBeFocused()
  await expect(page.getByRole('tabpanel').getByRole('heading', { name: 'My investment', exact: true })).toBeVisible()
  expect(await page.locator('main').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.setViewportSize({ width: 1200, height: 800 })
  await page.getByRole('tab', { name: 'Bank (1)', exact: true }).click()
  await page.getByRole('button', { name: 'Add account', exact: true }).click()
  await expect(page.getByLabel('Account type', { exact: true })).toHaveValue('bank')
  await page.getByLabel('Account type', { exact: true }).selectOption('cash')
  await page.getByLabel('Account name', { exact: true }).fill('Extra cash')
  await page.getByRole('button', { name: 'Save account', exact: true }).click()
  await expect(page.getByRole('tab', { name: 'Cash (2)', exact: true })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: 'Extra cash', exact: true })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Settings', exact: true }).click()
  await page.getByLabel('Currency', { exact: true }).selectOption('USD')
  await page.getByRole('button', { name: 'Save currency' }).click()
  await expect(page.getByRole('alert')).toContainText('Currency cannot change')
})
