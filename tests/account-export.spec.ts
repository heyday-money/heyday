import { test, expect } from '@playwright/test'

test('Data exports Excel with cancellation, retry, and preserved General drafts', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 25 }
      if (command === 'plugin:app|version') return '0.0.1-beta.4'
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'get_credit_limit_groups') return { groups: [], cards: [] }
      if (command === 'export_accounts_excel') {
        sessionStorage.setItem('export-calls', String(Number(sessionStorage.getItem('export-calls') ?? 0) + 1))
        const mode = sessionStorage.getItem('export-mode')
        if (mode === 'cancel') return null
        if (mode === 'error') throw 'Permission denied'
        if (mode === 'pause') await new Promise<void>(resolve => { (window as any).finishExport = resolve })
        return '/Exports/accounts.xlsx'
      }
      return []
    } } })
  })
  await page.goto('/#/settings')
  await page.getByLabel('Period start day').selectOption('28')
  await page.getByRole('tab', { name: 'Data', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Restore backup', exact: true })).toBeDisabled()
  const button = page.getByRole('button', { name: 'Export accounts to Excel', exact: true })
  await page.evaluate(() => sessionStorage.setItem('export-mode', 'cancel'))
  await button.click()
  await expect(button).toBeEnabled()
  await expect(page.getByText('Accounts exported.', { exact: true })).toHaveCount(0)
  await page.evaluate(() => sessionStorage.setItem('export-mode', 'error'))
  await button.click()
  await expect(page.getByRole('alert')).toHaveText('Permission denied')
  await page.evaluate(() => sessionStorage.setItem('export-mode', 'pause'))
  await button.click()
  await expect(page.getByRole('button', { name: 'Exporting…', exact: true })).toBeDisabled()
  await page.evaluate(() => (window as any).finishExport())
  await expect(page.getByText('Accounts exported.', { exact: true })).toBeVisible()
  await expect(page.getByText('Export saved at: /Exports/accounts.xlsx')).toBeVisible()
  expect(await page.evaluate(() => sessionStorage.getItem('export-calls'))).toBe('3')
  await page.getByRole('tab', { name: 'General', exact: true }).click()
  await expect(page.getByLabel('Period start day')).toHaveValue('28')
})
