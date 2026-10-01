import { expect, test } from '@playwright/test'
import type { PlannerChange, PlannerData } from '../src/lib/cashflow'

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date(2026, 11, 27, 12))
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const categories: PlannerData['categories'] = [
      { id: 'income', name: 'Gross Income', subtotal: 'Total Gross Income' }, { id: 'deductions', name: 'Income Deductions', subtotal: 'Total Deductions' },
      { id: 'debt', name: 'Debt Payments', subtotal: 'Total Debt Payments' }, { id: 'installments', name: 'Card Installments', subtotal: 'Total Card Installments' },
      { id: 'cards', name: 'Credit Cards', subtotal: 'Total Card Payments' }, { id: 'expenses', name: 'General Expenses', subtotal: 'Total General Expenses' },
    ]
    const initial: PlannerData = { incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], source_currency: 'THB', categories, period_start_day: 25, opening: null, months: [], amounts: [], expense_categories: [{ id: 'water', name: 'Water', icon: 'plug-zap', is_archived: false }],
      items: categories.map((c, i) => ({ id: c.id, category_id: c.id, name: ['Salary', 'Withholding tax', 'Mortgage', 'Card / installment 1', 'Card 1', 'Water'][i], description: 'Original description', card_name: '', transaction_category_id: c.id === 'expenses' ? 'water' : null, schedule_amount: null, schedule_start: null, schedule_end: null })) }
    const read = (): PlannerData => JSON.parse(localStorage.getItem('planner-test') ?? JSON.stringify(initial))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { input: PlannerChange }) => {
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 25 }
      if (command === 'list_accounts') return []
      if (command === 'get_cashflow_planner') return read()
      if (command === 'save_cashflow_planner') {
        if (sessionStorage.getItem('fail-planner')) throw 'Could not save planner.'
        const input = args.input, state = read()
        switch (input.kind) {
          case 'item': { const id = input.id ?? crypto.randomUUID(); state.items = [...state.items.filter(i => i.id !== id), { ...input, id }]; break }
          case 'delete': state.items = state.items.filter(i => i.id !== input.id); state.amounts = state.amounts.filter(a => a.item_id !== input.id); break
          case 'amount': state.amounts = state.amounts.filter(a => a.item_id !== input.item_id || a.month !== input.month); if (input.amount !== null) state.amounts.push({ ...input, amount: input.amount }); break
          case 'opening': state.opening = input.amount === null ? null : { month: input.month, amount: input.amount }; break
          case 'status': state.months = [...state.months.filter(m => m.month !== input.month), input]; break
        }
        localStorage.setItem('planner-test', JSON.stringify(state)); return state
      }
      throw new Error(command)
    } } })
  })
  await page.goto('/#/outlook')
})

