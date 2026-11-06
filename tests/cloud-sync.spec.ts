import { expect, test, type Page } from '@playwright/test'
test.use({ channel: 'chrome' })
async function setup(page: Page, mode = 'new') {
  await page.clock.install()
  await page.addInitScript(({ mode }) => {
    const conflict = { recordId: 'test-category', local: { name: 'Dining local', icon: 'tag', isArchived: false }, remote: { name: 'Dining cloud', icon: 'utensils', isArchived: true }, localRevision: '9007199254740993', remoteVersion: '9007199254740995' }
    const account = { name: 'Savings local', type: 'bank', currency: 'THB', opening_balance: '9007199254740993', is_archived: false, institution: null, notes: 'Emergency fund', credit_limit: null, statement_day: null, payment_due_day: null, loan_type: null, last_four: '1234', interest_rate_ten_thousandths: null, monthly_installment: null, initial_loan_amount: null }
    const financial = { ...conflict, recordId: 'account:bank', entity: 'account', local: { name: account.name, icon: 'wallet', isArchived: false }, remote: { name: 'Savings cloud', icon: 'wallet', isArchived: true }, localDetails: account, remoteDetails: { ...account, name: 'Savings cloud', is_archived: true } }
    const income = { currency: 'THB', name: 'Salary local', type: 'salary', destination_account_id: 'bank', estimated_amount: '9007199254740993', recurrence_frequency: 'monthly', recurrence_day_of_month: 25, is_active: true, deductions: [{ id: 'tax', name: 'Tax withholding', description: 'Annual adjustment', amount: '100000', debt_account_id: null }] }
    const sourceConflict = { ...conflict, entity: 'income', recordId: 'income:salary', local: { name: income.name, icon: 'wallet', isArchived: false }, remote: { name: 'Salary cloud', icon: 'wallet', isArchived: false }, localDetails: income, remoteDetails: { ...income, name: 'Salary cloud', estimated_amount: '6000000' } }
    const payroll = { currency: 'THB', protected: true, entries: [], payroll: { id: 'payroll', income_id: 'salary', occurrence: '2026-10', date: '2026-10-25', gross: '100000', net: '0', breakdown: [{ id: 'tax', name: 'Tax withholding', amount: '100000', debt_account_id: null, debt_account_name: null, principal: '0', interest: '0', fee: '0', contract_id: null }], principal_entries: [] } }
    const payrollConflict = { ...conflict, entity: 'transaction', recordId: 'transaction:salary-payroll', local: null, remote: { name: '2026-10', icon: 'receipt', isArchived: false }, localDetails: null, remoteDetails: payroll }
    const forecast = { source_type: 'deduction', source_id: 'tax', month: '2026-11', currency: 'THB', amount: '9007199254740993' }
    const forecastConflict = { ...conflict, entity: 'income-plan', recordId: 'income-plan:deduction_2026-11_tax', local: { name: '2026-11', icon: 'calendar', isArchived: false }, remote: mode === 'forecast-clear' ? null : { name: '2026-11', icon: 'calendar', isArchived: false }, localDetails: forecast, remoteDetails: mode === 'forecast-clear' ? null : { ...forecast, amount: '0' } }
    const metadata = { kind: 'opening', fields: {id:'1',month:'2026-11',amount:'-9007199254740993'}, children:{} }
    const metadataConflict = {...conflict,entity:'metadata',recordId:'metadata:opening:1',local:{name:'2026-11',icon:'folder',isArchived:false},remote:{name:'2026-11',icon:'folder',isArchived:false},localDetails:metadata,remoteDetails:{...metadata,fields:{...metadata.fields,amount:'0'}}}
    let state = { fullEnabled: !['forecast-only', 'selected'].includes(mode), plannerEnabled: mode !== 'payroll-only', incomeEnabled: mode !== 'ledger', financialEnabled: mode !== 'legacy', bound: mode !== 'new', enabled: ['active', 'cached', 'selected', 'active-conflict'].includes(mode), bootstrapped: mode !== 'new', pending: mode === 'active' ? 1 : 0, conflictCount: ['conflict', 'active-conflict', 'financial', 'income', 'payroll', 'forecast', 'forecast-clear', 'metadata'].includes(mode) ? 1 : 0, lastSuccess: mode === 'cached' ? '2026-11-03T10:00:00.000Z' : null as string | null, conflicts: mode === 'metadata' ? [metadataConflict] : mode.startsWith('forecast') ? [forecastConflict] : mode === 'income' ? [sourceConflict] : mode === 'payroll' ? [payrollConflict] : mode === 'financial' ? [financial] : ['conflict', 'active-conflict'].includes(mode) ? [conflict] : [] }
    Object.defineProperty(window, 'isTauri', { value: true })
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string, args: Record<string, unknown> = {}) => {
      if (command === 'get_cashflow_planner') return { incomes: [{ id: 'salary', name: 'Salary' }], income_deductions: [{ id: 'tax', income_id: 'salary', name: 'Tax withholding' }] }
      if (command === 'list_accounts') return [{ id: 'bank', name: 'Bank' }, { id: 'cash', name: 'Cash' }]
      if (command === 'get_settings') return { currency: null, period_start_day: 1 }
      if (command === 'plugin:app|version') return 'test'
      if (command === 'cloud_account') return { account: { id: 'alice', email: 'alice@example.test' }, online: true }
      if (command === 'cloud_cancel_encryption') return
      if (command === 'cloud_sync_status') return state
      if (command === 'cloud_enable_sync') {
        if (args.confirmed !== true) throw 'Confirm that you want to sync categories with this account.'
        if (sessionStorage.getItem('access-ended')) throw 'Sync access has ended or has not been granted. Your cloud data is retained.'
        state = { ...state, bound: true, enabled: true, financialEnabled: true, incomeEnabled: true, plannerEnabled: true, fullEnabled: true, pending: 1 }; return state
      }
      if (command === 'cloud_sync_now') {
        sessionStorage.setItem('sync-calls', String(Number(sessionStorage.getItem('sync-calls') ?? 0) + 1))
        if (sessionStorage.getItem('hold-sync')) await new Promise<void>(resolve => window.addEventListener('finish-test-sync', () => resolve(), { once: true }))
        if (sessionStorage.getItem('access-ended')) throw 'Sync access has ended or has not been granted. Your cloud data is retained.'
        if (sessionStorage.getItem('expired')) throw 'Your sign-in has expired or device access was revoked. Please sign in again.'
        if (sessionStorage.getItem('rate-limited')) throw 'Too many sync requests. Try again in a minute.'
        if (sessionStorage.getItem('offline')) throw 'Could not sync. Check your connection and try again.'
        state = { ...state, bootstrapped: true, pending: 0, lastSuccess: '2026-11-04T10:00:00.000Z' }; return state
      }
      if (command === 'cloud_pause_sync') { state = { ...state, enabled: false }; return state }
      if (command === 'cloud_resolve_sync') {
        sessionStorage.setItem('resolution', JSON.stringify(args))
        if (sessionStorage.getItem('stale')) throw 'This sync conflict has changed. Refresh and try again.'
        state = { ...state, conflictCount: 0, conflicts: [], pending: args.choice === 'remote' ? 0 : 1 }; return state
      }
      throw new Error(`Unexpected command: ${command}`)
    } } })
  }, { mode })
  await page.goto('/#/settings?section=account')
  await expect(page.getByRole('heading', { name: 'Cloud sync', exact: true })).toBeVisible()
}
test('category sync requires confirmation and active access, preserves failed setup and pauses offline', async ({ page }) => {
  await setup(page)
  await expect(page.getByText('Cloud sync is not connected', { exact: true })).toBeVisible()
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBeNull()
  await page.getByRole('button', { name: 'Enable cloud sync', exact: true }).click()
  const dialog = page.getByRole('dialog')
  const enable = dialog.getByRole('button', { name: 'Enable cloud sync', exact: true })
  await expect(enable).toBeDisabled()
  await expect(dialog).toContainText('alice@example.test')
  await dialog.getByRole('checkbox').check()
  await page.evaluate(() => sessionStorage.setItem('access-ended', 'yes'))
  await enable.click()
  await expect(dialog.getByRole('alert')).toContainText('Your cloud data is retained')
  await expect(dialog.getByRole('checkbox')).toBeChecked()
  await page.evaluate(() => sessionStorage.removeItem('access-ended'))
  await enable.click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('1 change waiting to sync', { exact: true })).toBeVisible()
  await page.evaluate(() => sessionStorage.setItem('offline', 'yes'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Check your connection')
  await page.getByRole('button', { name: 'Pause sync', exact: true }).click()
  await expect(page.getByText('Cloud sync paused', { exact: true })).toBeVisible()
  await expect(page.getByText('1 change waiting to sync', { exact: true })).toBeVisible()
})
test('conflict choices preserve exact revisions, retain failed choices and support Thai', async ({ page }) => {
  await setup(page, 'conflict')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('Dining local')
  await expect(dialog).toContainText('Dining cloud')
  await expect(dialog.getByRole('img', { name: 'Icon: Food', exact: true })).toBeVisible()
  await page.evaluate(() => sessionStorage.setItem('stale', 'yes'))
  await dialog.getByRole('button', { name: 'Use cloud version', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('This sync conflict has changed')
  const sent = JSON.parse((await page.evaluate(() => sessionStorage.getItem('resolution')))!)
  expect(sent.localRevision).toBe('9007199254740993')
  expect(sent.remoteVersion).toBe('9007199254740995')
  await page.evaluate(() => {
    sessionStorage.removeItem('stale')
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  await expect(dialog).toHaveAccessibleName('แก้ไขข้อมูลซิงก์ที่ขัดแย้งกัน')
  await page.screenshot({ path: 'test-results/cloud-sync-conflict-thai.png', animations: 'disabled' })
  await dialog.getByRole('button', { name: 'ใช้ข้อมูลบนคลาวด์', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('list', { name: 'ข้อมูลซิงก์ที่ขัดแย้งกัน', exact: true })).toHaveCount(0)
})
test('automatic sync debounces edits, avoids download feedback loops and honors rate limits', async ({ page }) => {
  await setup(page, 'active')
  await page.clock.fastForward(1100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('transaction-options-changed', { detail: { source: 'cloud-sync' } })))
  await page.clock.fastForward(4000)
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(() => { window.dispatchEvent(new Event('transaction-options-changed')); window.dispatchEvent(new Event('transaction-options-changed')) })
  await page.clock.fastForward(3100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
  await page.getByRole('button', { name: 'Sync now', exact: true }).scrollIntoViewIfNeeded()
  await page.screenshot({ path: 'test-results/cloud-sync-ready.png', animations: 'disabled' })
  await page.evaluate(() => sessionStorage.setItem('rate-limited', 'yes'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Try again in a minute')
  await page.clock.fastForward(10000)
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('3')
  await page.evaluate(() => { sessionStorage.removeItem('rate-limited'); sessionStorage.setItem('expired', 'yes') })
  await page.clock.fastForward(61000)
  // The periodic retry discovers expiry and clears shared account state.
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Account settings for alice@example.test', exact: true })).toHaveCount(0)
})
test('expired access stops automatic retries and resumes only on a user retry', async ({ page }) => {
  await setup(page, 'active')
  await page.evaluate(() => sessionStorage.setItem('access-ended', 'yes'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Your cloud data is retained')
  await page.clock.fastForward(600_000)
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(() => sessionStorage.removeItem('access-ended'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(page.getByRole('status', { name: 'Sync status', exact: true })).toContainText('All changes synced')
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
})

test('an existing category connection requires fresh financial consent', async ({ page }) => {
  await setup(page, 'legacy')
  await expect(page.getByText('This connection currently syncs categories only. Confirm before adding accounts and transactions.')).toBeVisible()
  await page.getByRole('button', { name: 'Add accounts and transactions', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('complete financial records, planning data, reconciliation history')
  await expect(dialog.getByRole('button', { name: 'Enable cloud sync', exact: true })).toBeDisabled()
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBeNull()
  await dialog.getByRole('checkbox').check()
  await dialog.getByRole('button', { name: 'Enable cloud sync', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Add accounts and transactions', exact: true })).toHaveCount(0)
})
test('account conflict shows exact financial details and protects category-only actions', async ({ page }) => {
  await setup(page, 'financial')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('90,071,992,547,409.93 THB')
  await expect(dialog).toContainText('Emergency fund')
  await expect(dialog).toContainText('1234')
  await expect(dialog.getByRole('textbox')).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: 'Save cloud category with new name' })).toHaveCount(0)
  await page.screenshot({ path: 'test-results/cloud-sync-account-conflict.png', animations: 'disabled' })
  await dialog.getByRole('button', { name: 'Use cloud version', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  const args = JSON.parse((await page.evaluate(() => sessionStorage.getItem('resolution')))!)
  expect(args.recordId).toBe('account:bank'); expect(args.remoteVersion).toBe('9007199254740995')
})
test('account and transaction edits debounce together without sync feedback', async ({ page }) => {
  await setup(page, 'active')
  await page.clock.fastForward(1100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(() => { window.dispatchEvent(new Event('accounts-changed')); window.dispatchEvent(new Event('transactions-changed')) })
  await page.clock.fastForward(3100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
  await page.clock.fastForward(5000)
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
})


test('adding income and payroll to ledger sync requires explicit consent', async ({ page }) => {
  await setup(page, 'ledger')
  await page.getByRole('button', { name: 'Add income and payroll', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('complete financial records, planning data, reconciliation history')
  await expect(dialog.getByRole('button', { name: 'Enable cloud sync', exact: true })).toBeDisabled()
  await dialog.getByRole('checkbox').check()
  await dialog.getByRole('button', { name: 'Enable cloud sync', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Add income and payroll', exact: true })).toHaveCount(0)
})
test('income conflicts show exact gross amounts and deduction details', async ({ page }) => {
  await setup(page, 'income')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('90,071,992,547,409.93')
  await expect(dialog).toContainText('Tax withholding')
  await expect(dialog).toContainText('Annual adjustment')
  await expect(dialog.getByLabel('Optional new name for the cloud category')).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Use cloud version', exact: true }).click()
  expect(JSON.parse((await page.evaluate(() => sessionStorage.getItem('resolution')))!).recordId).toBe('income:salary')
})
test('zero-net payslip conflicts show the full breakdown and duplicate-month guidance', async ({ page }) => {
  await setup(page, 'payroll')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('2026-10-25')
  await expect(dialog).toContainText('Tax withholding')
  await expect(dialog).toContainText('Net salary deposited: 0.00 THB')
  await expect(dialog).toContainText('reverse the unwanted payment in Income')
  await expect(dialog).not.toContainText('manage deletion on its original device')
  await page.screenshot({ path: 'test-results/cloud-sync-payroll.png', animations: 'disabled' })
})


test('income forecasts require fresh consent for complete financial scope', async ({ page }) => {
  await setup(page, 'payroll-only')
  await page.getByText('What syncs', { exact: true }).click()
  await expect(page.getByText(/Appearance preferences, login credentials/)).toBeVisible()
  await page.getByRole('button', { name: 'Add income forecasts', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('payday settings, and custom logos')
  await expect(dialog.getByRole('button', { name: 'Enable cloud sync', exact: true })).toBeDisabled()
  await dialog.getByRole('checkbox').check()
  await dialog.getByRole('button', { name: 'Enable cloud sync', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Add income forecasts', exact: true })).toHaveCount(0)
})

test('forecast conflicts show source, exact amounts, zero and scheduled estimate clearing', async ({ page }) => {
  await setup(page, 'forecast')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('Salary · Tax withholding')
  await expect(dialog).toContainText('Forecast cycle: 2026-11')
  await expect(dialog).toContainText('90,071,992,547,409.93 THB')
  await expect(dialog).toContainText('Entered forecast: 0.00 THB')
  await expect(dialog).not.toContainText('Deleting transactions')
  await page.screenshot({ path: 'test-results/cloud-sync-forecast.png', animations: 'disabled' })
  await dialog.getByRole('button', { name: 'Use cloud version', exact: true }).click()
  expect(JSON.parse((await page.evaluate(() => sessionStorage.getItem('resolution')))!).recordId).toBe('income-plan:deduction_2026-11_tax')
})

test('clearing a forecast uses scheduled estimate wording and preserves failed choice', async ({ page }) => {
  await setup(page, 'forecast-clear')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('Clearing an override restores its scheduled estimate')
  await expect(dialog).not.toContainText('Deleted record')
  await page.evaluate(() => sessionStorage.setItem('stale', 'yes'))
  await dialog.getByRole('button', { name: 'Use scheduled estimate', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('This sync conflict has changed')
  await page.evaluate(() => sessionStorage.removeItem('stale'))
  await dialog.getByRole('button', { name: 'Use scheduled estimate', exact: true }).click()
  await expect(dialog).toHaveCount(0)
})

test('planner edits trigger background sync without download feedback', async ({ page }) => {
  await setup(page, 'active')
  await page.clock.fastForward(1100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(() => window.dispatchEvent(new Event('plans-changed')))
  await page.clock.fastForward(3100)
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('plans-changed', { detail: { source: 'cloud-sync' } })))
  await page.clock.fastForward(5000)
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('2')
})


test('complete financial sync requires renewed consent and retains cancellation', async ({page}) => {
  await setup(page,'forecast-only')
  await page.getByRole('button',{name:'Add remaining financial data',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await expect(dialog).toContainText('reconciliation history, payday settings, and custom logos')
  await expect(dialog.getByRole('button',{name:'Enable cloud sync',exact:true})).toBeDisabled()
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await expect(page.getByRole('button',{name:'Add remaining financial data',exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Add remaining financial data',exact:true}).click()
  await dialog.getByRole('checkbox').check()
  await dialog.getByRole('button',{name:'Enable cloud sync',exact:true}).click()
  await expect(page.getByRole('button',{name:'Add remaining financial data',exact:true})).toHaveCount(0)
})
test('complete metadata conflict displays signed exact amounts and resolves explicitly',async({page})=>{
  await setup(page,'metadata')
  await page.getByRole('button',{name:'Review conflict',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await expect(dialog).toContainText('Opening cash')
  await expect(dialog).toContainText('\u221290,071,992,547,409.93 THB')
  await expect(dialog).toContainText('0.00 THB')
  await expect(dialog.getByRole('textbox')).toHaveCount(0)
  await page.screenshot({path:'test-results/cloud-sync-complete-metadata.png',animations:'disabled'})
  await dialog.getByRole('button',{name:'Use cloud version',exact:true}).click()
  const args=JSON.parse((await page.evaluate(()=>sessionStorage.getItem('resolution')))!)
  expect(args.recordId).toBe('metadata:opening:1');expect(args.choice).toBe('remote')
})
test('complete metadata edits debounce without triggering download loops',async({page})=>{
  await setup(page,'active');await page.clock.fastForward(1100)
  await expect.poll(()=>page.evaluate(()=>sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.evaluate(()=>{for(const event of ['plans-changed','settings-changed','verification-changed','institutions-changed','subscription-providers-changed'])window.dispatchEvent(new Event(event))})
  await page.clock.fastForward(3100)
  await expect.poll(()=>page.evaluate(()=>sessionStorage.getItem('sync-calls'))).toBe('2')
  await page.clock.fastForward(5000);expect(await page.evaluate(()=>sessionStorage.getItem('sync-calls'))).toBe('2')
})

test('the green check clears immediately for edits, including edits made during a sync', async ({ page }) => {
  await setup(page, 'active')
  const card = page.locator('[data-sync-state]')
  const summary = page.getByRole('status', { name: 'Sync status', exact: true })
  await expect(card).toHaveAttribute('data-sync-state', 'pending')
  await page.clock.fastForward(1100)
  await expect(summary).toContainText('All changes synced')
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
  await expect(summary.locator('time')).toHaveAttribute('datetime', '2026-11-04T10:00:00.000Z')
  await expect(page.getByText(/Appearance preferences, login credentials/)).not.toBeVisible()
  const section = page.getByRole('region', { name: 'Cloud sync', exact: true })
  await section.scrollIntoViewIfNeeded()
  for (const theme of ['light', 'dark', 'heyday']) {
    await page.evaluate(theme => document.documentElement.setAttribute('data-theme', theme), theme)
    await section.screenshot({ path: `test-results/cloud-sync-${theme}.png`, animations: 'disabled' })
  }
  await page.setViewportSize({ width: 480, height: 850 })
  await section.screenshot({ path: 'test-results/cloud-sync-narrow.png', animations: 'disabled' })
  expect(await card.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.evaluate(() => {
    sessionStorage.setItem('hold-sync', 'yes')
    window.dispatchEvent(new Event('transactions-changed'))
  })
  await expect(card).toHaveAttribute('data-sync-state', 'pending')
  await expect(summary).not.toContainText('All changes synced')
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('1')
  await page.clock.fastForward(3100)
  await expect(card).toHaveAttribute('data-sync-state', 'syncing')
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled()
  await page.evaluate(() => {
    window.dispatchEvent(new Event('accounts-changed'))
    sessionStorage.removeItem('hold-sync')
    window.dispatchEvent(new Event('finish-test-sync'))
  })
  await expect(card).toHaveAttribute('data-sync-state', 'pending')
  await page.clock.fastForward(3100)
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
  expect(await page.evaluate(() => sessionStorage.getItem('sync-calls'))).toBe('3')
})

test('cached status and resuming sync cannot claim a completed upload', async ({ page }) => {
  await setup(page, 'cached')
  const card = page.locator('[data-sync-state]')
  await expect(card).toHaveAttribute('data-sync-state', 'waiting')
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
  await page.getByRole('button', { name: 'Pause sync', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'paused')
  await page.getByRole('button', { name: 'Resume sync', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(card).toHaveAttribute('data-sync-state', 'pending')
  await page.clock.fastForward(1100)
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
})

test('errors and going offline clear the green check until a successful sync', async ({ page, context }) => {
  await setup(page, 'active')
  const card = page.locator('[data-sync-state]')
  await page.clock.fastForward(1100)
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
  await page.evaluate(() => sessionStorage.setItem('offline', 'yes'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'error')
  await expect(page.getByRole('alert')).toContainText('Check your connection')
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'error')
  await page.evaluate(() => sessionStorage.removeItem('offline'))
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
  await context.setOffline(true)
  await expect(card).toHaveAttribute('data-sync-state', 'offline')
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled()
  await context.setOffline(false)
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
})

test('a successful request with conflicts cannot show completed sync', async ({ page }) => {
  await setup(page, 'active-conflict')
  const card = page.locator('[data-sync-state]')
  await page.clock.fastForward(1100)
  await expect(card).toHaveAttribute('data-sync-state', 'conflict')
  await page.getByRole('button', { name: 'Review conflict', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Use cloud version', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'waiting')
  await page.getByRole('button', { name: 'Sync now', exact: true }).click()
  await expect(card).toHaveAttribute('data-sync-state', 'synced')
})

test('limited consent and Thai sync status stay clear in a narrow layout', async ({ page }) => {
  await setup(page, 'selected')
  await page.clock.fastForward(1100)
  const summary = page.getByRole('status', { name: 'Sync status', exact: true })
  await expect(summary).toContainText('Selected data synced')
  await expect(summary).not.toContainText('All changes synced')
  await expect(page.getByRole('button', { name: 'Add remaining financial data', exact: true })).toBeVisible()
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  const thaiSummary = page.getByRole('status', { name: 'สถานะการซิงก์', exact: true })
  await expect(thaiSummary).toContainText('ซิงก์ข้อมูลที่เลือกแล้ว')
  await page.setViewportSize({ width: 480, height: 850 })
  const section = page.getByRole('region', { name: 'การซิงก์คลาวด์', exact: true })
  await section.screenshot({ path: 'test-results/cloud-sync-thai-narrow.png', animations: 'disabled' })
  expect(await section.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
})
