import { expect, test } from '@playwright/test'
import type { NewAccount, AccountUpdate, Account } from '../src/lib/desktop'
import { interestRateText } from '../src/lib/installments'
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

test('account interest precision preserves four decimals and rejects excess precision', () => {
  for (const rate of ['0', '0.0001', '1.1957', '4.3219', '100']) {
    expect(interestRateText(decimalToInteger(rate, 4), 4)).toBe(rate)
  }
  expect(interestRateText('11950', 4)).toBe('1.195')
  expect(() => decimalToInteger('1.19571', 4)).toThrow('up to 4 decimal places')
  expect(interestRateText('1195')).toBe('1.195')
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
          case 'list_institutions': return { institutions: [], accounts: [] }
          case 'get_settings': return settings()
          case 'update_currency':
            if (accounts().length) throw 'Currency cannot change after accounts or income have been created.'
            localStorage.setItem('test-currency', args.currency!); return settings()
          case 'list_transaction_options': return { payees: [], categories: [] }
          case 'list_card_limit_groups': return { groups: [], cards: [] }
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
  await expect(page.getByRole('button', { name: 'Table', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: 'Cards', exact: true }).click()
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
    if (type === 'loan') {
      await page.getByLabel('Annual interest rate (%, optional)').fill('1.1957')
      await page.getByLabel('Monthly installment (THB, optional)').fill('123.45')
      await page.getByLabel('Initial Loan Amount (THB, optional)').fill('100000')
    }
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
  await expect(page.getByText('Initial Loan Amount: 100,000.00 THB')).toBeVisible()
  await expect(page.getByText('Credit limit: 50,000.00 THB')).toBeVisible()
  await expect(page.getByText('Annual interest rate: 1.1957%')).toBeVisible()
  await expect(page.getByText('Monthly installment: 123.45 THB')).toBeVisible()
  await expect(page.getByText('Mortgage · Liability · •••• 0123', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Table', exact: true }).click()
  await page.getByRole('button', { name: 'Edit account My loan', exact: true }).click()
  const editDialog = page.getByRole('dialog', { name: 'Edit account', exact: true })
  await expect(page.getByLabel('Account name', { exact: true })).toHaveValue('My loan')
  await expect(page.getByLabel('Opening balance (THB)')).toHaveValue('1234.56')
  await expect(page.getByLabel('Opening balance (THB)')).toHaveAttribute('readonly', '')
  await expect(page.getByLabel('Account type', { exact: true })).toBeDisabled()
  await expect(page.getByLabel('Last four digits (optional)')).toHaveValue('0123')
  await expect(page.getByLabel('Annual interest rate (%, optional)')).toHaveValue('1.1957')
  await expect(page.getByLabel('Monthly installment (THB, optional)')).toHaveValue('123.45')
  await expect(page.getByLabel('Initial Loan Amount (THB, optional)')).toHaveValue('100000.00')
  await page.getByLabel('Initial Loan Amount (THB, optional)').fill('90071992547409.93')
  await page.getByLabel('Monthly installment (THB, optional)').fill('0')
  await page.getByLabel('Annual interest rate (%, optional)').fill('4.3219')
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
  await expect(page.getByRole('rowheader', { name: 'Student loan', exact: false })).toBeVisible()
  await page.getByRole('button', { name: 'Cards', exact: true }).click()
  await expect(page.getByRole('complementary').getByRole('link', { name: /^Student loan:/ })).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: 'Edit account Student loan', exact: true }).click()
  await expect(page.getByLabel('Annual interest rate (%, optional)')).toHaveValue('4.3219')
  await expect(page.getByLabel('Initial Loan Amount (THB, optional)')).toHaveValue('90071992547409.93')
  await expect(page.getByLabel('Monthly installment (THB, optional)')).toHaveValue('0.00')
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

test('type summaries show exact current assets and liabilities and refresh after account changes', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const rows = [
      ['bank', '9007199254740993'], ['bank', '-100'],
      ['credit_card', '50000'], ['credit_card', '-2500'],
      ['loan', '10000'], ['wallet', '300'], ['investment', '-200'],
    ].map(([type, current_balance], index) => ({ id: String(index), name: `Account ${index}`, type, current_balance, opening_balance: '0', loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null, monthly_installment: null }))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string) => {
        if (command === 'plugin:app|version') return '0.0.1-alpha.3'
        if (command === 'list_institutions') return { institutions: [], accounts: [] }
        if (command === 'get_settings') return { currency: 'THB', period_start_day: 1 }
        if (command === 'list_transaction_options') return { payees: [], categories: [] }
        if (command === 'list_accounts') return rows.map(row => row.type === 'wallet' && sessionStorage.getItem('updated-wallet') ? { ...row, current_balance: '400' } : row)
        throw new Error(`Unexpected command: ${command}`)
      },
    } })
  })
  await page.goto('/#/accounts')
  const table = page.getByRole('table', { name: 'Accounts', exact: true })
  await expect(page.getByRole('button', { name: 'Table', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(table.locator('tbody tr')).toHaveCount(7)
  await expect(table.getByRole('row').filter({ hasText: 'Account 0' })).toContainText('90,071,992,547,409.93 THB')
  await expect(table.getByRole('row').filter({ hasText: 'Account 3' })).toContainText('Credit / overpayment')
  await page.getByRole('button', { name: 'Cards', exact: true }).click()
  await expect(table).toHaveCount(0)
  await expect(page.getByRole('tabpanel').getByRole('listitem')).toHaveCount(7)
  await page.reload()
  await expect(page.getByRole('button', { name: 'Cards', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: 'Table', exact: true }).click()
  await page.reload()
  await expect(table.locator('tbody tr')).toHaveCount(7)
  const bank = page.getByRole('group', { name: 'Bank summary', exact: true })
  await expect(bank).toContainText('2 accounts')
  await expect(bank.locator('dd')).toHaveText(['90,071,992,547,408.93 THB', '90,071,992,547,409.93 THB', '1.00 THB'])
  await expect(page.getByRole('group', { name: 'Credit cards summary' }).locator('dd')).toHaveText(['−475.00 THB', '25.00 THB', '500.00 THB'])
  await expect(page.getByRole('group', { name: 'Loans summary' }).locator('dd')).toHaveText(['−100.00 THB', '0.00 THB', '100.00 THB'])
  await expect(page.getByRole('group', { name: 'Investments summary' }).locator('dd')).toHaveText(['−2.00 THB', '0.00 THB', '2.00 THB'])
  const cash = page.getByRole('group', { name: 'Cash summary', exact: true })
  await expect(cash).toContainText('0 accounts')
  await expect(cash.locator('dd')).toHaveText(['0.00 THB', '0.00 THB', '0.00 THB'])
  await page.getByRole('tab', { name: 'Bank (2)', exact: true }).click()
  await expect(table.locator('tbody tr')).toHaveCount(2)
  await expect(page.getByRole('group', { name: 'Credit cards summary' })).toBeVisible()
  await page.evaluate(() => {
    sessionStorage.setItem('updated-wallet', 'true')
    window.dispatchEvent(new Event('accounts-changed'))
  })
  await expect(page.getByRole('group', { name: 'Wallets summary' }).locator('dd')).toHaveText(['4.00 THB', '4.00 THB', '0.00 THB'])
  await page.setViewportSize({ width: 650, height: 700 })
  expect(await bank.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  expect(await page.locator('main').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  const scroll = page.getByRole('region', { name: 'Scrollable accounts', exact: true })
  expect(await scroll.evaluate(element => element.scrollWidth > element.clientWidth)).toBe(true)
  await page.getByRole('tab', { name: 'Cash (0)', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'No cash accounts yet' })).toBeVisible()
  await expect(table).toHaveCount(0)
})

test('long sidebar account lists scroll without moving navigation or Settings', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('sidebar-collapsed', 'false')
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string) => {
        if (command === 'plugin:app|version') return '0.0.1-alpha.5'
        if (command === 'list_institutions') return { institutions: [], accounts: [] }
        if (command === 'get_settings') return { currency: 'THB', period_start_day: 1 }
        if (command === 'list_transaction_options') return { payees: [], categories: [] }
        if (command === 'list_accounts') return Array.from({ length: 40 }, (_, index) => ({
          id: String(index), name: `Long account name ${index}`, type: 'cash',
          current_balance: '10000', opening_balance: '10000', is_archived: false,
        }))
        throw new Error(`Unexpected command: ${command}`)
      },
    } })
  })
  await page.goto('/#/settings')
  const region = page.getByRole('region', { name: 'Saved accounts' })
  const nav = page.getByRole('navigation', { name: 'Main navigation' })
  const home = nav.getByRole('link', { name: 'Home', exact: true })
  const settings = nav.getByRole('link', { name: 'Settings', exact: true })
  for (const viewport of [{ width: 1200, height: 800 }, { width: 760, height: 560 }]) {
    await page.setViewportSize(viewport)
    await expect(region.getByRole('link')).toHaveCount(40)
    await region.evaluate(element => { element.scrollTop = 0 })
    const homeBefore = await home.boundingBox()
    const settingsBefore = await settings.boundingBox()
    expect(await region.evaluate(element => element.clientHeight > 0 && element.scrollHeight > element.clientHeight)).toBe(true)
    expect(await region.evaluate(element => getComputedStyle(element).scrollbarWidth)).toBe('none')
    await region.focus()
    await page.keyboard.press('End')
    await expect.poll(() => region.evaluate(element => element.scrollTop)).toBeGreaterThan(0)
    expect(await home.boundingBox()).toEqual(homeBefore)
    expect(await settings.boundingBox()).toEqual(settingsBefore)
    await expect(settings).toBeInViewport()
    await expect(page.getByRole('button', { name: 'Collapse sidebar' })).toBeInViewport()
    expect(await region.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  }
})

