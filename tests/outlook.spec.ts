import { test, expect } from '@playwright/test'
import type { FinancialData, SavePaymentPlan, PaymentPlan } from '../src/lib/desktop'

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date(2026, 0, 15, 12))
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const accounts = [
      { id: 'bank', name: 'Everyday bank', type: 'bank', opening_balance: '92000', current_balance: '100000' },
      { id: 'card', name: 'Credit card', type: 'credit_card', opening_balance: '20000', current_balance: '20000' },
      { id: 'fund', name: 'Investments', type: 'investment', opening_balance: '500000', current_balance: '500000' },
    ].map(account => ({ ...account, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_millis: null }))
    const incomes = [{ id: 'salary', name: 'Salary', type: 'salary', destination_account_id: 'bank', destination_account_name: 'Everyday bank', estimated_amount: '30000', recurrence_frequency: 'monthly', recurrence_day_of_month: 25, is_active: true, is_auto_create_transaction: false, created_at: '', updated_at: '' }]
    const transactions = [{ id: 'received', type: 'income', amount: '10000' }, { id: 'spent', type: 'expense', amount: '2000' }].map(row => ({ ...row, account_id: 'bank', account_name: 'Everyday bank', destination_account_id: null, destination_account_name: null, date: '2026-01-10', description: '', payee_id: null, payee_name: null, category_id: 'housing', category_name: 'Housing' }))
    const plans = (): PaymentPlan[] => JSON.parse(localStorage.getItem('test-plans') ?? '[]')
    const categories = [{ id: 'housing', name: 'Housing', is_archived: false }]
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { input: SavePaymentPlan; id: string }) => {
      switch (command) {
        case 'get_financial_data': {
          if (sessionStorage.getItem('fail-outlook')) throw 'Load failed'
          return { accounts: sessionStorage.getItem('no-accounts') ? [] : accounts, settings: { currency: sessionStorage.getItem('no-currency') ? null : 'THB', period_start_day: 1 }, incomes, transactions, plans: plans(), categories } as FinancialData
        }
        case 'get_settings': return { currency: 'THB', period_start_day: 1 }
        case 'list_accounts': return accounts
        case 'save_payment_plan': {
          if (sessionStorage.getItem('fail-plan')) throw 'Could not save plan.'
          const plan = { ...args.input, id: args.input.id ?? crypto.randomUUID(), account_name: 'Everyday bank', destination_account_name: args.input.destination_account_id ? 'Credit card' : null, category_name: args.input.category_id ? 'Housing' : null }
          localStorage.setItem('test-plans', JSON.stringify([...plans().filter(item => item.id !== plan.id), plan]))
          return plan
        }
        case 'delete_payment_plan': localStorage.setItem('test-plans', JSON.stringify(plans().filter(plan => plan.id !== args.id))); return
        default: throw new Error(command)
      }
    } } })
  })
})

