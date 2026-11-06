import { expect, test, type Page } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'

const raw = JSON.parse(readFileSync(new URL('./fixtures/e2ee-v1.json', import.meta.url), 'utf8')) as { recoveryKey: string }
const checksum = createHash('sha256').update(Buffer.from(raw.recoveryKey, 'base64url')).digest('hex').slice(0, 8)
const fixture = { recoveryKey: `HDK1.${raw.recoveryKey}.${checksum}` }

test.use({ channel: 'chrome' })

async function setup(page: Page, initial = 'not_set') {
  // Public test vector only. Never exercise the user's OS credential store.
  await page.addInitScript(({ initial, recoveryKey }) => {
    let state = initial
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: Record<string, unknown> = {}) => {
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'plugin:app|version') return 'test'
      if (command === 'cloud_sync_status') return { bound: false, enabled: false, bootstrapped: false, pending: 0, conflictCount: 0, lastSuccess: null, conflicts: [] }
      if (command === 'cloud_account') return { account: { id: 'alice', email: 'alice@example.test' }, online: !sessionStorage.getItem('offline') }
      if (command === 'cloud_cancel_encryption') { sessionStorage.setItem('cancelled', 'yes'); return }
      if (command === 'cloud_encryption_status') {
        if (sessionStorage.getItem('expired')) throw 'Your sign-in has expired or device access was revoked. Please sign in again.'
        if (sessionStorage.getItem('unregistered')) throw 'Register this device in Manage devices before setting up encryption.'
        return { state }
      }
      if (command === 'cloud_prepare_encryption') return { recoveryKey }
      if (command === 'cloud_save_recovery_key') { sessionStorage.setItem('exported', 'yes'); return true }
      if (command === 'cloud_confirm_encryption' || command === 'cloud_recover_encryption') {
        if (args.recoveryKey !== recoveryKey) throw 'The recovery key is incorrect or the encrypted key package was changed.'
        if (command === 'cloud_confirm_encryption' && !args.saved) throw 'Save your recovery key and confirm that support cannot recover it.'
        if (sessionStorage.getItem('network-error')) { state = 'pending'; throw 'Could not reach encryption setup. Check your connection and try again.' }
        state = 'unlocked'; return { state }
      }
      if (command === 'cloud_pause_sync') {
        if (sessionStorage.getItem('pause-error')) throw 'Could not pause cloud sync.'
        sessionStorage.setItem('paused-before-key-removal', 'yes')
        return { bound: false, enabled: false, bootstrapped: false, pending: 0, conflictCount: 0, lastSuccess: null, conflicts: [] }
      }
      if (command === 'cloud_lock_encryption') {
        if (sessionStorage.getItem('paused-before-key-removal') !== 'yes') throw 'Sync must pause first'
        state = 'locked'; sessionStorage.setItem('key-removed', 'yes'); return
      }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  }, { initial, recoveryKey: fixture.recoveryKey })
  await page.goto('/#/settings')
  await page.getByRole('tab', { name: 'Heyday account', exact: true }).click()
  await expect(page.getByText('alice@example.test', { exact: true })).toBeVisible()
  await page.getByText('Advanced security', { exact: true }).click()
}

test('setup requires saved-key confirmation, preserves failures and clears visible secrets on success', async ({ page }) => {
  await setup(page)
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await page.getByRole('button', { name: 'Set up encryption', exact: true }).click()
  const dialog = page.getByRole('dialog')
  const submit = dialog.getByRole('button', { name: 'Confirm encryption setup', exact: true })
  await expect(dialog.getByLabel('Your recovery key', { exact: true })).toHaveValue(fixture.recoveryKey)
  await expect(submit).toBeDisabled()
  await dialog.getByRole('button', { name: 'Save recovery key file', exact: true }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('exported'))).toBe('yes')
  const entered = dialog.getByLabel('Re-enter your saved recovery key', { exact: true })
  await entered.fill('wrong-key')
  await expect(submit).toBeDisabled()
  await dialog.getByRole('checkbox', { name: 'I saved my recovery key and understand that support cannot recover my encrypted data.', exact: true }).check()
  await submit.click()
  await expect(dialog.getByRole('alert')).toContainText('recovery key is incorrect')
  await expect(entered).toHaveValue('wrong-key')
  await entered.fill(fixture.recoveryKey)
  await page.screenshot({ path: 'test-results/cloud-encryption-setup.png', fullPage: true, animations: 'disabled' })
  await submit.click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('End-to-end encrypted', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Enable cloud sync', exact: true })).toBeEnabled()
  await expect(page.locator('#generated-recovery')).toHaveCount(0)
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain(fixture.recoveryKey)
})