test('Loans shows exact annual rates and monthly installments with unset values sorted last', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const accounts = [null, 100000, 0, 11957, 75000, 11950].map((rate, index) => ({
      id: String(index), name: `Loan ${index}`, type: 'loan', loan_type: 'personal_loan',
      current_balance: '10000', opening_balance: '10000', interest_rate_ten_thousandths: rate,
      institution: null, last_four: null, notes: null, credit_limit: null,
      statement_day: null, payment_due_day: null, monthly_installment: [null, '9007199254740993', '0', '1000', '9007199254740992', '900'][index],
    }))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'plugin:app|version') return '0.0.1-alpha.5'
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 1 }
      if (command === 'list_accounts') return accounts
      if (command === 'list_institutions') return { institutions: [], accounts: [] }
      if (command === 'list_card_limit_groups') return { groups: [], cards: [] }
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      throw new Error(command)
    } } })
  })
  await page.goto('/#/accounts')
  const table = page.getByRole('table', { name: 'Accounts', exact: true })
  await expect(table.getByRole('columnheader', { name: /annual interest rate/i })).toHaveCount(0)
  await expect(table.getByRole('columnheader', { name: /monthly installment/i })).toHaveCount(0)
  await page.getByRole('tab', { name: 'Loans (6)', exact: true }).click()
  const header = table.getByRole('columnheader', { name: /annual interest rate/i })
  await expect(header).toHaveAttribute('aria-sort', 'none')
  await expect(table.locator('tbody tr').nth(0)).toContainText('Not set')
  await expect(table.locator('tbody tr').nth(2)).toContainText('0%')
  await expect(table.locator('tbody tr').nth(3)).toContainText('1.1957%')
  await table.getByRole('button', { name: 'Sort annual interest rate lowest first' }).click()
  await expect(header).toHaveAttribute('aria-sort', 'ascending')
  await expect(table.getByRole('rowheader')).toContainText(['Loan 2', 'Loan 5', 'Loan 3', 'Loan 4', 'Loan 1', 'Loan 0'])
  await table.getByRole('button', { name: 'Sort annual interest rate highest first' }).press('Enter')
  await expect(header).toHaveAttribute('aria-sort', 'descending')
  await expect(table.getByRole('rowheader')).toContainText(['Loan 1', 'Loan 4', 'Loan 3', 'Loan 5', 'Loan 2', 'Loan 0'])
  const monthlyHeader = table.getByRole('columnheader', { name: /monthly installment/i })
  await expect(monthlyHeader).toHaveAttribute('aria-sort', 'none')
  await expect(table.getByRole('row').filter({ has: page.getByRole('rowheader', { name: 'Loan 1', exact: true }) })).toContainText('90,071,992,547,409.93 THB')
  await expect(table.getByRole('row').filter({ has: page.getByRole('rowheader', { name: 'Loan 2', exact: true }) })).toContainText('0.00 THB')
  await table.getByRole('button', { name: 'Sort monthly installment lowest first' }).click()
  await expect(monthlyHeader).toHaveAttribute('aria-sort', 'ascending')
  await expect(header).toHaveAttribute('aria-sort', 'none')
  await expect(table.getByRole('rowheader')).toContainText(['Loan 2', 'Loan 5', 'Loan 3', 'Loan 4', 'Loan 1', 'Loan 0'])
  await table.getByRole('button', { name: 'Sort monthly installment highest first' }).press('Enter')
  await expect(monthlyHeader).toHaveAttribute('aria-sort', 'descending')
  await expect(table.getByRole('rowheader')).toContainText(['Loan 1', 'Loan 4', 'Loan 3', 'Loan 5', 'Loan 2', 'Loan 0'])
  await page.getByRole('button', { name: 'Cards', exact: true }).click()
  await expect(page.getByText('Annual interest rate: 1.1957%', { exact: true })).toBeVisible()
})

