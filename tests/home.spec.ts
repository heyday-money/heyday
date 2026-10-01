import { expect, test, type Page } from '@playwright/test'
import type { Account, FinancialData, Income, Transaction } from '../src/lib/desktop'
import { homeCycle, homeOverview } from '../src/lib/home'

const account = (id: string, type: Account['type'], balance: string): Account => ({ id, name: id === 'bank' ? 'Everyday savings' : id === 'card' ? 'Travel card' : id, type, current_balance: balance, opening_balance: balance, loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: 25, payment_due_day: 5, interest_rate_ten_thousandths: null, monthly_installment: null })
const income = (id: string, destination: string, day: number, amount = '5000000'): Income => ({ id, name: id, destination_account_id: destination, destination_account_name: 'Everyday savings', type: 'salary', estimated_amount: amount, deductions_total: '500000', recurrence_day_of_month: day, recurrence_frequency: 'monthly', is_active: true, is_auto_create_transaction: false, created_at: '', updated_at: '' })
const transaction = (id: string, type: Transaction['type'], date: string, amount: string, source = 'bank', destination: string | null = null): Transaction => ({ id, type, date, amount, account_id: source, account_name: source === 'bank' ? 'Everyday savings' : 'Travel card', destination_account_id: destination, destination_account_name: destination ? 'Travel card' : null, description: id, payee_id: null, payee_name: null, category_id: null, category_name: null })
function example(): FinancialData {
  return { settings: { currency: 'THB', period_start_day: 25 },
    accounts: [account('bank', 'bank', '2485000'), account('card', 'credit_card', '850000')],
    incomes: [income('Monthly salary', 'bank', 25)],
    transactions: [transaction('Salary received', 'income', '2026-09-25', '4500000'), transaction('Lunch', 'expense', '2026-10-01', '12500'), transaction('New phone', 'expense', '2026-09-28', '300000', 'card'), transaction('Card payment', 'repayment', '2026-09-29', '100000', 'bank', 'card')],
    plans: [{ id: 'rent', name: 'Rent', type: 'expense', date: '2026-09-30', amount: '1200000', account_id: 'bank', account_name: 'Everyday savings', destination_account_id: null, destination_account_name: null, category_id: null, category_name: null }],
    card_billing: { statements: [{ id: 'bill', account_id: 'card', start_date: '2026-08-26', end_date: '2026-09-25', due_date: '2026-10-04', amount: '950000', minimum: '50000', needs_review: false }], plans: [], allocations: [{ statement_id: 'bill', transaction_id: 'payment', amount: '100000', date: '2026-09-29' }], installments: [] },
    categories: [], installments: [], subscriptions: [] }
}

test('Home sums exact actuals across archived history, excludes repayments, and never adds expected income', () => {
  const data = example()
  data.accounts.push(account('wallet', 'wallet', '3000'), account('cash', 'cash', '-1500'))
  data.accounts[0].current_balance = '9007199254740993'
  data.transactions.push(transaction('Archived account purchase', 'expense', '2026-09-27', '100', 'archived'), transaction('Old boundary', 'expense', '2026-09-24', '99999'), transaction('Future receipt', 'income', '2026-10-02', '999999'), transaction('Next boundary', 'expense', '2026-10-25', '99999'))
  const before = JSON.stringify(data)
  const result = homeOverview(data, '', new Date(2026, 9, 1))
  expect(result).toMatchObject({ cash: 9007199254742493n, incomeReceived: 4500000n, spending: 312600n })
  expect(result.cycle).toMatchObject({ startKey: '2026-09-25', endKey: '2026-10-25' })
  expect(result.upcomingIncome[0]).toMatchObject({ date: '2026-10-25', amount: 4500000n })
  expect(JSON.stringify(data)).toBe(before)
})

