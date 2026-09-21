import { expect, test } from '@playwright/test'

test('data reset requires exact confirmation, handles failure, then reloads defaults', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { confirmation?: string }) => {
      const cleared = localStorage.getItem('reset-done') === 'true'
      if (command === 'get_settings') return { currency: cleared ? null : 'THB', period_start_day: cleared ? 1 : 25 }
      if (command === 'plugin:app|version') return '0.0.1'
      if (command === 'list_accounts' || command === 'list_incomes') return []
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'clear_all_data') {
        if (args.confirmation !== 'DELETE ALL DATA') throw new Error('Invalid confirmation')
        const calls = Number(localStorage.getItem('reset-calls') ?? '0') + 1
        localStorage.setItem('reset-calls', String(calls))
        if (calls === 1) throw new Error('Simulated database failure')
        localStorage.setItem('reset-done', 'true')
        return
      }
      return []
    } } })
  })
  await page.goto('/#/settings')
  await page.getByRole('button', { name: 'Clear all data', exact: true }).click()
  const confirm = page.getByRole('button', { name: 'Permanently clear all data' })
  await expect(confirm).toBeDisabled()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect(await page.evaluate(() => localStorage.getItem('reset-calls'))).toBeNull()
  await page.getByRole('button', { name: 'Clear all data', exact: true }).click()
  await page.getByLabel('Type DELETE ALL DATA to confirm').fill('delete all data')
  await expect(confirm).toBeDisabled()
  await page.getByLabel('Type DELETE ALL DATA to confirm').fill('DELETE ALL DATA')
  await confirm.click()
  await expect(page.getByRole('alert')).toContainText('No changes were saved')
  await expect(page.getByLabel('Period start day')).toHaveValue('25')
  await confirm.click()
  await expect(page.getByLabel('Period start day')).toHaveValue('1')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByText('All data cleared. Choose your currency to start again.')).toBeVisible()
  expect(await page.evaluate(() => localStorage.getItem('reset-calls'))).toBe('2')
})
