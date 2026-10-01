import { expect, test } from '@playwright/test'
import type { Income, IncomeDeduction, IncomeDeductionInput, NewIncome } from '../src/lib/desktop'

async function setup(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string, args: { input?: NewIncome & { id?: string; income_id?: string; deductions?: IncomeDeductionInput[] }; incomeId?: string }) => {
        const sources = (): Income[] => JSON.parse(localStorage.getItem('test-income') ?? '[]')
        const deductions = (): IncomeDeduction[] => JSON.parse(localStorage.getItem('test-income-deductions') ?? '[]')
        const account = { id: 'bank', name: 'Everyday bank', type: 'bank', opening_balance: '150000' }
        switch (command) {
          case 'plugin:app|version': return '0.1.0'
          case 'get_settings': return { currency: localStorage.getItem('test-currency'), period_start_day: 25 }
          case 'list_transaction_options': return { payees: [], categories: [] }
          case 'list_accounts': return !localStorage.getItem('test-no-active-account') && localStorage.getItem('test-account') ? [account, ...(localStorage.getItem('test-other-account') ? [{ id: 'other', name: 'Other bank', type: 'bank', opening_balance: '0' }] : []), ...(localStorage.getItem('test-loan') ? [{ id: 'student', name: 'Student Loan', type: 'loan' }] : [])] : []
          case 'list_incomes': return sources()
          case 'list_income_deductions': return deductions().filter(row => row.income_id === args.incomeId)
          case 'save_salary_deductions': {
            if (sessionStorage.getItem('fail-income')) throw 'Could not save deductions.'
            const rows = (args.input?.deductions ?? []).map(row => ({ ...row, id: row.id ?? crypto.randomUUID(), income_id: args.input!.income_id! }))
            const source = { ...sources().find(row => row.id === args.input!.income_id)!, deductions_total: rows.reduce((sum, row) => sum + BigInt(row.amount), 0n).toString() }
            localStorage.setItem('test-income-deductions', JSON.stringify([...deductions().filter(row => row.income_id !== source.id), ...rows]))
            localStorage.setItem('test-income', JSON.stringify(sources().map(row => row.id === source.id ? source : row)))
            return source
          }
          case 'get_cashflow_planner': return {
            categories: [{ id: 'income', name: 'Gross Income', subtotal: 'Total Gross Income' }, { id: 'deductions', name: 'Income Deductions', subtotal: 'Total Deductions' }],
            incomes: sources().map(source => ({ ...source, account_archived: false })), income_deductions: deductions(), source_currency: 'THB', debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], items: [], amounts: [], months: [], opening: null, period_start_day: 25, expense_categories: [],
          }
          case 'update_income': {
            if (sessionStorage.getItem('fail-income')) throw 'Could not save income. Please try again.'
            if (sessionStorage.getItem('pause-update')) await new Promise<void>(resolve => { (window as any).finishIncomeUpdate = resolve })
            const existing = sources().find(source => source.id === args.input!.id)!
            const income = { ...existing, ...args.input!, destination_account_name: args.input!.destination_account_id === 'other' ? 'Other bank' : existing.destination_account_id === args.input!.destination_account_id ? existing.destination_account_name : account.name, updated_at: 'updated' }
            localStorage.setItem('test-income', JSON.stringify(sources().map(source => source.id === income.id ? income : source)))
            sessionStorage.setItem('update-input', JSON.stringify(args.input))
            return income
          }
          case 'create_income': {
            if (sessionStorage.getItem('fail-income')) throw 'Could not save income. Please try again.'
            const id = crypto.randomUUID()
            const rows = (args.input?.deductions ?? []).map(row => ({ ...row, id: crypto.randomUUID(), income_id: id }))
            const income = { ...args.input!, id, deductions_total: rows.reduce((sum, row) => sum + BigInt(row.amount), 0n).toString(), destination_account_name: account.name, created_at: '', updated_at: '' }
            localStorage.setItem('test-income-deductions', JSON.stringify([...deductions(), ...rows]))
            localStorage.setItem('test-income', JSON.stringify([...sources(), income]))
            return income
          }
          default: throw new Error(`Unexpected command ${command}`)
        }
      },
    } })
  })
}

