import { expect, test, type Page } from '@playwright/test'

test.use({ channel: 'chrome' })

async function setup(page: Page, enabled = true, mode = 'none', bound = true) {
  await page.addInitScript(({ enabled, mode, bound }) => {
    let signedIn = true
    sessionStorage.setItem('subscription-mode', mode)
    const account = { id: 'alice', email: 'alice@example.test' }
    let sync = { bound, enabled, financialEnabled: true, incomeEnabled: true, plannerEnabled: true, fullEnabled: true, bootstrapped: true, pending: 2, conflictCount: 0, lastSuccess: null as string | null, conflicts: [] }
    const increment = (key: string) => sessionStorage.setItem(key, String(Number(sessionStorage.getItem(key) ?? 0) + 1))
    const status = () => {
      const mode = sessionStorage.getItem('subscription-mode')!
      const end = sessionStorage.getItem('access-until') ?? new Date(Date.now() + 86_400_000).toISOString()
      const allowed = ['trial', 'active', 'cancel', 'beta'].includes(mode)
      return { account: signedIn ? account : null, online: mode !== 'offline' && signedIn, deviceRegistered: signedIn,
        subscription: mode === 'offline' || mode === 'unknown' ? null : {
          entitlement: { canSync: allowed, reason: mode === 'disabled' ? 'not_configured' : mode === 'beta' ? 'beta_access' : allowed ? mode === 'trial' ? 'trial' : 'subscription' : 'premium_required', accessUntil: allowed ? end : null },
          billing: { enabled: true, subscriptions: ['none', 'beta', 'disabled'].includes(mode) ? [] : [{ status: mode === 'trial' ? 'trialing' : mode === 'payment' ? 'past_due' : mode === 'ended' ? 'canceled' : 'active', periodEnd: end, cancelAt: mode === 'cancel' ? end : null }] },
        },
      }
    }
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: unknown) => {
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'list_accounts') return []
      if (command === 'plugin:app|version') return 'test'
      if (command === 'cloud_account') {
        increment('account-checks')
        const result = status()
        if (sessionStorage.getItem('hold-account')) await new Promise<void>(resolve => window.addEventListener('finish-account', () => resolve(), { once: true }))
        return result
      }
      if (command === 'cloud_manage_subscription') { sessionStorage.setItem('billing-opened', 'yes'); sessionStorage.setItem('billing-args', JSON.stringify(args ?? null)); return }
      if (command === 'cloud_encryption_status') return { state: 'unlocked', rememberEncryption: true }
      if (command === 'cloud_cancel_encryption') return
      if (command === 'cloud_sync_status') return sync
      if (command === 'cloud_sync_now') {
        increment('sync-calls')
        const error = sessionStorage.getItem('sync-error')
        if (error) throw error
        sync = { ...sync, pending: 0, lastSuccess: new Date().toISOString() }; return sync
      }
      if (command === 'cloud_pause_sync') { sync = { ...sync, enabled: false }; return sync }
      if (command === 'cloud_enable_sync') { increment('enable-calls'); sync = { ...sync, enabled: true, bound: true }; return sync }
      if (command === 'cloud_sign_out') { signedIn = false; return true }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  }, { enabled, mode, bound })
  await page.goto('/#/settings?section=account')
  await expect(page.getByRole('heading', { name: 'Account subscription', exact: true })).toBeVisible()
}
async function refreshMode(page: Page, mode: string) {
  await page.evaluate(mode => sessionStorage.setItem('subscription-mode', mode), mode)
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click()
}

