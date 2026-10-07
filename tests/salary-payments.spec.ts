import { test, expect, type Page } from '@playwright/test'
async function setup(page: Page) {
  await page.addInitScript(() => {
    const bank = { id: 'bank', name: 'Bank', type: 'bank', current_balance: '0', opening_balance: '0', is_archived: false }
    const loan = { ...bank, id: 'loan', name: 'Home loan', type: 'loan', current_balance: '10000000' }
    const salary = { id: 'salary', name: 'Monthly salary', type: 'salary', destination_account_id: 'bank', destination_account_name: 'Bank', estimated_amount: '5000000', deductions_total: '920000', recurrence_day_of_month: 25, is_active: true }
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: any) => {
      switch (command) {
        case 'plugin:app|version': return '0.0.1-beta.2'
        case 'get_settings': return { currency: 'THB', period_start_day: 25 }
        case 'list_accounts': return [bank, loan]
        case 'list_incomes': return [salary]
        case 'list_institutions': return { institutions: [], accounts: [bank, loan] }
        case 'list_credit_limit_groups': return []
        case 'list_transaction_options': return { payees: [], categories: [] }
        case 'list_income_deductions': return [{ id: 'deduction', income_id: 'salary', name: 'Home payment', amount: '920000', description: '', debt_account_id: 'loan', debt_account_name: 'Home loan' }]
        case 'get_loan_account': return { account: loan, currency: 'THB', contracts: [], transactions: [], payment_parts: [], facility: null }
        case 'record_salary_payment': {
          localStorage.setItem('submitted-payroll', JSON.stringify(args.input))
          if (localStorage.getItem('fail-payroll')) throw 'Salary deductions changed. Reload before recording.'
          const d = args.input.deductions[0]
          localStorage.setItem('salary-history', JSON.stringify([{ id: 'saved', income_id: 'salary', date: args.input.date, occurrence: args.input.occurrence, gross: args.input.gross, net: (BigInt(args.input.gross) - BigInt(d.amount)).toString(), breakdown: JSON.stringify([{ ...d, name: 'Home payment', debt_account_name: 'Home loan', principal: (BigInt(d.amount) - BigInt(d.interest) - BigInt(d.fee)).toString() }]) }]))
          return null
        }
        case 'list_salary_payments': return JSON.parse(localStorage.getItem('salary-history') ?? '[]')
        case 'delete_salary_payment': localStorage.setItem('salary-history', '[]'); return null
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
  await page.goto('/#/income')
}
test('recording confirms exact payslip amounts, retains failures, and supports history and reversal', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', e => errors.push(e.message))
  await setup(page)
  await page.getByRole('button', { name: 'Record salary payment', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('button', { name: 'Record salary payment', exact: true })).toBeDisabled()
  await dialog.getByLabel('Salary month', { exact: true }).fill('2024-01')
  await dialog.getByLabel('Payment date', { exact: true }).fill('2024-01-25')
  await dialog.getByLabel('Home payment · Interest', { exact: true }).fill('2000')
  await dialog.getByLabel('Home payment · Fees', { exact: true }).fill('200')
  await expect(dialog).toContainText('7,000.00 THB')
  await expect(dialog).toContainText('40,800.00 THB')
  await page.screenshot({ path: '/tmp/heyday-salary-payment.png', fullPage: true, animations: 'disabled' })
  await dialog.getByLabel('I confirm these actual payslip amounts and linked repayments.').check()
  await page.evaluate(() => localStorage.setItem('fail-payroll', 'yes'))
  await dialog.getByRole('button', { name: 'Record salary payment', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Salary deductions changed')
  await expect(dialog.getByLabel('Home payment · Interest', { exact: true })).toHaveValue('2000')
  await page.evaluate(() => localStorage.removeItem('fail-payroll'))
  await dialog.getByRole('button', { name: 'Record salary payment', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  const input = await page.evaluate(() => JSON.parse(localStorage.getItem('submitted-payroll')!))
  expect(input).toMatchObject({ gross: '5000000', confirmed: true, occurrence: '2024-01', deductions: [{ debt_account_id: 'loan', amount: '920000', interest: '200000', fee: '20000', contract_id: null }] })
  await page.getByRole('button', { name: 'More actions for Monthly salary', exact: true }).click()
  await page.getByRole('menuitem', { name: 'Salary payment history', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('7,000.00 THB')
  await page.getByRole('button', { name: 'Reverse salary payment', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Reconciled entries')
  await page.getByRole('button', { name: 'Confirm reversal', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('No salary payments recorded.')
  expect(errors).toEqual([])
})
test('salary recording protects changed drafts and requires renewed confirmation after amount edits', async ({ page }) => {
  await setup(page)
  await page.getByRole('button', { name: 'Record salary payment', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await dialog.getByLabel('Actual gross salary').fill('51000')
  await dialog.getByLabel('I confirm these actual payslip amounts and linked repayments.').check()
  await dialog.getByLabel('Home payment · Interest', { exact: true }).fill('100')
  await expect(dialog.getByRole('button', { name: 'Record salary payment', exact: true })).toBeDisabled()
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(dialog).toContainText('Discard unsaved changes?')
  await dialog.getByRole('button', { name: 'Keep editing', exact: true }).click()
  await expect(dialog.getByLabel('Actual gross salary')).toHaveValue('51000')
})

test('salary-linked account details explain the locked installment while other details can be saved', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => {
    const api = (window as any).__TAURI_INTERNALS__
    const original = api.invoke
    const account = { id: 'loan', name: 'Home loan', type: 'loan', loan_type: 'mortgage', current_balance: '10000000', opening_balance: '10000000', monthly_installment: '920000', payroll_linked: true, is_archived: false, institution: null, credit_limit: null, interest_rate_ten_thousandths: null }
    api.invoke = async (command: string, args: any) => {
      if (command === 'list_accounts') return [account]
      if (command === 'get_loan_account') return { account, currency: 'THB', contracts: [], transactions: [], payment_parts: [], facility: null }
      if (command === 'list_card_limit_groups') return { groups: [], cards: [] }
      if (command === 'get_selective_default') return { periods: [], period_start_day: 25, linked_items: [] }
      if (command === 'update_account') { localStorage.setItem('saved-linked-account', JSON.stringify(args.input)); return { ...account, ...args.input } }
      return original(command, args)
    }
    location.hash = '/accounts/loan/details'
  })
  await expect(page.getByText('Deducted from salary', { exact: false })).toBeVisible()
  await page.getByRole('button', { name: 'Edit account Home loan', exact: true }).click()
  const dialog = page.getByRole('dialog')
  const installment = dialog.getByLabel('Monthly installment (THB, optional)')
  await expect(installment).toHaveValue('9200.00')
  await expect(installment).toHaveAttribute('readonly', '')
  await expect(dialog).toContainText('Manage the deduction or remove its account link in Income.')
  await dialog.getByLabel('Account name', { exact: true }).fill('Renamed home')
  await dialog.getByRole('button', { name: 'Save account', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('saved-linked-account')!))).toMatchObject({ name: 'Renamed home', monthly_installment: '920000' })
})
