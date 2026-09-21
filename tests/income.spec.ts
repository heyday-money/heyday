import { expect, test } from '@playwright/test'
import type { Income, IncomeDeduction, IncomeDeductionInput, NewIncome } from '../src/lib/desktop'

async function setup(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string, args: { input?: NewIncome & { income_id?: string; deductions?: IncomeDeductionInput[] }; incomeId?: string }) => {
        const sources = (): Income[] => JSON.parse(localStorage.getItem('test-income') ?? '[]')
        const deductions = (): IncomeDeduction[] => JSON.parse(localStorage.getItem('test-income-deductions') ?? '[]')
        const account = { id: 'bank', name: 'Everyday bank', type: 'bank', opening_balance: '150000' }
        switch (command) {
          case 'plugin:app|version': return '0.1.0'
          case 'get_settings': return { currency: localStorage.getItem('test-currency'), period_start_day: 25 }
          case 'list_transaction_options': return { payees: [], categories: [] }
          case 'list_accounts': return localStorage.getItem('test-account') ? [account] : []
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
  await page.getByLabel('Deduction 1 (THB)', { exact: true }).fill('2000')
  await page.getByLabel('Deduction 2 (THB)', { exact: true }).fill('750')
  await expect(page.getByText('Estimated Net Salary: 47,250.00 THB')).toBeVisible()
  await page.getByLabel('Income type', { exact: true }).selectOption('other')
  await expect(page.getByRole('region', { name: 'Salary deductions' })).not.toBeVisible()
  await page.getByLabel('Income type', { exact: true }).selectOption('salary')
  await expect(page.getByLabel('Deduction 1 (THB)', { exact: true })).toHaveValue('2000')
  await page.getByRole('button', { name: 'Save income source', exact: true }).click()
  await page.reload()
  await expect(page.getByText('Net: 47,250.00 THB', { exact: true })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Outlook', exact: true }).click()
  await expect(page.getByRole('link', { name: 'Monthly Salary · Withholding Tax', exact: true })).toBeVisible()
  const net = page.getByRole('row', { name: /^Net Income After Deductions/ }).last()
  await expect(net.getByRole('cell').first()).toHaveText('47,250.00')
  await page.getByRole('link', { name: 'Manage salary deductions in Income', exact: true }).click()
  await page.getByRole('button', { name: 'Manage deductions for Monthly Salary', exact: true }).click()
  await page.getByLabel('Deduction 1 (THB)', { exact: true }).fill('51000')
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Total deductions cannot exceed gross salary.')
  await page.getByLabel('Deduction 1 (THB)', { exact: true }).fill('1000')
  await page.keyboard.press('Escape')
  await expect(page.getByText('Discard unsaved deduction changes?', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-income', 'true'))
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Could not save deductions.')
  await expect(page.getByLabel('Deduction 1 (THB)', { exact: true })).toHaveValue('1000')
  await page.evaluate(() => sessionStorage.removeItem('fail-income'))
  await page.getByRole('button', { name: 'Remove deduction 2', exact: true }).click()
  await page.getByRole('button', { name: 'Save deductions', exact: true }).click()
  await page.reload()
  await expect(page.getByText('Net: 49,000.00 THB', { exact: true })).toBeVisible()
  await page.getByRole('navigation').getByRole('link', { name: 'Outlook', exact: true }).click()
  await expect(net.getByRole('cell').first()).toHaveText('49,000.00')
  await expect(page.getByRole('link', { name: 'Monthly Salary · Social Security', exact: true })).toHaveCount(0)
})
