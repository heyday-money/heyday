import { expect, test } from '@playwright/test'

test.use({ channel: 'chrome' })

test('returning login restores remembered encryption; forgetting requires confirmation and recovery', async ({ page }) => {
  await page.addInitScript(() => {
    let signedIn = true, remembered = false, unlocked = true, registered = true, syncEnabled = true
    const account = { id: 'alice', email: 'alice@example.test' }
    const device = { id: '11111111-1111-4111-8111-111111111111', name: 'Personal Mac', platform: 'macos', createdAt: '2026-11-05T00:00:00Z', lastSeenAt: '2026-11-05T00:00:00Z', revokedAt: null, current: true }
    const status = () => ({ account: signedIn ? account : null, online: signedIn, deviceRegistered: signedIn && registered })
    const encryption = () => ({ state: unlocked ? 'unlocked' : 'locked', rememberEncryption: remembered })
    const syncState = () => ({ bound: true, enabled: syncEnabled, financialEnabled: true, incomeEnabled: true, plannerEnabled: true, fullEnabled: true, bootstrapped: true, pending: 0, conflictCount: 0, lastSuccess: '2026-11-05T00:00:00Z', conflicts: [] })
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: Record<string, unknown> = {}) => {
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'plugin:app|version') return 'test'
      if (command === 'cloud_account') return status()
      if (command === 'cloud_sign_in') { signedIn = true; return status() }
      if (command === 'cloud_sign_out') {
        if (sessionStorage.getItem('deny-signout')) throw 'Could not access the secure credential store. Please try again.'
        sessionStorage.setItem('forget-device', String(args.forgetDevice))
        if (args.forgetDevice) { remembered = false; registered = false }
        if (!remembered) unlocked = false
        signedIn = false; return true
      }
      if (command === 'cloud_cancel_encryption') return
      if (command === 'cloud_sync_status') return syncState()
      if (command === 'cloud_sync_now' || command === 'cloud_enable_sync') {
        if (!unlocked || !registered) throw 'Set up or unlock encryption before syncing.'
        if (command === 'cloud_enable_sync') syncEnabled = true
        return syncState()
      }
      if (command === 'cloud_pause_sync') { syncEnabled = false; return syncState() }
      if (command === 'cloud_encryption_status') return encryption()
      if (command === 'cloud_remember_encryption') {
        if (sessionStorage.getItem('deny-keychain')) throw 'Could not access the encryption key in the secure store. Please try again.'
        remembered = args.remember === true; return encryption()
      }
      if (command === 'cloud_devices') return { currentDevice: registered ? device : null, savedName: device.name, page: { devices: registered ? [device] : [], nextCursor: null } }
      if (command === 'cloud_register_device') { registered = true; return device }
      if (command === 'cloud_recover_encryption') { unlocked = true; remembered = args.remember === true; return encryption() }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  })
  await page.goto('/#/settings')
  await page.getByRole('tab', { name: 'Heyday account', exact: true }).click()
  await expect(page.getByText('End-to-end encrypted', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Remove saved encryption key', exact: true })).toBeHidden()
  await expect(page.getByRole('button', { name: /Lock encryption|Unlock encryption/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Pause sync', exact: true }).click()
  await expect(page.getByText('Cloud sync paused', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Resume sync', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByText('All changes synced', { exact: true })).toBeVisible()
  await page.screenshot({ path: 'test-results/cloud-simple-sync.png', animations: 'disabled' })
  await page.getByText('Advanced security', { exact: true }).click()
  const remember = page.getByRole('checkbox', { name: 'Remember encryption on this device', exact: true })
  await expect(remember).toBeVisible() // existing registered account loads status automatically
  await expect(remember).not.toBeChecked()
  await page.evaluate(() => sessionStorage.setItem('deny-keychain', 'yes'))
  await remember.click()
  await expect(page.getByRole('alert')).toContainText('secure store')
  await expect(remember).not.toBeChecked()
  await page.evaluate(() => sessionStorage.removeItem('deny-keychain'))
  await remember.check()
  await expect(remember).toBeChecked()
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
  expect(await page.evaluate(() => sessionStorage.getItem('forget-device'))).toBe('false')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await page.getByText('Advanced security', { exact: true }).click()
  await expect(remember).toBeChecked()
  await expect(page.getByRole('button', { name: 'Restore encryption key', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Manage devices', exact: true }).click()
  await expect(page.getByRole('list', { name: 'Current device' })).toContainText('Personal Mac')
  await expect(page.getByRole('button', { name: 'Register this device', exact: true })).toHaveCount(0)
  await page.screenshot({ path: 'test-results/cloud-returning-login.png', fullPage: true, animations: 'disabled' })
  await remember.scrollIntoViewIfNeeded()
  await page.screenshot({ path: 'test-results/cloud-remember-encryption.png', animations: 'disabled' })
  await page.getByRole('button', { name: 'Sign out and forget this device', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('enter your recovery key')
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(remember).toBeChecked()
  expect(await page.evaluate(() => sessionStorage.getItem('forget-device'))).toBe('false')
  await page.getByRole('button', { name: 'Sign out and forget this device', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('deny-signout', 'yes'))
  await dialog.getByRole('button', { name: 'Sign out and forget this device', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('secure credential store')
  await expect(page.getByText('alice@example.test', { exact: true })).toBeVisible()
  await page.evaluate(() => sessionStorage.removeItem('deny-signout'))
  await dialog.getByRole('button', { name: 'Sign out and forget this device', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
  expect(await page.evaluate(() => sessionStorage.getItem('forget-device'))).toBe('true')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await page.getByText('Advanced security', { exact: true }).click()
  await page.getByRole('button', { name: 'Manage devices', exact: true }).click()
  await page.getByRole('button', { name: 'Register this device', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Restore encryption key', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Set up encryption', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Restore encryption key', exact: true }).click()
  await dialog.getByLabel('Recovery key', { exact: true }).fill('public-test-key')
  await dialog.getByRole('checkbox', { name: 'Remember encryption on this device', exact: true }).uncheck()
  await dialog.getByRole('button', { name: 'Restore encryption key', exact: true }).click()
  await expect(remember).not.toBeChecked()
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await page.getByText('Advanced security', { exact: true }).click()
  await expect(page.getByRole('button', { name: 'Restore encryption key', exact: true })).toBeVisible()
})