test('cycle and source dates clamp independently, preserve zero, and exclude inactive/unavailable sources', () => {
  const data = example()
  data.settings.period_start_day = 31
  data.incomes = [income('Month end', 'bank', 31, '500000'), { ...income('Zero', 'bank', 29, '0'), deductions_total: '0' }, { ...income('Inactive', 'bank', 28), is_active: false }, income('Unavailable', 'archived', 28)]
  expect(homeCycle(31, '2024-02', new Date(2024, 1, 29))).toMatchObject({ startKey: '2024-02-29', endKey: '2024-03-31' })
  const result = homeOverview(data, '', new Date(2024, 1, 29))
  expect(result.upcomingIncome.map(row => [row.source.id, row.date, row.amount])).toEqual([['Month end', '2024-02-29', 0n], ['Zero', '2024-02-29', 0n]])
  const march = homeOverview(data, '2024-03', new Date(2024, 1, 29))
  expect(march.incomeReceived).toBe(0n)
  expect(march.cash).toBe(result.cash)
  expect(homeCycle(31, '0001-02', new Date())).toMatchObject({ startKey: '0001-02-28', endKey: '0001-03-31' })
})

test('attention keeps one card reminder, respects applied payments and review flags, and never infers plan payment', () => {
  const data = example()
  data.card_billing!.statements.push({ ...data.card_billing!.statements[0], id: 'old', start_date: '2026-07-26', end_date: '2026-08-25', due_date: '2026-09-04', amount: '99999999' })
  data.plans.push({ ...data.plans[0], id: 'unavailable', account_id: 'archived', date: '2027-01-01' }, { ...data.plans[0], id: 'later', date: '2026-11-01' })
  const result = homeOverview(data, '', new Date(2026, 9, 1))
  const cardRows = result.attention.filter(item => item.kind === 'billing')
  expect(cardRows).toHaveLength(1)
  expect(cardRows[0]).toMatchObject({ amount: 850000n, detail: 'Minimum covered · balance remains' })
  expect(result.attention.find(item => item.id === 'plan:rent')?.title).toBe('Past plan · needs review')
  expect(result.attention.find(item => item.id === 'plan:unavailable')?.title).toBe('Plan account unavailable')
  expect(result.attention.find(item => item.id === 'plan:later')).toBeUndefined()
  data.card_billing!.statements[1].needs_review = true
  const review = homeOverview(data, '', new Date(2026, 9, 1)).attention.find(item => item.kind === 'billing')!
  expect(review.title).toBe('Statement needs review')
  expect(review.amount).toBeUndefined()
})

test('recent entries use newest dates, retain same-date order, and stop at five without future entries', () => {
  const data = example()
  data.transactions = Array.from({ length: 8 }, (_, i) => transaction(String(i), 'expense', i < 6 ? `2026-09-${25 + i}` : `2026-10-0${i - 5}`, '100'))
  data.transactions.push(transaction('same-date', 'expense', '2026-09-30', '100'))
  const recent = homeOverview(data, '', new Date(2026, 8, 30)).recent
  expect(recent.map(row => row.id)).toEqual(['5', 'same-date', '4', '3', '2'])
})

async function nativeSnapshot(page: Page, data = example()) {
  await page.clock.setFixedTime(new Date(2026, 9, 1, 12))
  await page.addInitScript(snapshot => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'get_financial_data') {
        if (sessionStorage.getItem('fail-home')) throw 'Could not load snapshot'
        return JSON.parse(localStorage.getItem('home-data') ?? JSON.stringify(snapshot))
      }
      if (command === 'list_accounts') return snapshot.accounts
      if (command === 'get_settings') return snapshot.settings
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'list_institutions') return { institutions: [], accounts: snapshot.accounts }
      if (command === 'list_card_limit_groups') return { currency: 'THB', groups: [], cards: snapshot.accounts.filter(a => a.type === 'credit_card') }
      if (command === 'get_cashflow_planner') throw 'Planner not used by Home'
      throw new Error(command)
    } } })
  }, data)
}

