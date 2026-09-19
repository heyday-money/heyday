import { expect, test } from '@playwright/test'

test('period settings save, survive reload, and allow retry after a failure', async ({ page }) => {
  // Mock only the native transport; Rust tests exercise the real SQLite update.
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {
      invoke: async (command: string, args: { day?: number }) => {
        if (command === 'plugin:app|version') return '0.1.0'
        if (command === 'update_period') {
          if (sessionStorage.getItem('fail-save')) throw new Error('Save failed')
          localStorage.setItem('test-period', String(args.day))
        }
        if (command === 'get_settings' || command === 'update_period') {
          return { currency: null, period_start_day: Number(localStorage.getItem('test-period') ?? 1) }
        }
        throw new Error(`Unexpected command: ${command}`)
      },
    } })
  })
  await page.goto('/#/settings')
  await expect(page.getByText('Heyday Money · Version 0.1.0')).toBeVisible()
  const day = page.getByLabel('Period start day', { exact: true })
  await expect(day).toHaveValue('1')
  await day.selectOption('25')
  await page.getByRole('button', { name: 'Save period' }).click()
  await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Period saved.' })).toBeVisible()
  await page.reload()
  await expect(day).toHaveValue('25')
  await day.selectOption('31')
  await page.evaluate(() => sessionStorage.setItem('fail-save', 'true'))
  await page.getByRole('button', { name: 'Save period' }).click()
  await expect(page.getByRole('alert')).toContainText('Could not save')
  expect(await page.evaluate(() => localStorage.getItem('test-period'))).toBe('25')
  await page.evaluate(() => sessionStorage.removeItem('fail-save'))
  await page.getByRole('button', { name: 'Save period' }).click()
  await expect(page.locator('[data-sonner-toast]').filter({ hasText: 'Period saved.' })).toBeVisible()
  await page.reload()
  await expect(day).toHaveValue('31')
})