test('cycle amounts, opening cash, comparison columns and item edits survive reload and window changes', async ({ page }) => {
  await expect(page.getByRole('columnheader')).toHaveCount(17)
  await expect(page.getByRole('rowheader').filter({ has: page.getByRole('button', { name: 'Edit Water', exact: true }) }).locator('svg.lucide-plug-zap')).toBeVisible()
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2026-12')
  await expect(page.getByRole('columnheader').nth(1)).toContainText('25 Dec 2026 – 24 Jan 2027')
  await expect(page.getByRole('columnheader', { name: /^Jun 2027/ })).toBeVisible()
  await expect(page.getByText('Enter opening cash and its initial cycle')).toHaveCount(0)
  await expect(page.getByText('Forecast is your full-cycle estimate.')).toHaveCount(0)
  await page.getByRole('button', { name: 'Edit Salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('50000')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByRole('columnheader', { name: /Dec 2026/ })).toHaveAttribute('colspan', '2')
  await page.getByRole('button', { name: 'Set opening cash' }).click()
  await page.getByLabel('Available cash, bank, and wallet balance (THB)').fill('0')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  const closing = page.getByRole('row').last()
  await expect(closing).toContainText('Cumulative Closing Cash')
  await expect(closing.getByRole('cell').first()).toHaveText('50,000.00')
  await page.getByRole('button', { name: 'Edit Salary', exact: true }).click()
  await page.getByLabel('Item name', { exact: true }).fill('Main salary')
  await page.getByLabel('Description (optional)').fill('Before payroll deductions')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.getByLabel('First visible cycle').fill('2027-07')
  await expect(closing.getByRole('cell').first()).toHaveText('50,000.00')
  await page.getByLabel('First visible cycle').fill('2026-12')
  await page.reload()
  await expect(page.getByRole('button', { name: 'Edit Main salary 2026-12 amount', exact: true })).toContainText('50,000.00')
  await page.getByRole('button', { name: 'Help: Main salary', exact: true }).scrollIntoViewIfNeeded()
  await page.getByRole('button', { name: 'Help: Main salary', exact: true }).hover()
  await expect(page.getByRole('tooltip')).toContainText('Before payroll deductions')
  const content = page.locator('[data-slot="tooltip-content"]').first()
  await expect(content).toHaveCSS('background-color', 'rgb(255, 255, 255)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await page.getByRole('button', { name: 'Help: Main salary', exact: true }).hover()
  await expect(content).toHaveCSS('background-color', 'rgb(34, 34, 37)')
  await expect(content).toHaveCSS('color', 'rgb(244, 244, 245)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await page.keyboard.press('Escape')
  await expect(page.getByRole('tooltip')).toHaveCount(0)
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  for (const control of [
    page.getByRole('button', { name: 'Add item to Gross Income', exact: true }),
    page.getByRole('link', { name: 'Manage salary deductions in Income', exact: true }),
  ]) {
    await control.hover()
    await expect(control).toHaveCSS('color', 'rgb(244, 244, 245)')
  }
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await expect(page.getByLabel('Status 2026-12')).toHaveCount(0)
})

test('installments end on time, zero overrides persist, failed saves preserve drafts and deletion is explicit', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit Card / installment 1', exact: true }).click()
  await page.getByLabel('Item / purchase name').fill('Laptop')
  await page.getByLabel('Card name', { exact: true }).fill('Visa')
  await page.getByLabel('Schedule', { exact: true }).selectOption('repeat')
  await page.getByLabel('Monthly installment (THB)').fill('3000')
  await page.getByLabel('Number of installments (including first)').fill('3')
  await expect(page.getByText('Final payment cycle: Feb 2027')).toBeVisible()
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Edit Laptop 2027-02 amount', exact: true })).toContainText('3,000.00')
  await expect(page.getByRole('button', { name: 'Edit Laptop 2027-03 amount', exact: true })).toContainText('Enter amount')
  await page.getByRole('button', { name: 'Edit Laptop 2027-01 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('0.00')
  await page.evaluate(() => sessionStorage.setItem('fail-planner', 'yes'))
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Could not save planner')
  await expect(page.getByLabel('Cycle amount (THB)')).toHaveValue('0.00')
  await page.evaluate(() => sessionStorage.removeItem('fail-planner'))
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Edit Laptop 2027-01 amount', exact: true })).toContainText('0.00')
  await expect(page.getByRole('button', { name: 'Edit Laptop 2026-12 amount', exact: true })).toContainText('3,000.00')
  await page.getByRole('button', { name: 'Delete Laptop', exact: true }).click()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Edit Laptop', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Delete Laptop', exact: true }).click()
  await page.getByRole('button', { name: 'Delete item', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Edit Laptop', exact: true })).toHaveCount(0)
})

test('table uses full content height and keeps its item column visible during horizontal scrolling', async ({ page }) => {
  const region = page.getByRole('region', { name: 'Scrollable seven-cycle cashflow planner in thb' })
  const corner = page.getByRole('columnheader').first()
  const before = await corner.boundingBox()
  await region.evaluate(el => { el.scrollLeft = 450; el.scrollTop = 400 })
  expect(await region.evaluate(el => el.scrollTop)).toBe(0)
  const after = await corner.boundingBox()
  expect(Math.abs(after!.x - before!.x)).toBeLessThan(2)
  expect(Math.abs(after!.y - before!.y)).toBeLessThan(2)
  await page.screenshot({ path: 'test-results/cashflow-planner.png', fullPage: true })
})

test('Income rows are queried, allow cycle overrides, and still allow extra income', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit Salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('100')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.evaluate(() => {
    const data = JSON.parse(localStorage.getItem('planner-test')!)
    data.incomes = [{ id: 'source', name: 'Employer salary', estimated_amount: '5000000', recurrence_day_of_month: 25, is_active: true, destination_account_name: 'Bank', account_archived: false }]
    localStorage.setItem('planner-test', JSON.stringify(data))
  })
  await page.reload()
  await expect(page.getByRole('link', { name: 'Employer salary', exact: true })).toHaveAttribute('href', /income/)
  const salaryLink = page.getByRole('link', { name: 'Employer salary', exact: true })
  await expect(salaryLink).toHaveCSS('border-bottom-color', 'rgba(0, 0, 0, 0)')
  await salaryLink.hover()
  await expect(salaryLink).toHaveCSS('border-bottom-style', 'dashed')
  await expect(salaryLink).toHaveCSS('border-bottom-width', '1px')
  await expect(salaryLink).toHaveCSS('border-bottom-color', 'rgb(36, 33, 43)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await expect(salaryLink).toHaveCSS('border-bottom-color', 'rgb(255, 255, 255)')
  await page.mouse.move(0, 0)
  await salaryLink.focus()
  await expect(salaryLink).toHaveCSS('border-bottom-color', 'rgb(255, 255, 255)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await expect(page.getByRole('button', { name: 'Edit Employer salary 2026-12 amount', exact: true })).toContainText('50,000.00')
  await expect(page.getByRole('button', { name: 'Edit Employer salary 2026-12 amount', exact: true })).toContainText('Expected income')
  await page.getByRole('button', { name: 'Edit Employer salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('45000')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.getByRole('button', { name: 'Add item to Gross Income', exact: true }).click()
  await page.getByLabel('Item name', { exact: true }).fill('Extra freelance income')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Edit Extra freelance income', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Edit Employer salary 2026-12 amount', exact: true })).toContainText('45,000.00')
  await expect(page.getByRole('button', { name: 'Edit Employer salary 2027-01 amount', exact: true })).toContainText('50,000.00')
})


test('loan accounts and saved card installments populate linked rows with persistent cycle overrides', async ({ page }) => {
  // Save once to initialize the mock database, then simulate existing source pages.
  await page.getByRole('button', { name: 'Edit Salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('100')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.evaluate(() => {
    const data = JSON.parse(localStorage.getItem('planner-test')!)
    data.debt_accounts = [{ id: 'loan', name: 'Home mortgage', loan_type: 'mortgage', current_balance: '250000000', monthly_installment: '1100000', notes: null, is_archived: false }]
    data.installments = [{ id: 'laptop', name: 'Work laptop', debt_account_id: 'card', debt_account_name: 'Visa', debt_account_type: 'credit_card', account_name: 'Bank', monthly_amount: '300000', installment_count: 2, first_due_date: '2026-12-25', accounts_available: true }]
    localStorage.setItem('planner-test', JSON.stringify(data))
  })
  await page.reload()
  await expect(page.getByRole('link', { name: 'Home mortgage', exact: true })).toHaveAttribute('href', /accounts/)
  await expect(page.getByRole('link', { name: 'Visa · Work laptop', exact: true })).toHaveAttribute('href', /installments/)
  await expect(page.getByRole('button', { name: 'Edit Home mortgage 2026-12 amount', exact: true })).toContainText('11,000.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2026-12 amount', exact: true })).toContainText('3,000.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2027-01 amount', exact: true })).toContainText('3,000.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2027-02 amount', exact: true })).toContainText('0.00')
  await page.getByRole('button', { name: 'Edit Home mortgage 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('12000')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.getByRole('button', { name: 'Edit Work laptop 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('0')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Edit Home mortgage 2026-12 amount', exact: true })).toContainText('12,000.00')
  await expect(page.getByRole('button', { name: 'Edit Home mortgage 2027-01 amount', exact: true })).toContainText('11,000.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2026-12 amount', exact: true })).toContainText('0.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2027-01 amount', exact: true })).toContainText('3,000.00')
  await expect(page.getByRole('button', { name: 'Add item to Debt Payments', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Add item to Card Installments', exact: true })).toBeVisible()
})

test('recorded card payments refresh by account and cycle without adding installment forecasts twice', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit Salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('100')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.evaluate(() => {
    const data = JSON.parse(localStorage.getItem('planner-test')!)
    data.credit_cards = [{ id: 'card', name: 'Visa', is_archived: false }]
    data.categories.find((c: { id: string }) => c.id === 'cards').name = 'Credit cards'
    data.installments = [{ id: 'laptop', name: 'Work laptop', debt_account_id: 'card', debt_account_name: 'Visa', debt_account_type: 'credit_card', account_name: 'Bank', monthly_amount: '300000', installment_count: 2, first_due_date: '2026-12-25', accounts_available: true }]
    data.card_transactions = [{ id: 'bill', type: 'repayment', account_id: 'bank', account_type: 'bank', destination_account_id: 'card', destination_account_type: 'credit_card', amount: '200000', date: '2026-12-25' }]
    localStorage.setItem('planner-test', JSON.stringify(data))
    window.dispatchEvent(new Event('transactions-changed'))
  })
  await expect(page.getByRole('link', { name: 'Visa', exact: true })).toHaveAttribute('href', /accounts\/card\/billing/)
  await expect(page.getByRole('button', { name: 'Visa 2026-12: Recorded payments', exact: true })).toContainText('2,000.00')
  await expect(page.getByRole('button', { name: 'Work laptop 2026-12: Using recorded card total', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2027-01 amount', exact: true })).toContainText('3,000.00')
  await page.evaluate(() => {
    const data = JSON.parse(localStorage.getItem('planner-test')!)
    data.card_transactions = []
    localStorage.setItem('planner-test', JSON.stringify(data))
    window.dispatchEvent(new Event('accounts-changed'))
  })
  await expect(page.getByRole('button', { name: 'Visa 2026-12: Recorded payments', exact: true })).toContainText('0.00')
  await expect(page.getByRole('button', { name: 'Edit Work laptop 2026-12 amount', exact: true })).toContainText('3,000.00')
})

test('current cycle follows the computer date after focus while explicit selections remain pinned', async ({ page }) => {
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2026-12')
  await page.clock.setFixedTime(new Date(2027, 0, 25, 12))
  await page.evaluate(() => window.dispatchEvent(new Event('focus')))
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2027-01')
  await page.getByLabel('First visible cycle').fill('2026-11')
  await page.clock.setFixedTime(new Date(2027, 1, 25, 12))
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')))
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2026-11')
  await page.getByRole('button', { name: 'Current cycle', exact: true }).click()
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2027-02')
})

test('recorded General Expenses use saved icons and refresh category changes', async ({ page }) => {
  await page.evaluate(() => {
    localStorage.setItem('planner-test', JSON.stringify({
      incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [], card_transactions: [],
      source_currency: 'THB', period_start_day: 25, opening: null, months: [], amounts: [], items: [],
      categories: [{ id: 'expenses', name: 'General Expenses', subtotal: 'Total General Expenses' }],
      expense_categories: [{ id: 'water', name: 'Water', icon: 'plug-zap', is_archived: true }],
      ledger_transactions: ['water', null].map((category, index) => ({
        id: String(index), type: 'expense', account_id: 'bank', account_name: 'Bank', account_type: 'bank',
        destination_account_id: null, destination_account_type: null, amount: '100', date: '2026-12-26',
        category_id: category, category_name: category ? 'Water' : null,
      })),
    }))
    window.dispatchEvent(new Event('transaction-options-changed'))
  })
  const water = page.getByRole('rowheader').filter({ has: page.getByRole('link', { name: 'Water', exact: true }) })
  const uncategorized = page.getByRole('rowheader').filter({ has: page.getByRole('link', { name: 'Uncategorized', exact: true }) })
  await expect(water.locator('svg.lucide-plug-zap')).toBeVisible()
  await expect(uncategorized.locator('svg.lucide-tag')).toBeVisible()
  await page.evaluate(() => {
    const state = JSON.parse(localStorage.getItem('planner-test')!)
    state.expense_categories[0].icon = 'coffee'
    localStorage.setItem('planner-test', JSON.stringify(state))
    window.dispatchEvent(new Event('transaction-options-changed'))
  })
  await expect(water.locator('svg.lucide-coffee')).toBeVisible()
})

test('Income and Expenses collapse to totals while expense subsections retain their subtotals', async ({ page }) => {
  const table = page.getByRole('table', { name: 'Seven-cycle cashflow planner in THB' })
  const closing = table.getByRole('row').last()
  const before = await closing.innerText()
  await table.getByRole('button', { name: 'Collapse Debt Payments', exact: true }).click()
  await expect(table.getByRole('button', { name: 'Edit Mortgage', exact: true })).toHaveCount(0)
  await expect(table.getByRole('rowheader', { name: /^Total Debt Payments/ })).toBeVisible()
  await expect(table.getByRole('button', { name: 'Edit Water', exact: true })).toBeVisible()
  await table.getByRole('button', { name: 'Collapse Expenses', exact: true }).click()
  await expect(table.getByRole('button', { name: 'Edit Water', exact: true })).toHaveCount(0)
  await expect(table.getByRole('rowheader', { name: /^Total Expenses/ })).toBeVisible()
  await expect(table.getByRole('rowheader', { name: /^Total Debt Payments/ })).toHaveCount(0)
  await table.getByRole('button', { name: 'Collapse Income', exact: true }).click()
  await expect(table.getByRole('button', { name: 'Edit Salary', exact: true })).toHaveCount(0)
  await expect(table.getByRole('rowheader', { name: /^Net Income \/ Cash Received/ })).toBeVisible()
  await expect(table.getByRole('rowheader', { name: /^Cumulative Closing Cash/ })).toBeVisible()
  expect(await closing.innerText()).toBe(before)
  await page.getByLabel('First visible cycle').fill('2027-01')
  await expect(table.getByRole('button', { name: 'Expand Expenses', exact: true })).toHaveAttribute('aria-expanded', 'false')
  const expand = table.getByRole('button', { name: 'Expand Expenses', exact: true })
  await expand.focus(); await page.keyboard.press('Enter')
  // Parent collapse preserves the independently collapsed subsection.
  await expect(table.getByRole('button', { name: 'Expand Debt Payments', exact: true })).toBeVisible()
  await expect(table.getByRole('button', { name: 'Edit Mortgage', exact: true })).toHaveCount(0)
  await table.getByRole('button', { name: 'Expand Debt Payments', exact: true }).click()
  await expect(table.getByRole('button', { name: 'Edit Mortgage', exact: true })).toBeVisible()
  await table.getByRole('button', { name: 'Expand Income', exact: true }).click()
  await expect(table.getByRole('button', { name: 'Edit Salary', exact: true })).toBeVisible()
  expect(await page.evaluate(() => localStorage.getItem('planner-test'))).toBeNull()
})

test('credit card rows open a Billing context menu and navigate to the selected card', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Edit Salary', exact: true })).toBeVisible()
  await page.evaluate(() => {
    const native = (window as any).__TAURI_INTERNALS__
    const invoke = native.invoke
    native.invoke = async (command: string, args: any) => {
      if (command === 'get_credit_limit_groups') return { groups: [], cards: [] }
      if (command === 'get_card_billing') return { account: { id: args.accountId, name: 'SCB CardX BEYOND', current_balance: '1000', opening_balance: '1000', credit_limit: null }, currency: 'THB', billing: { statements: [], plans: [], allocations: [], installments: [] }, transactions: [], entries: [], installments: [] }
      return invoke(command, args)
    }
    const categories = ['income', 'deductions', 'debt', 'installments', 'cards', 'expenses'].map(id => ({ id, name: id, subtotal: `Total ${id}` }))
    localStorage.setItem('planner-test', JSON.stringify({ incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [{ id: 'scb', name: 'SCB CardX BEYOND', is_archived: false }], card_transactions: [], source_currency: 'THB', categories, period_start_day: 25, opening: null, months: [], amounts: [], expense_categories: [], items: [] }))
    window.dispatchEvent(new Event('accounts-changed'))
  })
  const row = page.getByRole('row').filter({ has: page.getByRole('link', { name: 'SCB CardX BEYOND', exact: true }) })
  await expect(row).toHaveAttribute('tabindex', '0')
  // Right-click a forecast cell, not just the card name.
  for (const theme of ['light', 'dark', 'heyday']) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme)
    await row.getByRole('cell').first().click({ button: 'right' })
    await expect(page.getByRole('menu', { name: 'SCB CardX BEYOND actions' })).toBeVisible()
    await expect(page.getByRole('menuitem', { name: 'Billing', exact: true })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByRole('menu')).toHaveCount(0)
  }
  await row.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Billing', exact: true }).focus()
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/#\/accounts\/scb\/billing$/)
  await expect(page.getByRole('heading', { name: 'SCB CardX BEYOND · Billing' })).toBeVisible()
})
