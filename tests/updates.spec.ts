import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'get_settings') return { currency: 'THB', period_start_day: 25 }
      if (command === 'plugin:app|version') return '0.0.1-alpha.1'
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'check_app_update') {
        const state = sessionStorage.getItem('update-state')
        if (state === 'offline') throw 'Could not reach GitHub. Check your internet connection and try again.'
        return { current_version: '0.0.1-alpha.1', version: state === 'current' ? null : '0.0.1-alpha.2', notes: 'New features and fixes.', can_install: state !== 'manual', message: state === 'current' ? 'You’re using the latest available version for your release channel.' : state === 'manual' ? 'Install its DMG manually.' : 'Update available. Save your work before installing.' }
      }
      if (command === 'install_app_update') { sessionStorage.setItem('install-called', 'true'); throw 'Could not install update. Your database backup is retained.' }
      return []
    } } })
  })
  await page.goto('/#/settings')
})

test('checks updates, requires confirmation and permits retry after failed installation', async ({ page }) => {
  await page.getByRole('button', { name: 'Check for Updates' }).click()
  await expect(page.getByText('Version 0.0.1-alpha.2 is available')).toBeVisible()
  await page.getByRole('button', { name: 'Download and Install' }).click()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('install-called'))).toBeNull()
  await page.getByRole('button', { name: 'Download and Install' }).click()
  await page.getByRole('button', { name: 'Install and Restart' }).click()
  await expect(page.getByRole('alert')).toContainText('backup is retained')
  await expect(page.getByRole('button', { name: 'Install and Restart' })).toBeEnabled()
})

test('handles offline, current version and manual-only releases', async ({ page }) => {
  await page.evaluate(() => sessionStorage.setItem('update-state', 'offline'))
  await page.getByRole('button', { name: 'Check for Updates' }).click()
  await expect(page.getByRole('alert')).toContainText('Could not reach GitHub')
  await page.evaluate(() => sessionStorage.setItem('update-state', 'current'))
  await page.getByRole('button', { name: 'Check for Updates' }).click()
  await expect(page.getByText('You’re using the latest available version for your release channel.')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Download and Install' })).toHaveCount(0)
  await page.evaluate(() => sessionStorage.setItem('update-state', 'manual'))
  await page.getByRole('button', { name: 'Check for Updates' }).click()
  await expect(page.getByText('Install its DMG manually.')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Download and Install' })).toHaveCount(0)
})
