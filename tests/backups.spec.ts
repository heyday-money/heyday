import { test, expect } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: any) => {
      if (command === 'get_settings') return { currency: localStorage.getItem('restored') ? 'JPY' : 'THB', period_start_day: localStorage.getItem('restored') ? 28 : 25 }
      if (command === 'plugin:app|version') return '0.0.1-alpha.7'
      if (command === 'list_accounts' || command === 'list_incomes' || command === 'list_institutions') return []
      if (command === 'list_transaction_options') return { payees: [], categories: [] }
      if (command === 'get_credit_limit_groups') return { groups: [], cards: [] }
      if (command === 'export_backup') {
        sessionStorage.setItem('export-called', 'true')
        if (sessionStorage.getItem('export-fail')) throw 'Could not save the backup: permission denied.'
        return sessionStorage.getItem('picker-cancel') ? null : '/Backups/heyday.db'
      }
      if (command === 'preview_backup') {
        if (sessionStorage.getItem('picker-cancel')) return null
        if (sessionStorage.getItem('invalid')) throw 'This backup was created by a newer app. Update Heyday Money before restoring it.'
        return { token: 'selected-token', filename: 'heyday.db', schema_version: 36, accounts: 4, transactions: 50, currency: 'JPY' }
      }
      if (command === 'cancel_backup_restore') { sessionStorage.setItem('cancel-called', 'true'); return }
      if (command === 'restore_backup') {
        if (args.confirmation !== 'RESTORE BACKUP' || args.token !== 'selected-token') throw 'Invalid confirmation'
        sessionStorage.setItem('restore-called', JSON.stringify(args))
        if (sessionStorage.getItem('restore-fail')) throw 'Could not create the recovery folder. Restore stopped.'
        if (sessionStorage.getItem('restore-pause')) await new Promise<void>(resolve => { (window as any).finishRestore = resolve })
        localStorage.setItem('restored', 'true')
        return { recovery_path: '/ApplicationData/backups/before-restore.db' }
      }
      return []
    } } })
  })
  await page.goto('/#/settings')
})

test('exports backup, handles picker cancellation and errors without changing settings', async ({ page }) => {
  await page.getByRole('button', { name: 'Export backup', exact: true }).click()
  await expect(page.getByText('Backup exported.', { exact: true })).toBeVisible()
  await expect(page.getByText('Backup saved at: /Backups/heyday.db')).toBeVisible()
  await page.evaluate(() => sessionStorage.setItem('picker-cancel', 'true'))
  await page.getByRole('button', { name: 'Restore backup', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  expect(await page.evaluate(() => sessionStorage.getItem('restore-called'))).toBeNull()
  await page.evaluate(() => { sessionStorage.removeItem('picker-cancel'); sessionStorage.setItem('export-fail', 'true') })
  await page.getByRole('button', { name: 'Export backup', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('permission denied')
  await expect(page.getByLabel('Period start day')).toHaveValue('25')
})

test('validates before confirmation and cancels a checked backup without restoring', async ({ page }) => {
  await page.evaluate(() => sessionStorage.setItem('invalid', 'true'))
  await page.getByRole('button', { name: 'Restore backup', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('newer app')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.evaluate(() => sessionStorage.removeItem('invalid'))
  await page.getByRole('button', { name: 'Restore backup', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('4 accounts · 50 transactions · JPY')
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  expect(await page.evaluate(() => sessionStorage.getItem('cancel-called'))).toBe('true')
  expect(await page.evaluate(() => sessionStorage.getItem('restore-called'))).toBeNull()
})

test('requires exact confirmation, preserves data on failure, prevents busy dismissal, and reloads after success', async ({ page }) => {
  await page.getByRole('button', { name: 'Restore backup', exact: true }).click()
  const confirm = page.getByRole('button', { name: 'Replace data and restore', exact: true })
  await expect(confirm).toBeDisabled()
  await page.getByLabel('Type RESTORE BACKUP to confirm').fill('restore backup')
  await expect(confirm).toBeDisabled()
  await page.getByLabel('Type RESTORE BACKUP to confirm').fill('RESTORE BACKUP')
  await page.evaluate(() => sessionStorage.setItem('restore-fail', 'true'))
  await confirm.click()
  await expect(page.getByRole('alert')).toContainText('Restore stopped')
  await expect(page.getByLabel('Period start day')).toHaveValue('25')
  await expect(page.getByLabel('Type RESTORE BACKUP to confirm')).toHaveValue('RESTORE BACKUP')
  await page.evaluate(() => { sessionStorage.removeItem('restore-fail'); sessionStorage.setItem('restore-pause', 'true') })
  await confirm.click()
  await expect(page.getByRole('button', { name: 'Restoring…', exact: true })).toBeDisabled()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled()
  await page.evaluate(() => (window as any).finishRestore())
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByLabel('Period start day')).toHaveValue('28')
  await expect(page.getByText('Backup restored. Your previous data is saved in a recovery backup.')).toBeVisible()
  await expect(page.getByText('Backup saved at: /ApplicationData/backups/before-restore.db')).toBeVisible()
})

test('backup controls require saved settings and work in all appearances', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Restore backup', exact: true })).toBeEnabled()
  await page.getByLabel('Period start day').selectOption('28')
  await expect(page.getByRole('button', { name: 'Restore backup', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Export backup', exact: true })).toBeDisabled()
  await page.getByLabel('Period start day').selectOption('25')
  for (const appearance of ['Light', 'Dark', 'Heyday']) {
    await page.getByRole('radio', { name: appearance, exact: true }).check()
    await page.getByRole('button', { name: 'Restore backup', exact: true }).click()
    await expect(page.getByRole('dialog')).toBeVisible()
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  }
})
