import { test, expect } from '@playwright/test'
import { validateManagementUrl } from '../src/lib/subscription-management'
import type { SaveSubscription, Subscription } from '../src/lib/desktop'

test.beforeEach(async ({ page }) => {
  await page.clock.install({ time: new Date('2026-01-15T12:00:00') })
  await page.addInitScript(() => {
    const accounts = [{ id: 'bank', name: 'Everyday bank', type: 'bank', current_balance: '100000' }, { id: 'card', name: 'Credit card', type: 'credit_card', current_balance: '5000' }, { id: 'loan', name: 'Loan', type: 'loan', current_balance: '5000' }].map(account => ({ ...account, opening_balance: account.current_balance, loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null, monthly_installment: null }))
    const rows = (): Subscription[] => JSON.parse(localStorage.getItem('subscriptions') ?? '[]')
    const settings = () => ({ currency: sessionStorage.getItem('no-currency') ? null : 'THB', period_start_day: 1 })
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: { input: SaveSubscription; id: string }) => {
      const active = sessionStorage.getItem('no-accounts') ? [] : accounts
      switch (command) {
        case 'plugin:app|version': return '0.1.0'
        case 'get_settings': return settings()
        case 'list_accounts': return active
        case 'list_incomes': return []
        case 'get_financial_data':
          if (sessionStorage.getItem('fail-read')) throw 'Load failed'
          return { settings: settings(), accounts: active, incomes: [], transactions: [], plans: [], installments: [], subscriptions: rows(), categories: [{ id: 'services', name: 'Services', is_archived: false }] }
        case 'save_subscription': {
          if (sessionStorage.getItem('fail-save')) throw 'Could not save subscription.'
          const { logo_change, ...input } = args.input, account = accounts.find(account => account.id === input.account_id)!
          const row = { ...rows().find(row => row.id === input.id), ...input, id: input.id ?? crypto.randomUUID(), account_name: account.name, account_type: account.type, category_name: input.category_id ? 'Services' : null }
          if (logo_change) {
            row.logo_asset_id = logo_change.kind === 'custom' ? crypto.randomUUID() : null
            if (logo_change.kind === 'custom' && row.logo_asset_id) localStorage.setItem(row.logo_asset_id, logo_change.data)
          }
          localStorage.setItem('subscriptions', JSON.stringify([...rows().filter(item => item.id !== row.id), row])); return
        }
        case 'get_logo_asset': return localStorage.getItem(args.id)
        case 'open_subscription_management':
          if (sessionStorage.getItem('fail-open')) throw 'Could not open the management link. Please try again.'
          sessionStorage.setItem('opened-subscription', args.id); return
        case 'delete_subscription': localStorage.setItem('subscriptions', JSON.stringify(rows().filter(row => row.id !== args.id))); return
        default: throw new Error(command)
      }
    } } })
  })
})
test('subscription navigation and create, reload, pause, resume, edit and remove workflow', async ({ page }) => {
  await page.goto('/#/installments')
  const nav = page.getByRole('navigation')
  const links = await nav.getByRole('link').evaluateAll(elements => elements.map(element => element.getAttribute('href')))
  const installmentsIndex = links.findIndex(link => link?.endsWith('/installments'))
  expect(installmentsIndex).toBeGreaterThanOrEqual(0)
  expect(links.findIndex(link => link?.endsWith('/subscriptions'))).toBe(installmentsIndex + 1)
  const link = nav.getByRole('link', { name: 'Subscriptions', exact: true })
  await expect(link).toHaveAttribute('title', 'Subscriptions')
  await link.click()
  await expect(page).toHaveURL(/#\/subscriptions$/)
  await expect(link).toHaveAttribute('aria-current', 'page')
  await expect(page.getByText('No subscriptions yet.', { exact: false })).toBeVisible()
  await page.getByRole('button', { name: 'Add subscription', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await page.getByLabel('Subscription name', { exact: true }).fill('Streaming')
  await page.getByLabel('Amount per charge (THB)').fill('100.01')
  await expect(page.getByLabel('Billing frequency')).toHaveValue('monthly')
  await expect(page.getByLabel('Pay from').getByRole('option', { name: 'Loan', exact: true })).toHaveCount(0)
  await page.getByLabel('Pay from').selectOption('bank')
  await page.getByLabel('First billing date').fill('2026-01-31')
  await page.getByLabel('Category (optional)').selectOption('services')
  await page.keyboard.press('Escape')
  await expect(dialog).toContainText('Discard unsaved subscription changes?')
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await page.evaluate(() => sessionStorage.setItem('fail-save', '1'))
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not save subscription')
  await expect(page.getByLabel('Amount per charge (THB)')).toHaveValue('100.01')
  await page.evaluate(() => sessionStorage.removeItem('fail-save'))
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  const table = page.getByRole('table', { name: 'Subscription plans', exact: true })
  await expect(table).toContainText('100.01 THB')
  await expect(table).toContainText('2026-01-31')
  await page.reload()
  await expect(table).toContainText('Streaming')
  async function forecast(text: string) {
    await nav.getByRole('link', { name: 'Outlook', exact: true }).click()
  await page.getByRole('tab', { name: 'Account-Based Outlook', exact: true }).click()
    const expenses = page.getByRole('table', { name: /Cash outlook/ }).getByRole('row').filter({ has: page.getByRole('button', { name: 'Cash expenses', exact: true }) })
    await expect(expenses.getByRole('cell').first()).toContainText(text)
    await nav.getByRole('link', { name: 'Subscriptions', exact: true }).click()
  }
  await forecast('100.01 THB')
  await page.getByRole('button', { name: 'Edit subscription Streaming' }).click()
  await page.getByLabel('Subscription status').selectOption('false')
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(table).toContainText('Paused')
  await forecast('Not planned')
  await page.getByRole('button', { name: 'Edit subscription Streaming' }).click()
  await page.getByLabel('Subscription status').selectOption('true')
  await page.getByLabel('Amount per charge (THB)').fill('200')
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await forecast('200.00 THB')
  await page.getByRole('button', { name: 'Add subscription', exact: true }).click()
  await page.getByLabel('Subscription name').fill('Annual storage')
  await page.getByLabel('Amount per charge (THB)').fill('1200')
  await page.getByLabel('Billing frequency').selectOption('yearly')
  await page.getByLabel('Pay from').selectOption('card')
  await page.getByLabel('First billing date').fill('2026-01-20')
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(table).toContainText('Yearly')
  await expect(table).toContainText('Credit card · excluded from cash forecast')
  await forecast('200.00 THB')
  await page.setViewportSize({ width: 650, height: 700 })
  expect(await page.locator('main').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.getByRole('button', { name: 'Remove subscription Streaming' }).click()
  await page.getByRole('button', { name: 'Cancel', exact: true }).click()
  await expect(table.getByRole('rowheader', { name: 'Streaming', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Remove subscription Streaming' }).click()
  await page.getByRole('button', { name: 'Remove subscription', exact: true }).click()
  await expect(table.locator('tbody tr')).toHaveCount(1)
  await forecast('Not planned')
})
test('subscriptions handle missing prerequisites, failed loads and ended schedules', async ({ page }) => {
  await page.goto('/#/subscriptions')
  await page.evaluate(() => sessionStorage.setItem('fail-read', '1'))
  await page.reload()
  await expect(page.getByRole('alert')).toContainText('Could not load subscriptions')
  await page.evaluate(() => sessionStorage.removeItem('fail-read'))
  await page.getByRole('button', { name: 'Retry subscriptions' }).click()
  await expect(page.getByRole('button', { name: 'Add subscription', exact: true })).toBeEnabled()
  await page.evaluate(() => sessionStorage.setItem('no-currency', '1'))
  await page.reload()
  await expect(page.getByText('Choose your currency in', { exact: false })).toBeVisible()
  await page.evaluate(() => { sessionStorage.removeItem('no-currency'); sessionStorage.setItem('no-accounts', '1') })
  await page.reload()
  await expect(page.getByRole('button', { name: 'Add subscription', exact: true })).toBeDisabled()
  await page.evaluate(() => {
    sessionStorage.removeItem('no-accounts')
    localStorage.setItem('subscriptions', JSON.stringify([{ id: 'ended', name: 'Old service', account_id: 'bank', account_name: 'Everyday bank', account_type: 'bank', category_id: null, category_name: null, amount: '10000', frequency: 'monthly', first_billing_date: '2025-01-31', end_date: '2025-12-31', is_active: true }]))
  })
  await page.reload()
  await expect(page.getByRole('table', { name: 'Subscription plans' })).toContainText('Schedule ended')
})


test('service icons and management destinations persist with protected drafts', async ({ page }) => {
  await page.goto('/#/subscriptions')
  await page.getByRole('button', { name: 'Add subscription', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await page.getByLabel('Subscription name', { exact: true }).fill('Music service')
  await page.getByLabel('Amount per charge (THB)').fill('199')
  await page.getByLabel('Pay from').selectOption('bank')
  await page.getByLabel('Managed through').selectOption('apple_app_store')
  await page.getByLabel('Management link (optional)').fill('https://example.com/subscriptions')
  const encoded = await page.evaluate(() => {
    const canvas = document.createElement('canvas'); canvas.width = 128; canvas.height = 128
    const context = canvas.getContext('2d')!; context.fillStyle = '#743ae8'; context.fillRect(0, 0, 128, 128)
    return canvas.toDataURL().split(',')[1]
  })
  await page.getByLabel('Choose logo file').setInputFiles({ name: 'service.png', mimeType: 'image/png', buffer: Buffer.from(encoded, 'base64') })
  await expect(dialog.locator('img')).toHaveAttribute('src', /^data:image\/png;base64,/)
  await page.evaluate(() => sessionStorage.setItem('fail-save', '1'))
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not save subscription.')
  await expect(page.getByLabel('Managed through')).toHaveValue('apple_app_store')
  await page.keyboard.press('Escape')
  await expect(dialog).toContainText('Discard unsaved subscription changes?')
  await page.getByRole('button', { name: 'Keep editing' }).click()
  await page.evaluate(() => sessionStorage.removeItem('fail-save'))
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await page.reload()
  const row = page.getByRole('row').filter({ hasText: 'Music service' })
  await expect(row).toContainText('Apple App Store')
  await expect(row.locator('img')).toHaveAttribute('src', /^data:image\/png;base64,/)
  await row.getByRole('button', { name: 'Manage subscription Music service' }).click()
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('subscriptions')!)[0])
  expect(await page.evaluate(() => sessionStorage.getItem('opened-subscription'))).toBe(saved.id)
  await page.evaluate(() => sessionStorage.setItem('fail-open', '1'))
  await row.getByRole('button', { name: 'Manage subscription Music service' }).click()
  await expect(page.getByText('Could not open the management link. Please try again.', { exact: true })).toBeVisible()
  await row.getByRole('button', { name: 'Edit subscription Music service' }).click()
  await expect(page.getByLabel('Management link (optional)')).toHaveValue('https://example.com/subscriptions')
  await page.getByLabel('Managed through').selectOption('in_app')
  await page.getByLabel('Management link (optional)').fill('http://example.com/settings')
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('Enter an HTTPS management link')
  await page.getByLabel('Management link (optional)').fill('')
  await page.getByRole('button', { name: 'Remove logo', exact: true }).click()
  await page.getByRole('button', { name: 'Save subscription', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(row).toContainText('In-app')
  await expect(row.locator('img')).toHaveCount(0)
  await expect(row.getByRole('button', { name: 'Manage subscription Music service' })).toHaveCount(0)
})

test('management URL validation only accepts HTTPS browser destinations', () => {
  for (const url of ['file:///tmp/file', 'javascript:alert(1)', 'http://example.com', 'https://user:pass@example.com', 'https://user@example.com', 'bad']) expect(() => validateManagementUrl(url)).toThrow()
  expect(validateManagementUrl(' https://example.com/settings ')).toBe('https://example.com/settings')
  expect(validateManagementUrl('')).toBeNull()
})
