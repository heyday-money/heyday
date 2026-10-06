import { test, expect } from '@playwright/test'
import { readFileSync, readdirSync } from 'node:fs'
import ts from 'typescript'
import { thai } from '../src/lib/locales/th'
import { getLanguage, getLocale, setLanguage, t } from '../src/lib/i18n'
import { formatAmount } from '../src/lib/money'
import { periodLabel } from '../src/lib/period'

function sources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => entry.isDirectory() ? sources(`${directory}/${entry.name}`) : /\.tsx?$/.test(entry.name) ? [`${directory}/${entry.name}`] : [])
}
test('every literal translation has Thai copy with matching interpolation parameters', () => {
  let count = 0
  for (const file of sources('src').filter(file => !file.includes('/locales/'))) {
    const ast = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, file.endsWith('tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS)
    function visit(node: ts.Node) {
      if (ts.isCallExpression(node) && ['t', 'translate'].includes(node.expression.getText(ast)) && node.arguments[0] && ts.isStringLiteral(node.arguments[0])) {
        const key = node.arguments[0].text
        expect(thai[key], `${file}: ${key}`).toBeDefined()
        expect([...thai[key].matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort(), key).toEqual([...key.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort())
        count++
      }
      ts.forEachChild(node, visit)
    }
    visit(ast)
  }
  expect(count).toBeGreaterThan(1000)
})

test('Thai uses Gregorian dates and Latin digits without losing monetary precision', () => {
  try {
    setLanguage('th')
    expect(getLanguage()).toBe('th')
    expect(getLocale()).toContain('ca-gregory')
    expect(periodLabel(25, new Date(2026, 9, 2))).toContain('2026')
    expect(periodLabel(25, new Date(2026, 9, 2))).not.toContain('2569')
    expect(formatAmount('9007199254740993', 'THB').replaceAll(',', '')).toBe('90071992547409.93 THB')
    expect(formatAmount('12345', 'JPY')).toBe('12,345 JPY')
    expect(formatAmount('12345', 'KWD')).toBe('12.345 KWD')
    expect(t('Enter a valid amount with up to 3 decimal places.')).toContain('3')
    expect(t('Unknown native diagnostic')).toBe('Unknown native diagnostic')
    expect(t('toString')).toBe('toString')
    expect(t('Edit account {value0}', { value0: '<Savings & เงินออม>' })).toBe('แก้ไขบัญชี <Savings & เงินออม>')
  } finally { setLanguage('en') }
})

async function mockDesktop(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const account = { id: 'bank', name: 'Home', type: 'bank', current_balance: '9007199254740993', opening_balance: '9007199254740993', loan_type: null, institution: null, last_four: null, notes: null, credit_limit: null, statement_day: null, payment_due_day: null, interest_rate_ten_thousandths: null, monthly_installment: null }
    const settings = { currency: 'THB', period_start_day: 1 }
    const categories = [['income','Gross Income','Total Gross Income'],['deductions','Income Deductions','Total Deductions'],['debt','Debt Payments','Total Debt Payments'],['installments','Card Installments','Total Card Installments'],['cards','Credit Cards','Total Card Payments'],['expenses','General Expenses','Total General Expenses']].map(([id,name,subtotal]) => ({id,name,subtotal}))
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: { invoke: async (command: string) => {
      switch (command) {
        case 'plugin:app|version': return 'test'
        case 'get_settings': return settings
        case 'list_accounts': return [account]
        case 'list_institutions': return { institutions: [], accounts: [] }
        case 'list_card_limit_groups': return { currency: 'THB', groups: [], cards: [] }
        case 'list_transaction_options': return { payees: [], categories: [] }
        case 'list_transactions': return []
        case 'list_incomes': return []
        case 'get_expense_report': return { settings, accounts: [account], transactions: [] }
        case 'get_card_overview': return { currency: 'THB', accounts: [], archived_ids: [], transactions: [], billing: { statements: [], allocations: [], installments: [], plans: [] } }
        case 'get_cashflow_planner': return { categories, items: [], months: [], amounts: [], period_start_day: 1, source_currency: 'THB', opening: null, expense_categories: [], incomes: [], income_deductions: [], debt_accounts: [], installments: [], credit_cards: [], card_transactions: [], ledger_transactions: [] }
        case 'get_financial_data': return { settings, accounts: [account], incomes: [], transactions: [], categories: [], plans: [], installments: [], subscriptions: [] }
        case 'set_account_archived': throw 'Account archive status has changed. Reload and try again.'
        default: throw new Error(`Unexpected command: ${command}`)
      }
    } } })
  })
}