for (const [accountType, tabName] of [['bank', 'Bank'], ['credit_card', 'Credit cards']] as const) {
test(`${tabName} balances sort exactly in both directions, including negative and opening balances`, async ({ page }) => {
  await page.addInitScript(accountType => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const accounts = ['9007199254740993', '-100', '0', '9007199254740992', null].map((balance, index) => ({
      id: String(index), name: `Account ${index}`, type: accountType, loan_type: null,
      current_balance: balance, opening_balance: '1000', interest_rate_ten_thousandths: null,
      institution: null, last_four: null, notes: null, credit_limit: null,
      statement_day: null, payment_due_day: null, monthly_installment: null,
    }))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'plugin:app|version') return '0.0.1-alpha.5'
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 1 }
      if (command === 'list_accounts') return accounts
      if (command === 'list_institutions') return { institutions: [], accounts: [] }
      if (command === 'list_card_limit_groups') return { groups: [], cards: [] }
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      throw new Error(command)
    } } })
  }, accountType)
  await page.goto('/#/accounts')
  const table = page.getByRole('table', { name: 'Accounts', exact: true })
  await expect(table.getByRole('button', { name: /Sort balance/ })).toHaveCount(0)
  await page.getByRole('tab', { name: `${tabName} (5)`, exact: true }).click()
  const header = table.getByRole('columnheader', { name: /Sort balance/ })
  await expect(header).toHaveAttribute('aria-sort', 'none')
  await table.getByRole('button', { name: 'Sort balance lowest first' }).click()
  await expect(header).toHaveAttribute('aria-sort', 'ascending')
  await expect(table.getByRole('rowheader')).toContainText(['Account 1', 'Account 2', 'Account 4', 'Account 3', 'Account 0'])
  await table.getByRole('button', { name: 'Sort balance highest first' }).press('Enter')
  await expect(header).toHaveAttribute('aria-sort', 'descending')
  await expect(table.getByRole('rowheader')).toContainText(['Account 0', 'Account 3', 'Account 4', 'Account 2', 'Account 1'])
})
}

