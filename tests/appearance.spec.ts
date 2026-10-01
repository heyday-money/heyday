import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date(2026, 9, 1, 12))
  await page.addInitScript(() => {
    const errors: string[] = []
    Object.assign(window, { appearanceErrors: errors })
    window.addEventListener('error', event => errors.push(event.message))
    Object.defineProperty(window, 'isTauri', { value: true })
    const accounts = [
      { id: 'bank', name: 'Everyday savings · เงินออม', type: 'bank', current_balance: '2485000' },
      { id: 'card', name: 'Travel card with a deliberately long account name', type: 'credit_card', current_balance: '-12345' },
    ].map(a => ({ ...a, opening_balance: a.current_balance, institution: null, institution_id: null,
      loan_type: null, last_four: null, notes: null, credit_limit: '10000000', statement_day: 25,
      payment_due_day: 5, interest_rate_ten_thousandths: null, monthly_installment: null }))
    const incomes = [{ id: 'salary', name: 'Monthly salary', type: 'salary', destination_account_id: 'bank',
      destination_account_name: accounts[0].name, estimated_amount: '5000000', deductions_total: '0',
      recurrence_frequency: 'monthly', recurrence_day_of_month: 25, is_active: true,
      is_auto_create_transaction: false, created_at: '', updated_at: '' }]
    const categories = [{ id: 'food', name: 'Food', icon: 'utensils', is_archived: false }]
    const transactions = [{ id: 'lunch', type: 'expense', date: '2026-10-01', amount: '12500',
      description: 'Lunch', account_id: 'bank', account_name: accounts[0].name,
      destination_account_id: null, destination_account_name: null, payee_id: null, payee_name: null,
      category_id: 'food', category_name: 'Food', category_icon: 'utensils' }]
    const settings = { currency: 'THB', period_start_day: 1 }
    const billing = { statements: [], plans: [], allocations: [], installments: [] }
    const financial = { settings, accounts, incomes, transactions, categories, plans: [], installments: [], subscriptions: [] }
    const plannerCategories = [
      ['income', 'Gross Income', 'Total Gross Income'], ['deductions', 'Income Deductions', 'Total Deductions'],
      ['debt', 'Debt Payments', 'Total Debt Payments'], ['installments', 'Card Installments', 'Total Card Installments'],
      ['cards', 'Credit Cards', 'Total Card Payments'], ['expenses', 'General Expenses', 'Total General Expenses'],
    ].map(([id, name, subtotal]) => ({ id, name, subtotal }))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      switch (command) {
        case 'plugin:app|version': return '0.0.1-alpha.7'
        case 'get_settings': return settings
        case 'list_accounts': return accounts
        case 'list_incomes': return incomes
        case 'list_income_deductions': return []
        case 'list_transactions': return transactions
        case 'list_transaction_options': return { payees: [], categories }
        case 'list_institutions': return { institutions: [], accounts }
        case 'list_card_limit_groups': return { groups: [], cards: [accounts[1]], currency: 'THB' }
        case 'get_financial_data': return financial
        case 'get_expense_report': return { settings, accounts, transactions }
        case 'get_card_overview': return { currency: 'THB', accounts: [accounts[1]], transactions, archived_ids: [], billing }
        case 'get_cashflow_planner': return { categories: plannerCategories, items: [], months: [], amounts: [],
          period_start_day: 1, source_currency: 'THB', opening: { month: '2026-10', amount: '2485000' },
          expense_categories: categories, incomes: [], income_deductions: [], debt_accounts: [], installments: [],
          credit_cards: [], card_transactions: [], ledger_transactions: transactions.map(t => ({ ...t, account_type: 'bank', destination_account_type: null })) }
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
})

test.afterEach(async ({ page }) => {
  expect(await page.evaluate(() => (window as Window & { appearanceErrors?: string[] }).appearanceErrors)).toEqual([])
})

test('visual presets remain independent, select by keyboard, persist and preserve payday draft', async ({ page }) => {
  await page.goto('/#/settings')
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'heyday')
  const previews = page.locator('fieldset [data-theme]')
  const before = await previews.evaluateAll(elements => elements.map(el => getComputedStyle(el).backgroundColor))
  expect(new Set(before).size).toBe(3)
  await page.getByLabel('Period start day', { exact: true }).selectOption('25')
  const light = page.getByRole('radio', { name: 'Light', exact: true })
  await light.check()
  await light.focus()
  await page.keyboard.press('ArrowRight')
  await expect(page.getByRole('radio', { name: 'Dark', exact: true })).toBeChecked()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  expect(await previews.evaluateAll(elements => elements.map(el => getComputedStyle(el).backgroundColor))).toEqual(before)
  await expect(page.getByLabel('Period start day', { exact: true })).toHaveValue('25')
  await expect(page.getByRole('button', { name: 'Save period' })).toBeEnabled()
  await page.getByRole('tab', { name: 'Payees', exact: true }).click()
  await page.getByRole('tab', { name: 'General', exact: true }).click()
  await expect(page.getByLabel('Period start day', { exact: true })).toHaveValue('25')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.getByRole('button', { name: 'Add transaction', exact: true }).click()
  await page.getByLabel('Amount (THB)', { exact: true }).fill('250.50')
  await page.getByLabel('Description (optional)', { exact: true }).fill('Keep this draft')
  // Simulate an appearance update while the modal is open; normal pointer
  // interaction correctly remains trapped in the dialog.
  await page.locator('input[name=appearance][value=heyday]').evaluate(el => (el as HTMLInputElement).click())
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'heyday')
  await expect(page.getByLabel('Amount (THB)', { exact: true })).toHaveValue('250.50')
  await expect(page.getByLabel('Description (optional)', { exact: true })).toHaveValue('Keep this draft')
})

