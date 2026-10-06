import { expect, test } from '@playwright/test'
import { cycleForDate, exclusionFor, excludedOnDate, includedCycleCount } from '../src/lib/selective-defaults'

test('exclusions follow clamped payday cycles with an exclusive resumption boundary', () => {
  const periods = [{account_id:'loan',start_month:'2024-02',end_month:'2024-04'}, {account_id:'loan',start_month:'2024-06',end_month:null}]
  expect(cycleForDate('2024-02-28',31)).toBe('2024-01')
  expect(cycleForDate('2024-02-29',31)).toBe('2024-02')
  expect(cycleForDate('2025-02-28',31)).toBe('2025-02')
  expect(excludedOnDate(periods,'loan','2024-04-29',31)).toBe(true)
  expect(excludedOnDate(periods,'loan','2024-04-30',31)).toBe(false)
  expect(exclusionFor(periods,'other','2024-02')).toBeUndefined()
  expect(includedCycleCount(periods,'loan','2024-01','2025-01')).toBe(3)
})

test('account details require review, preserve failed drafts, and persist exclusion and resumption history', async ({ page }) => {
  await page.clock.setFixedTime(new Date(2026,9,6,12))
  await page.addInitScript(() => {
    Object.defineProperty(window,'isTauri',{value:true})
    const account={id:'loan',name:'SCB Loan',type:'loan',loan_type:'mortgage',opening_balance:'10000',current_balance:'10000',monthly_installment:'100',is_archived:false}
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
      const periods=JSON.parse(localStorage.getItem('exclusions')??'[]')
      switch(command){
        case 'get_settings':return {currency:'THB',period_start_day:25}
        case 'list_accounts':return [account]
        case 'get_loan_account':return {account,currency:'THB',facility:null,contracts:[],transactions:[],payment_parts:[]}
        case 'list_institutions':return {institutions:[],accounts:[]}
        case 'list_card_limit_groups':return {currency:'THB',groups:[],cards:[]}
        case 'get_selective_default':return {periods,period_start_day:25,linked_items:[{kind:'Loan schedule',name:'SCB Loan'},{kind:'Saved cycle override',name:'2026-09'}]}
        case 'save_selective_default':{
          if(sessionStorage.getItem('fail'))throw 'Could not save.'
          const input=args.input
          if(input.action==='exclude')periods.push({account_id:'loan',start_month:input.month,end_month:null})
          else periods.find((p:any)=>!p.end_month).end_month=input.month
          localStorage.setItem('exclusions',JSON.stringify(periods));return
        }
        default:return []
      }
    }}})
  })
  await page.goto('/#/accounts/loan/details')
  await page.getByRole('button',{name:'Exclude planned repayments',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await expect(dialog.getByLabel('Start cycle')).toHaveValue('2026-09')
  await expect(dialog).toContainText('Saved cycle override · 2026-09')
  await expect(dialog.getByRole('button',{name:'Confirm exclusion'})).toBeDisabled()
  await dialog.getByRole('checkbox').check()
  await page.evaluate(()=>sessionStorage.setItem('fail','1'))
  await dialog.getByRole('button',{name:'Confirm exclusion'}).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not save.')
  await expect(dialog.getByRole('checkbox')).toBeChecked()
  await page.evaluate(()=>sessionStorage.removeItem('fail'))
  await dialog.getByRole('button',{name:'Confirm exclusion'}).click()
  await expect(dialog).toHaveCount(0)
  await page.reload()
  await page.getByRole('button',{name:'Selective Default',exact:true}).focus()
  await expect(page.getByRole('tooltip')).toContainText('since 2026-09')
  await page.getByRole('button',{name:'Resume repayments',exact:true}).click()
  await dialog.getByLabel('Resume cycle').fill('2026-11')
  await dialog.getByRole('checkbox').check()
  await dialog.getByRole('button',{name:'Confirm resumption'}).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('Excluded from 2026-09',{exact:false})).toContainText('Repayments resume in 2026-11')
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('exclusions')!))).toEqual([{account_id:'loan',start_month:'2026-09',end_month:'2026-11'}])
  await expect(page.getByText('100.00 THB',{exact:true})).toBeVisible()
})
