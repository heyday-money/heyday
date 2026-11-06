import { expect, test } from '@playwright/test'

test.use({ channel: 'chrome' })

test('beta account supports email login, cancellation, offline status and logout', async ({ page }) => {
  await page.addInitScript(() => {
    let rejectLogin: ((reason: string) => void) | undefined
    let account: { id: string; email: string } | null = null
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'list_accounts') return []
      if (command === 'plugin:app|version') return '0.0.1-beta.4'
      if (command === 'cloud_account') {
        sessionStorage.setItem('account-checks', String(Number(sessionStorage.getItem('account-checks') ?? 0) + 1))
        await new Promise(resolve => setTimeout(resolve, 100))
        if (sessionStorage.getItem('deny-keychain')) throw 'Could not access the secure credential store. Please try again.'
        return { account, online: false }
      }
      if (command === 'cloud_sign_in') {
        if (sessionStorage.getItem('complete-login')) {
          account = { id: 'test-user', email: 'beta@example.test' }
          return { account, online: true }
        }
        return new Promise((_, reject) => { rejectLogin = reject })
      }
      if (command === 'cloud_cancel_sign_in') { rejectLogin?.('Sign-in cancelled.'); return }
      if (command === 'cloud_cancel_encryption') return
      if (command === 'cloud_manage_subscription') { sessionStorage.setItem('billing-opened', 'yes'); return }
      if (command === 'cloud_sync_status') return { bound: false, enabled: false, bootstrapped: false, pending: 0, conflictCount: 0, lastSuccess: null, conflicts: [] }
      if (command === 'cloud_sign_out') {
        if (sessionStorage.getItem('deny-keychain')) throw 'Could not access the secure credential store. Please try again.'
        account = null; return false
      }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  })
  await page.goto('/#/settings')
  await page.getByRole('tab', { name: 'Heyday account', exact: true }).click()
  const signIn = page.getByRole('button', { name: 'Sign in with email', exact: true })
  const avatar = page.getByRole('link', { name: 'Account settings for beta@example.test', exact: true })
  await expect(signIn).toBeEnabled()
  await expect(avatar).toHaveCount(0)
  expect(await page.evaluate(() => sessionStorage.getItem('account-checks'))).toBe('1')
  await page.evaluate(() => sessionStorage.setItem('deny-keychain', 'yes'))
  await page.getByRole('button', { name: 'Refresh status' }).click()
  await expect(page.getByRole('alert')).toHaveText('Could not access the secure credential store. Please try again.')
  await page.evaluate(() => sessionStorage.removeItem('deny-keychain'))
  await page.getByRole('button', { name: 'Refresh status' }).click()
  await expect(signIn).toBeEnabled()
  await expect(page.getByRole('alert')).toHaveCount(0)
  await expect(page.getByRole('button', { name: /Google|Apple/ })).toHaveCount(0)
  await signIn.click()
  await expect(page.getByRole('status').filter({ hasText: 'Finish signing in' })).toBeVisible()
  await page.getByRole('button', { name: 'Cancel sign-in', exact: true }).click()
  await expect(signIn).toBeEnabled()
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.evaluate(() => sessionStorage.setItem('complete-login', 'yes'))
  await signIn.click()
  await expect(page.getByText('beta@example.test', { exact: true })).toBeVisible()
  await expect(avatar).toBeVisible()
  await expect(avatar).toHaveText('B')
  await page.getByRole('button', { name: 'Manage subscription on website' }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('billing-opened'))).toBe('yes')
  const settings = page.getByRole('navigation').getByRole('link', { name: 'Settings', exact: true })
  expect((await avatar.boundingBox())!.y + (await avatar.boundingBox())!.height).toBeLessThan((await settings.boundingBox())!.y)
  await page.screenshot({ path: 'test-results/sidebar-avatar-collapsed.png', animations: 'disabled' })
  await settings.click()
  await expect(page.getByRole('tab', { name: 'General', exact: true })).toHaveAttribute('aria-selected', 'true')
  await page.getByLabel('Period start day', { exact: true }).selectOption('25')
  const checks = await page.evaluate(() => sessionStorage.getItem('account-checks'))
  await avatar.click()
  await expect(page.getByRole('tab', { name: 'Heyday account', exact: true })).toHaveAttribute('aria-selected', 'true')
  expect(await page.evaluate(() => sessionStorage.getItem('account-checks'))).toBe(checks)
  await settings.click()
  await expect(page.getByLabel('Period start day', { exact: true })).toHaveValue('25')
  await avatar.click()
  await page.getByRole('button', { name: 'Expand sidebar', exact: true }).click()
  await expect(avatar).toContainText('beta@example.test')
  expect(await page.getByRole('complementary').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
  await page.screenshot({ path: 'test-results/sidebar-avatar-expanded.png', animations: 'disabled' })
  await page.getByRole('button', { name: 'Refresh status' }).click()
  await expect(page.getByText('Offline or session unavailable.', { exact: false })).toBeVisible()
  await expect(avatar).toBeVisible()
  await page.evaluate(() => sessionStorage.setItem('deny-keychain', 'yes'))
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await expect(page.getByRole('alert')).toHaveText('Could not access the secure credential store. Please try again.')
  await expect(avatar).toBeVisible()
  await page.evaluate(() => sessionStorage.removeItem('deny-keychain'))
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await expect(signIn).toBeEnabled()
  await expect(avatar).toHaveCount(0)
  await expect(page.getByText('Signed out on this device.', { exact: false })).toBeVisible()
  await expect(page.getByText('Signing in does not start cloud sync.', { exact: false })).toBeVisible()
})