for (const appearance of ['light', 'dark', 'heyday']) {
  test(`${appearance} covers pages, dialogs, bundled typography, contrast and narrow layouts`, async ({ page }) => {
    await page.goto('/#/settings')
    await page.getByRole('radio', { name: appearance[0].toUpperCase() + appearance.slice(1), exact: true }).check()
    await page.reload()
    await expect(page.locator('html')).toHaveAttribute('data-theme', appearance)
    // Selected tabs use the appearance's action colors, rather than fixed zinc.
    const selectedTab = page.getByRole('tab', { name: 'General', exact: true })
    const primaryColors = await page.locator('input[name=appearance]:checked').locator('..').locator('span.rounded-full').evaluate(el => ({ background: getComputedStyle(el).backgroundColor, foreground: getComputedStyle(el).color }))
    await expect(selectedTab).toHaveCSS('background-color', primaryColors.background)
    await expect(selectedTab).toHaveCSS('color', primaryColors.foreground)
    await page.evaluate(() => document.fonts.ready)
    expect(await page.evaluate(() => [...document.fonts].some(font => font.family === 'Inter' && font.status === 'loaded'))).toBe(true)
    // Verify normal-text token pairs against WCAG AA, including primary actions.
    const ratios = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement)
      const luminance = (token: string) => {
        const hex = root.getPropertyValue(token).trim().slice(1)
        const rgb = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
          .map(c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
        return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
      }
      return [['--ink', '--page'], ['--muted', '--page'], ['--muted', '--soft'], ['--primary-ink', '--primary'], ['--accent-ink', '--accent']]
        .map(([a, b]) => (Math.max(luminance(a), luminance(b)) + 0.05) / (Math.min(luminance(a), luminance(b)) + 0.05))
    })
    ratios.forEach(ratio => expect(ratio).toBeGreaterThanOrEqual(4.5))
    await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-settings.png` })
    for (const [path, heading] of [
      ['/', 'A fresh start for your finances.'], ['/accounts', 'A place for every account.'],
      ['/transactions', 'Transactions'], ['/income', 'Know what’s coming in.'],
      ['/net-worth', 'Your overall financial position.'], ['/installments', 'Installment plans'],
      ['/subscriptions', 'Subscription plans'], ['/outlook', 'Cashflow Planner'],
    ]) {
      await page.goto(`/#${path}`)
      await expect(page.getByRole('heading', { name: heading, exact: true }).first()).toBeVisible()
      await expect(page.locator('main [role=alert]')).toHaveCount(0)
      await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-${path.replace('/', '') || 'home'}.png` })
      if (path === '/accounts') {
        await page.getByRole('button', { name: 'Add account', exact: true }).click()
        await expect(page.getByRole('dialog')).toBeVisible()
        await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-account-dialog.png` })
        await page.keyboard.press('Escape')
      }
    }
    for (const tab of ['Expenses', 'Credit Cards', 'Account-Based Outlook']) {
      await page.getByRole('tab', { name: tab, exact: true }).click()
      await expect(page.locator('main [role=alert]')).toHaveCount(0)
      await expect(page.getByRole('table').first()).toBeVisible()
      await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-${tab.toLowerCase().replaceAll(' ', '-')}.png` })
    }
    await page.getByRole('button', { name: 'Add transaction', exact: true }).click()
    await page.getByLabel('Amount (THB)', { exact: true }).fill('123.45')
    await page.getByLabel('Description (optional)', { exact: true }).fill('Draft remains intact')
    await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-transaction-dialog.png` })
    await page.setViewportSize({ width: 760, height: 560 })
    await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-narrow-dialog.png` })
    const dialog = page.getByRole('dialog')
    await expect.poll(async () => {
      const bounds = await dialog.boundingBox()
      return !!bounds && bounds.y >= 0 && bounds.y + bounds.height <= 560
    }).toBe(true)
    expect(await dialog.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.keyboard.press('Escape')
    await page.getByRole('button', { name: 'Discard changes', exact: true }).click()
    await page.goto('/#/settings')
    await page.locator('html').evaluate(el => { el.style.fontSize = '20px' })
    await expect(page.getByRole('radio', { name: 'Heyday', exact: true })).toBeVisible()
    expect(await page.locator('main').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.screenshot({ animations: 'disabled', path: `test-results/appearance-${appearance}-narrow-settings.png` })
  })
}

for (const saved of ['light', 'dark']) {
  test(`existing ${saved} preference is preserved before React loads`, async ({ page }) => {
    await page.addInitScript(value => localStorage.setItem('theme', value), saved)
    await page.route('**/src/main.tsx', route => route.abort())
    await page.goto('/')
    await expect(page.locator('html')).toHaveAttribute('data-theme', saved)
  })
}
