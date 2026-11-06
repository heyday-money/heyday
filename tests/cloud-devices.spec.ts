import { expect, test, type Page } from '@playwright/test'

test.use({ channel: 'chrome' })

async function setup(page: Page) {
  await page.addInitScript(() => {
    const stamp = '2026-11-04T06:30:00.000Z'
    let signedIn = true
    let current: { id: string; name: string; platform: string; createdAt: string; lastSeenAt: string; revokedAt: string | null; current: boolean } | null = null
    let revoked = false
    const other = { id: '11111111-1111-4111-8111-111111111111', name: 'Work laptop', platform: 'windows', createdAt: stamp, lastSeenAt: stamp, revokedAt: null, current: false }
    const old = { ...other, id: '22222222-2222-4222-8222-222222222222', name: 'Old computer', revokedAt: stamp }
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: Record<string, string | null> = {}) => {
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'plugin:app|version') return 'test'
      if (command === 'cloud_sync_status') return { bound: false, enabled: false, bootstrapped: false, pending: 0, conflictCount: 0, lastSuccess: null, conflicts: [] }
      if (command === 'cloud_account') return { account: signedIn ? { id: 'alice', email: 'alice@example.test' } : null, online: !sessionStorage.getItem('offline') }
      if (command === 'cloud_devices') {
        sessionStorage.setItem('device-requests', String(Number(sessionStorage.getItem('device-requests') ?? 0) + 1))
        if (sessionStorage.getItem('expired')) throw 'Your sign-in has expired or device access was revoked. Please sign in again.'
        if (sessionStorage.getItem('list-error')) throw 'Could not manage devices. Check your connection and try again.'
        return { currentDevice: current, savedName: sessionStorage.getItem('saved-name'), page: { devices: args.cursor ? [old] : [{ ...other, revokedAt: revoked ? stamp : null }], nextCursor: args.cursor ? null : other.id } }
      }
      if (command === 'cloud_register_device') {
        if (sessionStorage.getItem('register-error')) throw 'This installation was revoked. It cannot register again automatically. Your local finances remain available.'
        current = { ...other, id: '33333333-3333-4333-8333-333333333333', name: args.name!, platform: 'macos', current: true }
        return current
      }
      if (command === 'cloud_revoke_device') {
        if (sessionStorage.getItem('revoke-error')) throw 'Could not manage devices. Check your connection and try again.'
        const isCurrent = args.deviceId === current?.id
        if (isCurrent) signedIn = false
        else revoked = true
        sessionStorage.setItem('revoked-id', args.deviceId!)
        return { current: isCurrent, credentialsCleared: true }
      }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  })
  await page.goto('/#/settings')
  await expect(page.getByRole('link', { name: 'Account settings for alice@example.test', exact: true })).toBeVisible()
  await page.getByRole('tab', { name: 'Heyday account', exact: true }).click()
  await expect(page.getByText('alice@example.test', { exact: true })).toBeVisible()
}

test('registration preserves failed drafts, lists pages and requires confirmation to revoke', async ({ page }) => {
  await setup(page)
  expect(await page.evaluate(() => sessionStorage.getItem('device-requests'))).toBeNull()
  await page.getByRole('button', { name: 'Manage devices', exact: true }).click()
  const name = page.getByLabel('Device name', { exact: true })
  await name.fill('Personal Mac')
  await page.evaluate(() => sessionStorage.setItem('register-error', 'yes'))
  await page.getByRole('button', { name: 'Register this device', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('This installation was revoked')
  await expect(name).toHaveValue('Personal Mac')
  await page.evaluate(() => sessionStorage.removeItem('register-error'))
  await page.getByRole('button', { name: 'Register this device', exact: true }).click()
  await expect(page.getByRole('list', { name: 'Current device' })).toContainText('Personal Mac')
  await expect(page.getByText('This device', { exact: true })).toBeVisible()
  await expect(name).toHaveCount(0)
  await page.getByRole('button', { name: 'Load more devices', exact: true }).click()
  await expect(page.getByRole('list', { name: 'Other devices' })).toContainText('Old computer')
  await expect(page.getByRole('button', { name: 'Load more devices', exact: true })).toHaveCount(0)
  await page.screenshot({ path: 'test-results/cloud-devices.png', fullPage: true })
  await page.getByRole('button', { name: 'Revoke access for Work laptop', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('Financial data on that device is not erased.')
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('revoked-id'))).toBeNull()
  await page.getByRole('button', { name: 'Revoke access for Work laptop', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('revoke-error', 'yes'))
  await dialog.getByRole('button', { name: 'Revoke access', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not manage devices')
  await expect(dialog).toBeVisible()
  await page.evaluate(() => sessionStorage.removeItem('revoke-error'))
  await dialog.getByRole('button', { name: 'Revoke access', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Revoke access for Work laptop', exact: true })).toHaveCount(0)
  await expect(page.getByText('alice@example.test', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Revoke access for Personal Mac', exact: true }).click()
  await expect(dialog).toContainText('This will sign you out on this computer')
  await dialog.getByRole('button', { name: 'Revoke access', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeEnabled()
  await expect(page.getByText('Access revoked for this computer.', { exact: false })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Account settings for alice@example.test', exact: true })).toHaveCount(0)
  await expect(page.getByRole('list', { name: 'Current device' })).toHaveCount(0)
})

test('offline and expired sessions cannot act on stale device lists', async ({ page }) => {
  await setup(page)
  await page.getByRole('button', { name: 'Manage devices', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('offline', 'yes'))
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Refresh devices', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Revoke access for Work laptop', exact: true })).toBeDisabled()
  await page.evaluate(() => sessionStorage.removeItem('offline'))
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click()
  await page.evaluate(() => sessionStorage.setItem('expired', 'yes'))
  await page.getByRole('button', { name: 'Refresh devices', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeEnabled()
  await expect(page.getByRole('list', { name: 'Other devices' })).toHaveCount(0)
  await expect(page.getByText('Your sign-in has expired or device access was revoked. Please sign in again.')).toBeVisible()
  await expect(page.getByRole('link', { name: 'Account settings for alice@example.test', exact: true })).toHaveCount(0)
})

test('a new session reuses its saved device name', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => sessionStorage.setItem('saved-name', 'Previously registered Mac'))
  await page.getByRole('button', { name: 'Manage devices', exact: true }).click()
  await expect(page.getByLabel('Device name', { exact: true })).toHaveValue('Previously registered Mac')
  await expect(page.getByLabel('Device name', { exact: true })).toHaveAttribute('readonly', '')
  await page.getByRole('button', { name: 'Register this device', exact: true }).click()
  await expect(page.getByRole('list', { name: 'Current device' })).toContainText('Previously registered Mac')
})
