import { expect, test } from '@playwright/test'
import type { Income, NewIncome } from '../src/lib/desktop'

async function setup(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string, args: { input?: NewIncome }) => {
        const sources = (): Income[] => JSON.parse(localStorage.getItem('test-income') ?? '[]')
        const account = { id: 'bank', name: 'Everyday bank', type: 'bank', opening_balance: '150000' }
        switch (command) {
          case 'plugin:app|version': return '0.1.0'
          case 'get_settings': return { currency: localStorage.getItem('test-currency'), period_start_day: 25 }
          case 'list_accounts': return localStorage.getItem('test-account') ? [account] : []
          case 'list_incomes': return sources()
          case 'create_income': {
            if (sessionStorage.getItem('fail-income')) throw 'Could not save income. Please try again.'
            const income = { ...args.input!, id: crypto.randomUUID(), destination_account_name: account.name, created_at: '', updated_at: '' }
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
  await expect(list.getByText('50,000.25 THB')).toHaveCount(4)
  await expect(list.getByText('Inactive', { exact: true })).toHaveCount(1)
  const records = await page.evaluate(() => JSON.parse(localStorage.getItem('test-income') ?? '[]') as Income[])
  expect(records.every(record => record.estimated_amount === '5000025' && !record.is_auto_create_transaction && record.destination_account_id === 'bank')).toBe(true)
})