test('language switches immediately, preserves settings draft, survives restart and translates account warnings', async ({ page }) => {
  await mockDesktop(page)
  await page.goto('/#/settings')
  await page.getByLabel('Period start day', { exact: true }).selectOption('25')
  await page.getByLabel('Language', { exact: true }).selectOption('th')
  await expect(page.locator('html')).toHaveAttribute('lang', 'th')
  await expect(page.getByLabel('วันเริ่มรอบ', { exact: true })).toHaveValue('25')
  await expect(page.getByRole('button', { name: 'บันทึกรอบ', exact: true })).toBeEnabled()
  await page.screenshot({ path: '/tmp/heyday-th-settings.png', fullPage: true })
  await page.getByRole('tab', { name: 'ผู้รับเงิน', exact: true }).click()
  await page.getByRole('tab', { name: 'ทั่วไป', exact: true }).click()
  await expect(page.getByLabel('วันเริ่มรอบ', { exact: true })).toHaveValue('25')
  await page.reload()
  await expect(page.getByLabel('ภาษา', { exact: true })).toHaveValue('th')
  await page.getByRole('navigation').getByRole('link', { name: 'บัญชี', exact: true }).click()
  await expect(page.getByText('Home', { exact: true })).toBeVisible()
  await expect(page.getByRole('columnheader', { name: 'ยอดคงเหลือ', exact: true })).toBeVisible()
  await page.getByRole('link', { name: 'Home', exact: true }).click()
  await page.getByRole('button', { name: 'เก็บถาวร บัญชี Home', exact: true }).click()
  await expect(page.getByRole('dialog')).toContainText('การเก็บถาวรนำยอดนี้ออกจากมูลค่าสุทธิ')
  await page.getByRole('dialog').getByRole('button', { name: 'เก็บบัญชีถาวร', exact: true }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('สถานะเก็บถาวรเปลี่ยนแล้ว')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'en')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'en' }))
  })
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Account archive status has changed.')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click()
  await page.getByRole('navigation').getByRole('link', { name: 'การตั้งค่า', exact: true }).click()
  await page.getByLabel('ภาษา', { exact: true }).selectOption('en')
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
  await expect(page.getByLabel('Period start day', { exact: true })).toBeVisible()
})

test('switching language keeps open transaction drafts and account form values intact', async ({ page }) => {
  await mockDesktop(page)
  await page.goto('/#/accounts')
  await page.getByRole('button', { name: 'Add account', exact: true }).click()
  await page.getByLabel('Account name', { exact: true }).fill('เงินออม Savings')
  await page.getByLabel('Opening balance (THB)', { exact: true }).fill('1234.56')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  await expect(page.getByLabel('ชื่อบัญชี', { exact: true })).toHaveValue('เงินออม Savings')
  await expect(page.getByLabel('ยอดเริ่มต้น (THB)', { exact: true })).toHaveValue('1234.56')
  await expect(page.getByRole('dialog')).toHaveAccessibleName('เพิ่มบัญชี')
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'ละทิ้งการเปลี่ยนแปลง', exact: true }).click()
  await page.getByRole('button', { name: /เพิ่มธุรกรรม/ }).click()
  await page.getByLabel('จำนวนเงิน (THB)', { exact: true }).fill('555.25')
  await page.getByLabel('คำอธิบาย (ไม่จำเป็น)', { exact: true }).fill('Food ค่าอาหาร')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'en')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'en' }))
  })
  await expect(page.getByLabel('Amount (THB)', { exact: true })).toHaveValue('555.25')
  await expect(page.getByLabel('Description (optional)', { exact: true })).toHaveValue('Food ค่าอาหาร')
  await page.getByLabel('Amount (THB)', { exact: true }).fill('-1')
  await page.getByRole('button', { name: 'Save transaction', exact: true }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Amount must be greater than zero.')
  await page.evaluate(() => {
    localStorage.setItem('heyday-language', 'th')
    window.dispatchEvent(new StorageEvent('storage', { key: 'heyday-language', newValue: 'th' }))
  })
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('จำนวนเงินต้องมากกว่าศูนย์')
})

for (const [path, title, content] of [
  ['/', 'หน้าหลัก', 'เงินสดที่ใช้ได้'],
  ['/income', 'รายได้', 'ยังไม่มีแหล่งรายได้'],
  ['/transactions', 'รายการธุรกรรม', 'ความเคลื่อนไหวของเงินคุณ'],
  ['/net-worth', 'มูลค่าสุทธิ', 'ฐานะการเงินโดยรวมของคุณ'],
  ['/installments', 'รายการผ่อนชำระ', 'แผนผ่อนชำระ'],
  ['/subscriptions', 'รายการเรียกเก็บประจำ', 'ติดตามบริการประจำ'],
  ['/outlook', 'ภาพรวมล่วงหน้า', 'ยอดเงินสดปลายรอบสะสม'],
] as const) {
  test(`Thai ${path} loads with localized labels and no runtime errors`, async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', error => { if (!error.message.includes('ResizeObserver loop')) errors.push(error.message) })
    await mockDesktop(page)
    await page.addInitScript(() => localStorage.setItem('heyday-language', 'th'))
    await page.goto(`/#${path}`)
    await expect(page.getByRole('heading', { level: 1, name: title, exact: true })).toBeVisible()
    await expect(page.getByText(content, { exact: false }).first()).toBeVisible()
    if (path === '/outlook') {
      await expect(page.getByRole('table')).toContainText('รายได้ก่อนหัก')
      await expect(page.getByRole('table')).toContainText('ไม่พร้อมใช้งาน')
      await page.screenshot({ path: '/tmp/heyday-th-outlook.png', fullPage: true })
      await page.getByRole('tab', { name: 'ภาพรวมตามบัญชี', exact: true }).click()
      await expect(page.getByRole('table')).toContainText('ยอดเงินสดต้นรอบ')
    }
    await page.setViewportSize({ width: 620, height: 780 })
    await page.getByRole('button', { name: 'ขยายแถบด้านข้าง', exact: true }).click()
    expect(await page.locator('aside').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
    expect(errors).toEqual([])
  })
}
