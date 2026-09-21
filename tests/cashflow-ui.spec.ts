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
    const initial: PlannerData = { incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], source_currency: 'THB', categories, period_start_day: 25, opening: null, months: [], amounts: [], expense_categories: [{ id: 'water', name: 'Water', is_archived: false }],
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

test('cycle amounts, opening cash, explicit completion and item edits survive reload and window changes', async ({ page }) => {
  await expect(page.getByRole('columnheader')).toHaveCount(8)
  await expect(page.getByLabel('First visible cycle')).toHaveValue('2026-12')
  await expect(page.getByRole('columnheader').nth(1)).toContainText('25 Dec 2026 – 24 Jan 2027')
  await expect(page.getByRole('columnheader').last()).toContainText('Jun 2027')
  await expect(page.getByText('Enter opening cash and its initial cycle')).toBeVisible()
  await page.getByRole('button', { name: 'Edit Salary 2026-12 amount', exact: true }).click()
  await page.getByLabel('Cycle amount (THB)').fill('50000')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByLabel('Status 2026-12')).toHaveValue('tracking')
  await page.getByRole('button', { name: 'Set opening cash' }).click()
  await page.getByLabel('Available cash and bank balance (THB)').fill('0')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  const closing = page.getByRole('row').last()
  await expect(closing).toContainText('Cumulative Closing Cash')
  await expect(closing.getByRole('cell').first()).toHaveText('50,000.00')
  await page.getByLabel('Status 2026-12').selectOption('complete')
  await page.getByRole('button', { name: 'Edit Salary', exact: true }).click()
  await page.getByLabel('Item name', { exact: true }).fill('Main salary')
  await page.getByLabel('Description (optional)').fill('Before payroll deductions')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await page.getByLabel('First visible cycle').fill('2027-07')
  await expect(closing.getByRole('cell').first()).toHaveText('50,000.00')
  await page.getByLabel('First visible cycle').fill('2026-12')
  await page.reload()
  await expect(page.getByRole('button', { name: 'Edit Main salary 2026-12 amount', exact: true })).toContainText('50,000.00')
  await page.getByRole('button', { name: 'Help: Main salary', exact: true }).focus()
  await expect(page.getByRole('tooltip')).toContainText('Before payroll deductions')
  const content = page.locator('[data-slot="tooltip-content"]').first()
  await expect(content).toHaveCSS('background-color', 'rgb(255, 255, 255)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  await expect(content).toHaveCSS('background-color', 'rgb(43, 46, 51)')
  await expect(content).toHaveCSS('color', 'rgb(246, 245, 239)')
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await page.keyboard.press('Escape')
  await expect(page.getByRole('tooltip')).toHaveCount(0)
  await page.evaluate(() => document.documentElement.dataset.theme = 'dark')
  for (const control of [
    page.getByRole('button', { name: 'Account-Based Outlook', exact: true }),
    page.getByRole('button', { name: 'Add item to Gross Income', exact: true }),
    page.getByRole('link', { name: 'Manage salary deductions in Income', exact: true }),
  ]) {
    await control.hover()
    await expect(control).toHaveCSS('color', 'rgb(246, 245, 239)')
  }
  await page.evaluate(() => document.documentElement.dataset.theme = 'light')
  await expect(page.getByLabel('Status 2026-12')).toHaveValue('complete')
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
  await expect(salaryLink).toHaveCSS('border-bottom-color', 'rgb(33, 37, 41)')
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
    data.debt_accounts = [{ id: 'loan', name: 'Home mortgage', loan_type: 'mortgage', current_balance: '250000000', notes: null, is_archived: false }]
    data.installments = [{ id: 'laptop', name: 'Work laptop', debt_account_id: 'card', debt_account_name: 'Visa', debt_account_type: 'credit_card', account_name: 'Bank', monthly_amount: '300000', installment_count: 2, first_due_date: '2026-12-25', accounts_available: true }]
    localStorage.setItem('planner-test', JSON.stringify(data))
  })
  await page.reload()
  await expect(page.getByRole('link', { name: 'Home mortgage', exact: true })).toHaveAttribute('href', /accounts/)
  await expect(page.getByRole('link', { name: 'Visa · Work laptop', exact: true })).toHaveAttribute('href', /installments/)
  await expect(page.getByRole('button', { name: 'Edit Home mortgage 2026-12 amount', exact: true })).toContainText('Enter amount')
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
  await expect(page.getByRole('link', { name: 'Visa', exact: true })).toHaveAttribute('href', /transactions/)
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
