import { expect, test, type BrowserContext } from '@playwright/test'
import { readFileSync } from 'node:fs'

test.use({ channel: 'chrome' })

const url = 'http://127.0.0.1:1420/callback?code=test-secret-code&state=test-state'
const template = readFileSync(new URL('../src-tauri/assets/auth-callback.html', import.meta.url), 'utf8')
async function callbackPage(context: BrowserContext, result = 'accepted', language = 'en') {
  await context.route('**/callback?**', route => route.fulfill({
    status: result === 'accepted' ? 200 : 400,
    headers: {
      'content-type': 'text/html; charset=utf-8',
      'cache-control': 'no-store',
      'content-security-policy': "default-src 'none'; script-src 'nonce-test-nonce'; style-src 'nonce-test-nonce'; img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    },
    body: template.replaceAll('__NONCE__', 'test-nonce').replaceAll('__LANG__', language).replaceAll('__RESULT__', result),
  }))
}

test('blocked closing provides a clear fallback, removes query secrets and fits mobile', async ({ page, context }) => {
  await callbackPage(context)
  await page.addInitScript(() => {
    Object.defineProperty(window, 'close', { value: () => { document.documentElement.dataset.closeAttempts = String(Number(document.documentElement.dataset.closeAttempts ?? 0) + 1) } })
  })
  await page.clock.install()
  await page.goto(url)
  await expect(page.getByRole('heading', { name: 'Back to your Heyday.' })).toBeVisible()
  await expect(page).toHaveURL('http://127.0.0.1:1420/callback')
  await expect(page.getByText('The desktop app is finishing sign-in.', { exact: false })).toBeVisible()
  await page.clock.fastForward(2200)
  await expect(page.locator('html')).toHaveAttribute('data-close-attempts', '1')
  await expect(page.getByRole('status')).toContainText('Your browser kept this tab open')
  await page.getByRole('button', { name: 'Close this tab' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-close-attempts', '2')
  await page.screenshot({ path: 'test-results/auth-callback-light.png', fullPage: true })
  await page.setViewportSize({ width: 320, height: 740 })
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' })
  await page.clock.runFor(300)
  await expect(page.getByRole('button', { name: 'Close this tab' })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.setViewportSize({ width: 320, height: 900 })
  await page.clock.runFor(300)
  await page.screenshot({ path: 'test-results/auth-callback-mobile-dark.png' })
})

test('a script-opened callback tab closes automatically', async ({ page, context }) => {
  await callbackPage(context)
  await page.goto('http://127.0.0.1:1420')
  const opened = page.waitForEvent('popup')
  await page.evaluate(target => { window.open(target, '_blank') }, url)
  const popup = await opened
  const closed = popup.waitForEvent('close')
  await expect(popup.getByRole('heading', { name: 'Back to your Heyday.' })).toBeVisible()
  await closed
})

test('invalid callback shows Thai recovery copy and does not close automatically', async ({ page, context }) => {
  await callbackPage(context, 'invalid', 'th')
  await page.addInitScript(() => {
    Object.defineProperty(window, 'close', { value: () => { document.documentElement.dataset.closed = 'attempted' } })
  })
  await page.clock.install()
  await page.goto(url)
  await expect(page.getByRole('heading', { name: 'เข้าสู่ระบบต่อในแอป' })).toBeVisible()
  await expect(page.locator('#error-icon')).toBeVisible()
  await expect(page.locator('#success-icon')).toBeHidden()
  await page.clock.fastForward(3000)
  await expect(page.locator('html')).not.toHaveAttribute('data-closed', 'attempted')
  await page.getByRole('button', { name: 'ปิดแท็บนี้' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-closed', 'attempted')
})