test('Home keeps its island, shows actuals, follows cycle selection, refreshes and links reminders to their management views', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', e => errors.push(e.message))
  await nativeSnapshot(page)
  await page.goto('/')
  await expect(page.getByRole('img', { name: /peaceful floating island/ })).toBeVisible()
  await expect(page.getByLabel('Home financial summary')).toContainText('24,850.00 THB')
  await expect(page.getByLabel('Home financial summary')).toContainText('45,000.00 THB')
  await expect(page.getByLabel('Home financial summary')).toContainText('3,125.00 THB')
  await expect(page.getByLabel('Payday cycle')).toHaveValue('2026-09')
  await expect(page.getByRole('region', { name: 'Upcoming expected income' })).toContainText('25 Oct 2026')
  await expect(page.getByRole('region', { name: 'Needs attention' })).toContainText('8,500.00 THB')
  await expect(page.getByRole('region', { name: 'Needs attention' })).toContainText('Past plan · needs review')
  await page.screenshot({ path: 'test-results/home-dashboard-heyday.png', animations: 'disabled' })
  await page.getByLabel('Home financial summary').scrollIntoViewIfNeeded()
  await page.screenshot({ path: 'test-results/home-dashboard-summary.png', animations: 'disabled' })
  await page.getByRole('region', { name: 'Recent transactions' }).scrollIntoViewIfNeeded()
  await page.screenshot({ path: 'test-results/home-dashboard-recent.png', animations: 'disabled' })
  await page.getByLabel('Payday cycle').fill('2026-08')
  await expect(page.getByLabel('Home financial summary')).toContainText('0.00 THB')
  await page.getByRole('button', { name: 'Current cycle', exact: true }).click()
  await expect(page.getByLabel('Payday cycle')).toHaveValue('2026-09')
  const updated = example()
  updated.accounts[0].current_balance = '2475000'
  updated.transactions.unshift(transaction('New expense', 'expense', '2026-10-01', '10000'))
  await page.evaluate(snapshot => { localStorage.setItem('home-data', JSON.stringify(snapshot)); window.dispatchEvent(new Event('transactions-changed')) }, updated)
  await expect(page.getByLabel('Home financial summary')).toContainText('24,750.00 THB')
  await expect(page.getByRole('region', { name: 'Recent transactions' })).toContainText('New expense')
  await expect(page.getByRole('link', { name: 'Review bill', exact: true })).toHaveAttribute('href', /accounts\/card\/billing/)
  await page.getByRole('link', { name: 'Review plan', exact: true }).click()
  await expect(page.getByRole('tab', { name: 'Account-Based Outlook', exact: true })).toHaveAttribute('aria-selected', 'true')
  expect(errors).toEqual([])
})

test('Home supports all appearances and narrow layouts, keeps estimates distinct, and shows retry instead of stale totals', async ({ page }) => {
  await nativeSnapshot(page)
  await page.goto('/')
  for (const theme of ['light', 'dark', 'heyday']) {
    await page.evaluate(value => { localStorage.setItem('theme', value) }, theme)
    await page.reload()
    await expect(page.getByLabel('Home financial summary')).toBeVisible()
    await page.setViewportSize({ width: 760, height: 560 })
    expect(await page.locator('main').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.getByRole('button', { name: 'Expand sidebar', exact: true }).click()
    const handle = page.getByRole('separator', { name: 'Resize sidebar', exact: true })
    await handle.focus(); await page.keyboard.press('End')
    await expect(page.getByRole('img', { name: /peaceful floating island/ })).toBeVisible()
    expect(await page.locator('main').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click()
    await page.screenshot({ path: `test-results/home-dashboard-${theme}-narrow.png`, animations: 'disabled', fullPage: true })
  }
  await page.evaluate(() => { sessionStorage.setItem('fail-home', '1'); window.dispatchEvent(new Event('transactions-changed')) })
  await expect(page.getByRole('alert')).toContainText('Could not load your Home overview')
  await expect(page.getByLabel('Home financial summary')).toHaveCount(0)
  await page.evaluate(() => sessionStorage.removeItem('fail-home'))
  await page.getByRole('button', { name: 'Retry overview', exact: true }).click()
  await expect(page.getByLabel('Home financial summary')).toBeVisible()
})

test('Home retains the island while first-run setup explains missing currency', async ({ page }) => {
  const data = example()
  data.settings.currency = null
  data.accounts = []; data.incomes = []; data.transactions = []; data.plans = []
  await nativeSnapshot(page, data)
  await page.goto('/')
  await expect(page.getByRole('img', { name: /peaceful floating island/ })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Choose your currency first' })).toBeVisible()
  await expect(page.getByLabel('Home financial summary')).toHaveCount(0)
  await expect(page.getByRole('link', { name: 'Choose currency', exact: true })).toHaveAttribute('href', /settings/)
})