test('checkout return refreshes access and resumes existing sync; cancellation and payment failure retain data', async ({ page }) => {
  await setup(page)
  await expect(page.getByText('Choose a subscription', { exact: true })).toBeVisible()
  await expect(page.getByText('Sync is waiting for access', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled()
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBeNull()
  await page.getByRole('button', { name: 'Manage subscription on website' }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('billing-opened'))).toBe('yes')
  expect(await page.evaluate(() => sessionStorage.getItem('billing-args'))).toBe('{}')
  await page.evaluate(() => { sessionStorage.setItem('subscription-mode', 'trial'); window.dispatchEvent(new Event('focus')) })
  await expect(page.getByText('Trial active', { exact: true })).toBeVisible()
  await expect(page.getByText('All changes synced', { exact: true })).toBeVisible()
  expect(await page.evaluate(() => sessionStorage.getItem('enable-calls'))).toBeNull()
  await refreshMode(page, 'cancel')
  await expect(page.getByText('Cancellation scheduled', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeEnabled()
  await page.screenshot({ path: 'test-results/subscription-cancellation.png', fullPage: true })
  await refreshMode(page, 'payment')
  await expect(page.getByText('Payment needs attention', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: 'Pause sync', exact: true }).click()
  await refreshMode(page, 'active')
  await expect(page.getByText('Subscription active', { exact: true })).toBeVisible()
  await expect(page.getByText('Cloud sync paused', { exact: true })).toBeVisible()
  await page.evaluate(() => { localStorage.setItem('heyday-language', 'th'); window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' })) })
  await expect(page.getByText('สมาชิกใช้งานได้', { exact: true })).toBeVisible()
})

for (const bound of [false, true]) test(`subscription activation preserves ${bound ? 'paused sync' : 'first upload consent'}`, async ({ page }) => {
  await setup(page, false, 'none', bound)
  await refreshMode(page, 'trial')
  await expect(page.getByText('Trial active', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: bound ? 'Resume sync' : 'Enable cloud sync', exact: true })).toBeEnabled()
  expect(await page.evaluate(() => sessionStorage.getItem('enable-calls'))).toBeNull()
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBeNull()
  await page.getByRole('button', { name: bound ? 'Resume sync' : 'Enable cloud sync', exact: true }).click()
  if (bound) {
    await expect(page.getByRole('button', { name: 'Pause sync', exact: true })).toBeVisible()
    expect(await page.evaluate(() => sessionStorage.getItem('enable-calls'))).toBe('1')
  } else {
    await expect(page.getByRole('dialog')).toBeVisible()
    await expect(page.getByRole('dialog').getByRole('button', { name: 'Enable cloud sync', exact: true })).toBeDisabled()
  }
})

test('access expires while open even before the next server refresh', async ({ page }) => {
  await page.clock.install({ time: new Date('2026-11-06T00:00:00Z') })
  await page.addInitScript(() => sessionStorage.setItem('access-until', '2026-11-06T00:00:05Z'))
  await setup(page, true, 'trial')
  await expect(page.getByText('Trial active', { exact: true })).toBeVisible()
  await page.clock.fastForward(6000)
  await expect(page.getByText('Cloud sync access ended', { exact: true })).toBeVisible()
  await expect(page.getByText('2 changes waiting to sync', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled()
  await refreshMode(page, 'offline')
  await expect(page.getByText('Subscription status unavailable', { exact: true })).toBeVisible()
})

test('server denial refreshes subscription and successful renewal releases only the access block', async ({ page }) => {
  await page.clock.install()
  await setup(page, true, 'active')
  await page.evaluate(() => {
    sessionStorage.setItem('subscription-mode', 'payment')
    sessionStorage.setItem('sync-error', 'Sync access has ended or has not been granted. Your cloud data is retained.')
  })
  await page.clock.fastForward(1100)
  await expect(page.getByText('Payment needs attention', { exact: true })).toBeVisible()
  await page.evaluate(() => sessionStorage.removeItem('sync-error'))
  await refreshMode(page, 'active')
  await expect(page.getByText('All changes synced', { exact: true })).toBeVisible()
  await page.evaluate(() => sessionStorage.setItem('sync-error', 'Cloud history could not be verified. Sync stopped to protect your data.'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Cloud history could not be verified')
  const calls = await page.evaluate(() => sessionStorage.getItem('sync-calls'))
  await refreshMode(page, 'payment')
  await refreshMode(page, 'active')
  await page.clock.fastForward(2100)
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe(calls)
})

test('a delayed background account refresh cannot undo sign-out', async ({ page }) => {
  await setup(page, false, 'active')
  const before = Number(await page.evaluate(() => sessionStorage.getItem('account-checks')))
  await page.evaluate(() => { sessionStorage.setItem('hold-account', 'yes'); window.dispatchEvent(new Event('focus')) })
  await expect.poll(() => page.evaluate(() => Number(sessionStorage.getItem('account-checks')))).toBe(before + 1)
  await page.getByRole('button', { name: 'Sign out', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeVisible()
  await page.evaluate(() => window.dispatchEvent(new Event('finish-account')))
  await expect(page.getByRole('heading', { name: 'Account subscription', exact: true })).toHaveCount(0)
  await expect(page.getByRole('link', { name: 'Account settings for alice@example.test' })).toHaveCount(0)
})