test('archive and restore preserve account details with explicit confirmation', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const account = { id: 'bank', name: 'Savings', type: 'bank', opening_balance: '12345', current_balance: '12345', is_archived: false, notes: 'Keep this history', last_four: null, loan_type: null, institution: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null, monthly_installment: null }
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { includeArchived?: boolean; archived?: boolean }) => {
      switch (command) {
        case 'get_settings': return { currency: 'THB', period_start_day: 1 }
        case 'plugin:app|version': return 'test'
        case 'list_accounts': return args.includeArchived || !account.is_archived ? [account] : []
        case 'list_institutions': return { institutions: [], accounts: [] }
        case 'list_card_limit_groups': return { groups: [], cards: [] }
        case 'set_account_archived': account.is_archived = args.archived!; return null
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
  await page.goto('/#/accounts')
  await page.getByRole('button', { name: 'Archive account Savings', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('excludes this balance from Net Worth')
  await expect(dialog).toContainText('123.45')
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Archive account Savings', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Archive account Savings', exact: true }).click()
  await dialog.getByRole('button', { name: 'Archive account', exact: true }).click()
  await page.getByText('Archived accounts (1)', { exact: true }).click()
  await expect(page.getByRole('button', { name: 'Edit account Savings' })).toHaveCount(0)
  await page.getByText('Account details', { exact: true }).click()
  await expect(page.getByText('Keep this history', { exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Transaction history', exact: true })).toHaveAttribute('href', /account=bank/)
  await page.getByRole('button', { name: 'Restore account Savings', exact: true }).click()
  await expect(dialog).toContainText('may generate forecasts again')
  await dialog.getByRole('button', { name: 'Restore account', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Edit account Savings' })).toBeVisible()
  await expect(page.getByText('Archived accounts (1)', { exact: true })).toHaveCount(0)
})