test('income prerequisites and all source types with safe save and reload', async ({ page }) => {
  await setup(page)
  await page.goto('/#/income')
  await expect(page.getByRole('heading', { name: 'Choose your currency first' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Add income source', exact: true })).toBeDisabled()
  await page.evaluate(() => localStorage.setItem('test-currency', 'THB'))
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Add a destination account first' })).toBeVisible()
  await page.evaluate(() => localStorage.setItem('test-account', 'true'))
  await page.reload()
  const trigger = page.getByRole('button', { name: 'Add income source', exact: true })
  const dialog = page.getByRole('dialog', { name: 'Add income source', exact: true })
  await trigger.click()
  await expect(page.getByLabel('Income name', { exact: true })).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(dialog).not.toBeVisible()
  await expect(trigger).toBeFocused()
  await trigger.click()
  await page.getByLabel('Income name', { exact: true }).fill('Unsaved salary')
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await expect(page.getByLabel('Income name', { exact: true })).toHaveValue('Unsaved salary')
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('button', { name: 'Discard changes' }).click()
  for (const type of ['salary', 'variable', 'investment', 'other']) {
    await trigger.click()
    await page.getByLabel('Income name', { exact: true }).fill(`My ${type}`)
    await page.getByLabel('Income type', { exact: true }).selectOption(type)
    await page.getByLabel('Destination account', { exact: true }).selectOption('bank')
    await page.getByLabel('Day of month', { exact: true }).selectOption('31')
    await page.getByLabel('Estimated amount (THB)').fill('50000.25')
    await expect(page.getByLabel('Frequency', { exact: true })).toHaveValue('monthly')
    await expect(page.getByLabel('Automatically create transactions')).toBeDisabled()
    await expect(page.getByLabel('Automatically create transactions')).not.toBeChecked()
    if (type === 'other') await page.getByLabel('Status', { exact: true }).selectOption('false')
    if (type === 'salary') {
      await page.getByLabel('Estimated amount (THB)').fill('-1')
      await page.getByRole('button', { name: 'Save income source', exact: true }).click()
      await expect(dialog.getByRole('alert')).toHaveText('Estimated amount cannot be negative.')
      await page.getByLabel('Estimated amount (THB)').fill('50000.25')
      await page.evaluate(() => sessionStorage.setItem('fail-income', 'true'))
      await page.getByRole('button', { name: 'Save income source', exact: true }).click()
      await expect(dialog.getByRole('alert')).toContainText('Could not save income')
      await expect(page.getByLabel('Income name', { exact: true })).toHaveValue('My salary')
      await page.evaluate(() => sessionStorage.removeItem('fail-income'))
      await page.setViewportSize({ width: 760, height: 560 })
      await expect.poll(async () => {
        const button = await page.getByRole('button', { name: 'Save income source', exact: true }).boundingBox()
        return button ? button.y + button.height : Infinity
      }).toBeLessThan(560)
      await page.screenshot({ path: 'test-results/income-dialog.png' })
      await page.setViewportSize({ width: 1200, height: 800 })
    }
    await page.getByRole('button', { name: 'Save income source', exact: true }).click()
    await expect(dialog).not.toBeVisible()
    await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Income source added.' })).toBeVisible()
    await expect(page.getByRole('heading', { name: `My ${type}`, exact: true })).toBeVisible()
  }
  await page.reload()
  const list = page.getByRole('list', { name: 'Income sources' })
  await expect(list.getByRole('listitem')).toHaveCount(4)
  await expect(list.getByText('50,000.25 THB', { exact: true })).toHaveCount(4)
  await expect(list.getByText('Inactive', { exact: true })).toHaveCount(1)
  const records = await page.evaluate(() => JSON.parse(localStorage.getItem('test-income') ?? '[]') as Income[])
  expect(records.every(record => record.estimated_amount === '5000025' && !record.is_auto_create_transaction && record.destination_account_id === 'bank')).toBe(true)
})


test('salary deductions can be created and managed on Income and appear separately in Outlook', async ({ page }) => {
  await setup(page)
  await page.goto('/#/income')
  await page.evaluate(() => { localStorage.setItem('test-currency', 'THB'); localStorage.setItem('test-account', 'true') })
  await page.reload()
  await page.getByRole('button', { name: 'Add income source', exact: true }).click()
  await page.getByLabel('Income name', { exact: true }).fill('Monthly Salary')
  await page.getByLabel('Destination account', { exact: true }).selectOption('bank')
  await page.getByLabel('Estimated amount (THB)').fill('50000')
  await page.getByLabel('Day of month', { exact: true }).selectOption('25')
  await page.getByLabel('Deduction 1 amount per month (THB)', { exact: true }).fill('2000')
  await page.getByLabel('Deduction 2 amount per month (THB)', { exact: true }).fill('750')
  await expect(page.getByText('Estimated Net Salary: 47,250.00 THB')).toBeVisible()
  await page.getByLabel('Income type', { exact: true }).selectOption('other')
  await expect(page.getByRole('region', { name: 'Salary deductions' })).not.toBeVisible()
  await page.getByLabel('Income type', { exact: true }).selectOption('salary')
  await expect(page.getByLabel('Deduction 1 amount per month (THB)', { exact: true })).toHaveValue('2000')
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await page.reload()
  await expect(page.getByText('Net: 47,250.00 THB', { exact: true })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Outlook', exact: true }).click()
  await expect(page.getByRole('link', { name: 'Monthly Salary · Withholding Tax', exact: true })).toBeVisible()
  const net = page.getByRole('row', { name: /^Net Income \/ Cash Received/ }).last()
  await expect(net.getByRole('cell').first()).toHaveText('47,250.00')
  await page.getByRole('link', { name: 'Manage salary deductions in Income', exact: true }).click()
  await page.getByRole('button', { name: 'Manage deductions for Monthly Salary', exact: true }).click()
  await page.getByLabel('Deduction 1 amount per month (THB)', { exact: true }).fill('51000')
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Total deductions cannot exceed gross salary.')
  await page.getByLabel('Deduction 1 amount per month (THB)', { exact: true }).fill('1000')
  await page.keyboard.press('Escape')
  await expect(page.getByText('Discard unsaved deduction changes?', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-income', 'true'))
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Could not save deductions.')
  await expect(page.getByLabel('Deduction 1 amount per month (THB)', { exact: true })).toHaveValue('1000')
  await page.evaluate(() => sessionStorage.removeItem('fail-income'))
  await page.getByRole('button', { name: 'Remove deduction 2', exact: true }).click()
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await page.reload()
  await expect(page.getByText('Net: 49,000.00 THB', { exact: true })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Outlook', exact: true }).click()
  await expect(net.getByRole('cell').first()).toHaveText('49,000.00')
  await expect(page.getByRole('link', { name: 'Monthly Salary · Social Security', exact: true })).toHaveCount(0)
})

test('salary deduction links to a student loan and can be unlinked after reload', async ({ page }) => {
  await setup(page)
  await page.goto('/#/income')
  await page.evaluate(() => { localStorage.setItem('test-currency', 'THB'); localStorage.setItem('test-account', 'true'); localStorage.setItem('test-loan', 'true') })
  await page.reload()
  await page.getByRole('button', { name: 'Add income source', exact: true }).click()
  await page.getByLabel('Income name', { exact: true }).fill('Salary')
  await page.getByLabel('Income type', { exact: true }).selectOption('salary')
  await page.getByLabel('Destination account', { exact: true }).selectOption('bank')
  await page.getByLabel('Estimated amount (THB)').fill('50000')
  await page.getByLabel('Deduction 4 amount per month (THB)', { exact: true }).fill('2000')
  await page.getByLabel('Deduction 4 debt account (optional)').selectOption('student')
  await expect(page.getByLabel('Deduction 4 debt account (optional)').getByRole('option', { name: 'Everyday bank' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(page.getByText('Net: 48,000.00 THB', { exact: true })).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: 'Manage deductions for Salary', exact: true }).click()
  await expect(page.getByLabel('Deduction 1 debt account (optional)')).toHaveValue('student')
  await page.getByLabel('Deduction 1 debt account (optional)').selectOption('')
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await page.reload()
  await page.getByRole('button', { name: 'Manage deductions for Salary', exact: true }).click()
  await expect(page.getByLabel('Deduction 1 debt account (optional)')).toHaveValue('')
  await expect(page.getByLabel('Deduction 1 amount per month (THB)', { exact: true })).toHaveValue('2000.00')
})

test('edit income preserves deductions, protects drafts, supports disable/reactivate, and keeps automation disabled', async ({ page }) => {
  await setup(page)
  await page.goto('/#/income')
  await page.evaluate(() => {
    localStorage.setItem('test-currency', 'THB'); localStorage.setItem('test-account', 'true'); localStorage.setItem('test-other-account', 'true')
    localStorage.setItem('test-income', JSON.stringify([{ id: 'salary', name: 'Monthly Salary', type: 'salary', estimated_amount: '5000000', deductions_total: '10000', destination_account_id: 'bank', destination_account_name: 'Everyday bank', recurrence_frequency: 'monthly', recurrence_day_of_month: 28, is_active: true, is_auto_create_transaction: false, created_at: 'original', updated_at: 'original' }]))
    localStorage.setItem('test-income-deductions', JSON.stringify([{ id: 'tax', income_id: 'salary', name: 'Tax', description: '', amount: '10000' }]))
  })
  await page.reload()
  await page.getByRole('button', { name: 'Edit Monthly Salary', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Edit income source', exact: true })
  await expect(page.getByLabel('Income name', { exact: true })).toHaveValue('Monthly Salary')
  await expect(page.getByLabel('Estimated amount (THB)')).toHaveValue('50000.00')
  await expect(page.getByLabel('Day of month', { exact: true })).toHaveValue('28')
  await expect(page.getByLabel('Income type', { exact: true })).toBeDisabled()
  await expect(dialog).toContainText('including past estimates')
  await expect(page.getByLabel('Automatically create transactions')).toBeDisabled()
  await page.getByLabel('Income name', { exact: true }).fill('Updated salary')
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Keep editing', exact: true }).click()
  await expect(page.getByLabel('Income name', { exact: true })).toHaveValue('Updated salary')
  await page.getByLabel('Estimated amount (THB)').fill('99')
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('existing deductions')
  await page.getByLabel('Estimated amount (THB)').fill('90071992547409.93')
  await page.getByLabel('Destination account', { exact: true }).selectOption('other')
  await page.getByLabel('Day of month', { exact: true }).selectOption('31')
  await page.getByLabel('Status', { exact: true }).selectOption('false')
  await page.setViewportSize({ width: 760, height: 560 })
  await expect(page.getByRole('button', { name: 'Save income source', exact: true })).toBeInViewport()
  await page.screenshot({ path: 'test-results/income-edit-dialog.png', animations: 'disabled' })
  await page.setViewportSize({ width: 1200, height: 800 })
  await page.evaluate(() => sessionStorage.setItem('fail-income', 'true'))
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not save income')
  await expect(page.getByLabel('Status', { exact: true })).toHaveValue('false')
  await page.evaluate(() => { sessionStorage.removeItem('fail-income'); sessionStorage.setItem('pause-update', 'true') })
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Saving…', exact: true })).toBeDisabled()
  await page.keyboard.press('Escape'); await expect(dialog).toBeVisible()
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled()
  await page.evaluate(() => (window as any).finishIncomeUpdate())
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Edit Updated salary', exact: true })).toBeFocused()
  await expect(page.getByText('Income source updated.', { exact: true })).toBeVisible()
  const source = await page.evaluate(() => JSON.parse(localStorage.getItem('test-income')!)[0])
  expect(source).toMatchObject({ id: 'salary', name: 'Updated salary', estimated_amount: '9007199254740993', deductions_total: '10000', recurrence_day_of_month: 31, destination_account_id: 'other', is_active: false, created_at: 'original', is_auto_create_transaction: false })
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('test-income-deductions')!))).toEqual([{ id: 'tax', income_id: 'salary', name: 'Tax', description: '', amount: '10000' }])
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('update-input')!).deductions)).toBeUndefined()
  await page.reload()
  await expect(page.getByText('Inactive', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Edit Updated salary', exact: true }).click()
  await page.getByLabel('Status', { exact: true }).selectOption('true')
  await page.evaluate(() => sessionStorage.removeItem('pause-update'))
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('list', { name: 'Income sources' }).getByText('Active', { exact: true })).toBeVisible()
  await expect(page.getByRole('list', { name: 'Income sources' }).getByRole('listitem')).toHaveCount(1)
  await page.getByRole('button', { name: 'Edit Updated salary', exact: true }).click()
  await page.getByLabel('Income name', { exact: true }).fill('Discarded edit')
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Updated salary', exact: true })).toBeVisible()
})

test('inactive sources remove generated estimates while explicit overrides and actual receipts stay intact', async () => {
  const { cycleTotals, plannerItems, itemAmount } = await import('../src/lib/cashflow')
  const data: import('../src/lib/cashflow').PlannerData = { source_currency: 'THB', period_start_day: 1, categories: [], items: [], months: [], opening: null, debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], expense_categories: [],
    incomes: [{ id: 'salary', name: 'Salary', estimated_amount: '5000000', recurrence_day_of_month: 28, destination_account_name: 'Bank', is_active: true, account_archived: false }],
    income_deductions: [{ id: 'tax', income_id: 'salary', name: 'Tax', description: '', amount: '10000' }], amounts: [],
    ledger_transactions: [{ id: 'receipt', type: 'income', account_id: 'bank', account_type: 'bank', destination_account_id: null, destination_account_type: null, amount: '4990000', date: '2026-01-28', income_source_id: 'salary' }],
  }
  expect(cycleTotals(data, '2026-01', 'forecast').netIncome).toBe(4990000n)
  const actual = cycleTotals(data, '2026-01', 'actual').netIncome
  data.incomes[0].is_active = false
  expect(cycleTotals(data, '2026-01', 'forecast').netIncome).toBe(0n)
  expect(cycleTotals(data, '2026-01', 'actual').netIncome).toBe(actual)
  const override = { ...data, amounts: [{ item_id: 'income:salary', month: '2026-01', amount: '12345' }] }
  expect(itemAmount(override, plannerItems(override).find(item => item.income?.id === 'salary')!, '2026-01', 'forecast')).toMatchObject({ value: 12345n, source: 'Entered' })
  data.incomes[0].is_active = true
  data.incomes[0].recurrence_day_of_month = 31
  data.incomes[0].estimated_amount = '6000000'
  expect(cycleTotals(data, '2026-02', 'forecast').netIncome).toBe(5990000n)
  expect(cycleTotals(data, '2026-01', 'actual').netIncome).toBe(actual)
})

test('can disable a source with an unavailable destination and edit explicit zero in JPY', async ({ page }) => {
  await setup(page); await page.goto('/#/income')
  await page.evaluate(() => {
    localStorage.setItem('test-currency', 'JPY'); localStorage.setItem('test-no-active-account', 'true')
    localStorage.setItem('test-income', JSON.stringify([{ id: 'archived-source', name: 'Archived source', type: 'other', estimated_amount: '9007199254740993', deductions_total: '0', destination_account_id: 'archived', destination_account_name: 'Archived bank', recurrence_frequency: 'monthly', recurrence_day_of_month: 1, is_active: true, is_auto_create_transaction: false, created_at: 'original', updated_at: 'original' }]))
  })
  await page.reload()
  await expect(page.getByRole('button', { name: 'Add income source', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Edit Archived source', exact: true }).click()
  await expect(page.getByLabel('Estimated amount (JPY)')).toHaveValue('9007199254740993')
  await expect(page.getByLabel('Destination account', { exact: true })).toHaveValue('archived')
  await page.getByLabel('Estimated amount (JPY)').fill('0')
  await page.getByLabel('Status', { exact: true }).selectOption('false')
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  const input = await page.evaluate(() => JSON.parse(sessionStorage.getItem('update-input')!))
  expect(input).toMatchObject({ id: 'archived-source', estimated_amount: '0', currency: 'JPY', destination_account_id: 'archived', is_active: false, is_auto_create_transaction: false })
  await expect(page.getByText('Inactive', { exact: true })).toBeVisible()
})