test('interrupted setup resumes the saved key after closing the setup screen', async ({ page }) => {
  await setup(page)
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await page.getByRole('button', { name: 'Set up encryption', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await dialog.getByLabel('Re-enter your saved recovery key', { exact: true }).fill(fixture.recoveryKey)
  await dialog.getByRole('checkbox', { name: 'I saved my recovery key and understand that support cannot recover my encrypted data.', exact: true }).check()
  await page.evaluate(() => sessionStorage.setItem('network-error', 'yes'))
  await dialog.getByRole('button', { name: 'Confirm encryption setup', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Check your connection')
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(page.locator('#generated-recovery')).toHaveCount(0)
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await page.getByRole('button', { name: 'Finish encryption setup', exact: true }).click()
  await expect(dialog.locator('#generated-recovery')).toHaveCount(0)
  await dialog.getByLabel('Recovery key', { exact: true }).fill(fixture.recoveryKey)
  await dialog.getByRole('checkbox', { name: 'I saved my recovery key and understand that support cannot recover my encrypted data.', exact: true }).check()
  await page.evaluate(() => sessionStorage.removeItem('network-error'))
  await dialog.getByRole('button', { name: 'Confirm encryption setup', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Remove saved encryption key', exact: true })).toBeEnabled()
})

test('recovery rejects a wrong key; key removal pauses first and works offline', async ({ page }) => {
  await setup(page, 'locked')
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await page.locator('section[aria-labelledby="cloud-sync-heading"]').screenshot({ path: 'test-results/cloud-encryption-locked.png', animations: 'disabled' })
  await page.getByRole('button', { name: 'Restore encryption key', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await dialog.getByLabel('Recovery key', { exact: true }).fill('wrong-key')
  await dialog.getByRole('button', { name: 'Restore encryption key', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('recovery key is incorrect')
  await dialog.getByLabel('Recovery key', { exact: true }).fill(fixture.recoveryKey)
  await dialog.getByRole('button', { name: 'Restore encryption key', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await page.locator('section[aria-labelledby="cloud-sync-heading"]').screenshot({ path: 'test-results/cloud-encryption-unlocked.png', animations: 'disabled' })
  await page.evaluate(() => sessionStorage.setItem('offline', 'yes'))
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Check encryption status', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Remove saved encryption key', exact: true }).click()
  await expect(dialog).toContainText('You will need your recovery key')
  await page.evaluate(() => sessionStorage.setItem('pause-error', 'yes'))
  await dialog.getByRole('button', { name: 'Remove saved encryption key', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Try again before removing')
  expect(await page.evaluate(() => sessionStorage.getItem('key-removed'))).toBeNull()
  await page.evaluate(() => sessionStorage.removeItem('pause-error'))
  await dialog.getByRole('button', { name: 'Remove saved encryption key', exact: true }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('key-removed'))).toBe('yes')
  await expect(page.getByRole('button', { name: 'Restore encryption key', exact: true })).toBeDisabled()
})

test('setup expires, supports Thai and requires registration and a valid session', async ({ page }) => {
  await setup(page)
  await page.clock.install()
  await page.evaluate(() => sessionStorage.setItem('unregistered', 'yes'))
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Register this device')
  await page.evaluate(() => sessionStorage.removeItem('unregistered'))
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await page.getByRole('button', { name: 'Set up encryption', exact: true }).click()
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  await expect(page.getByRole('dialog')).toHaveAccessibleName('บันทึกกุญแจกู้คืนของคุณ')
  await page.screenshot({ path: 'test-results/cloud-encryption-thai.png', fullPage: true, animations: 'disabled' })
  await page.clock.fastForward(600_001)
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByRole('alert')).toContainText('การตั้งค่าการเข้ารหัสหมดเวลา')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'en')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'en' }))
    sessionStorage.setItem('expired', 'yes')
  })
  await page.getByRole('button', { name: 'Check encryption status', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'End-to-end encryption', exact: true })).toHaveCount(0)
})