test('Outlook navigation preserves the Home hero and supports the full planning workflow', async ({ page }) => {
  await page.goto('/#/')
  await expect(page.getByRole('img', { name: /peaceful floating island/ })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Monthly Outlook', exact: true })).not.toBeVisible()
  const outlookLink = page.getByRole('navigation').getByRole('link', { name: 'Outlook', exact: true })
  await expect(outlookLink).toHaveAttribute('title', 'Outlook')
  await outlookLink.click()
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await expect(page).toHaveURL(/#\/outlook$/)
  await expect(page.getByRole('heading', { name: 'Outlook', exact: true })).toBeVisible()
  await expect(outlookLink).toHaveAttribute('aria-current', 'page')
  await expect(page.getByRole('heading', { name: 'Monthly Outlook', exact: true })).toBeVisible()
  await expect(page.getByRole('columnheader')).toHaveCount(8)
  const closing = page.getByRole('row', { name: /^Closing cash balance/ }).getByRole('cell').first()
  await expect(closing).toContainText('1,300.00 THB')
  await expect(closing).toContainText('Partial forecast')
  await page.getByRole('button', { name: 'Income', exact: true }).click()
  await expect(page.getByRole('rowheader', { name: 'Salary · Expected', exact: true })).toBeVisible()
  const otherFlows = page.getByRole('button', { name: 'Other cash movements', exact: true })
  await otherFlows.focus()
  await page.keyboard.press('Enter')
  await expect(otherFlows).toHaveAttribute('aria-expanded', 'true')
  await expect(page.getByRole('rowheader', { name: 'No other cash movements to break down in these periods.' })).toBeVisible()
  await page.keyboard.press('Enter')
  await expect(otherFlows).toHaveAttribute('aria-expanded', 'false')
  await page.getByRole('button', { name: 'Add payment plan', exact: true }).click()
  await page.getByLabel('Planned amount (THB)').fill('100')
  await page.getByLabel('Plan name', { exact: true }).fill('Rent')
  await page.getByLabel('Planned date').fill('2026-01-20')
  await page.getByLabel('Planned category').selectOption('housing')
  await page.evaluate(() => sessionStorage.setItem('fail-plan', '1'))
  await page.getByRole('button', { name: 'Save plan', exact: true }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Could not save plan')
  await page.evaluate(() => sessionStorage.removeItem('fail-plan'))
  await page.getByRole('button', { name: 'Save plan', exact: true }).click()
  await expect(page.getByRole('dialog')).not.toBeVisible()
  await expect(closing).toContainText('1,200.00 THB')
  await expect(page.getByRole('button', { name: 'Income', exact: true })).toHaveAttribute('aria-expanded', 'true')
  await expect(page.getByRole('rowheader', { name: 'Salary · Expected', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Add payment plan', exact: true }).click()
  await page.getByLabel('Plan type', { exact: true }).selectOption('repayment')
  await page.getByLabel('Debt account', { exact: true }).selectOption('card')
  await page.getByLabel('Planned amount (THB)').fill('50')
  await page.getByLabel('Plan name', { exact: true }).fill('Card payment')
  await page.getByRole('button', { name: 'Save plan', exact: true }).click()
  await expect(closing).toContainText('1,150.00 THB')
  await expect(closing).toContainText('Forecast · known plans')
  await page.reload()
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await expect(closing).toContainText('1,150.00 THB')
  await page.getByText('Payment plans (2)', { exact: true }).click()
  await page.getByRole('button', { name: 'Edit plan Rent' }).click()
  await expect(page.getByLabel('Planned amount (THB)')).toHaveValue('100.00')
  await page.getByLabel('Planned amount (THB)').fill('120')
  await page.getByRole('button', { name: 'Save plan', exact: true }).click()
  await expect(closing).toContainText('1,130.00 THB')
  await page.getByRole('button', { name: 'Remove plan Rent', exact: true }).click()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Rent', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Remove plan Rent', exact: true }).click()
  await page.getByRole('button', { name: 'Remove plan', exact: true }).click()
  await expect(closing).toContainText('1,250.00 THB')
  await expect(closing).toContainText('Partial forecast')
  await page.getByRole('button', { name: 'Plan for February 2026', exact: true }).click()
  await expect(page.getByLabel('Planned date')).toHaveValue('2026-02-01')
  await page.getByLabel('Plan name', { exact: true }).fill('Unsaved')
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Discard changes' }).click()
  await page.setViewportSize({ width: 760, height: 700 })
  const region = page.getByRole('region', { name: 'Monthly outlook comparison' })
  expect(await region.evaluate(element => element.scrollWidth > element.clientWidth)).toBe(true)
  expect(await page.locator('main').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await region.screenshot({ path: 'test-results/monthly-outlook.png' })
  await page.getByRole('link', { name: 'Net Worth', exact: true }).click()
  await expect(page.getByText('5,800.00 THB', { exact: true })).toBeVisible()
  await expect(page.getByRole('region', { name: 'Credit cards breakdown' })).toContainText('−200.00 THB')
})

test('outlook and net worth handle missing prerequisites and retry failed reads', async ({ page }) => {
  await page.goto('/#/outlook')
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-outlook', '1'))
  await page.reload()
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Could not load your monthly outlook')
  await page.evaluate(() => sessionStorage.removeItem('fail-outlook'))
  await page.getByRole('button', { name: 'Retry outlook' }).click()
  await expect(page.getByRole('columnheader')).toHaveCount(8)
  await page.evaluate(() => sessionStorage.setItem('no-currency', '1'))
  await page.reload()
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await expect(page.getByText(/Choose your currency in/)).toBeVisible()
  await expect(page.getByRole('button', { name: 'Add payment plan' })).toBeDisabled()
  await page.evaluate(() => { sessionStorage.removeItem('no-currency'); sessionStorage.setItem('no-accounts', '1') })
  await page.reload()
  await page.getByRole('button', { name: 'Account-Based Outlook', exact: true }).click()
  await expect(page.getByText(/Add a cash, bank, or digital wallet account in/)).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Net Worth', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'No accounts yet' })).toBeVisible()
})
