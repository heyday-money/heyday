import { test, expect } from '@playwright/test'
import { loanSchedule, type LoanContract } from '../src/lib/loans'
import { cycleTotals, openingForCycle, type PlannerData } from '../src/lib/cashflow'
import { monthlyOutlook } from '../src/lib/financial'
import type { FinancialData, Account } from '../src/lib/desktop'
const contract: LoanContract={id:'c1',account_id:'loan',name:'Borrowing 1',principal:'2000000',remaining_principal:'2000000',borrowing_date:'2024-01-31',receiving_account_id:'bank',payment_account_id:'bank',interest_rate:75000,monthly_amount:'210000',installment_count:3,first_due_date:'2024-01-31',schedule_end:null,notes:'',borrowing_kind:'existing',borrowing_transaction_id:null,needs_review:false,accounts_available:true}
const planner=():PlannerData=>({loan_contracts:[contract],loan_facilities:[{account_id:'loan',credit_limit:'10000000'}],incomes:[],income_deductions:[],source_currency:'THB',debt_accounts:[{id:'loan',name:'Cash card',loan_type:'personal_loan',current_balance:'2000000',monthly_installment:'999999',notes:null,is_archived:false}],installments:[{id:'legacy',name:'Legacy',debt_account_id:'loan',debt_account_name:'Cash card',debt_account_type:'loan',account_name:'Bank',monthly_amount:'123456',installment_count:3,first_due_date:'2024-01-31',accounts_available:true}],credit_cards:[],card_transactions:[],categories:[],items:[],amounts:[],months:[],opening:{month:'2024-01',amount:'1000000'},period_start_day:31,expense_categories:[]})
test('contract dates clamp independently; stop dates and unavailable sources exclude estimates',()=>{
 expect(loanSchedule(contract).map(p=>p.date)).toEqual(['2024-01-31','2024-02-29','2024-03-31'])
 expect(loanSchedule({...contract,schedule_end:'2024-02-29'})).toHaveLength(2)
 expect(loanSchedule({...contract,needs_review:true})).toEqual([])
 expect(loanSchedule({...contract,accounts_available:false})).toEqual([])
})
test('contract forecasts replace legacy and monthly values once, preserving parent overrides and carry',()=>{
 const data=planner();expect(cycleTotals(data,'2024-01').expenses).toBe(210000n)
 data.amounts=[{item_id:'debt:loan',month:'2024-02',amount:'0'}]
 expect(cycleTotals(data,'2024-02').expenses).toBe(0n)
 expect(openingForCycle(data,'2024-04')).toBe(580000n)
 expect(cycleTotals(data,'2024-04').expenses).toBe(0n)
 data.loan_contracts=[contract,{...contract,id:'c2',monthly_amount:'175000'}]
 expect(cycleTotals(data,'2024-01').expenses).toBe(385000n)
 data.source_currency='USD';expect(cycleTotals(data,'2024-01').expenses).toBe(0n)
})
test('account outlook uses cash-funded contract schedules without duplicating legacy schedules',()=>{
 const base={loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null,opening_balance:'0'}
 const accounts:Account[]=[{...base,id:'loan',name:'Loan',type:'loan',current_balance:'2000000'},{...base,id:'bank',name:'Bank',type:'bank',current_balance:'1000000'}]
 const input:FinancialData={settings:{currency:'THB',period_start_day:1},accounts,incomes:[],transactions:[],plans:[],subscriptions:[],categories:[],loan_contracts:[contract],loan_facilities:[{account_id:'loan',credit_limit:'10000000'}],installments:[{id:'legacy',name:'Legacy',account_id:'bank',account_name:'Bank',debt_account_id:'loan',debt_account_name:'Loan',debt_account_type:'loan',monthly_amount:'999999',installment_count:3,interest_rate_millis:null,first_due_date:'2024-01-31',purchase_kind:'existing_purchase',purchase_transaction_id:null}]}
 const result=monthlyOutlook(input,new Date(2024,0,1))
 expect(result[0].closing).toBe(790000n)
})
test('loan account UI creates a draft, preserves failed saves, and supports split repayment entry',async({page})=>{
 await page.addInitScript(({contract})=>{
  const base={loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null,opening_balance:'0'}
  const accounts=[{...base,id:'loan',name:'Cash card',type:'loan',loan_type:'personal_loan',last_four:'7878',current_balance:'3000000'},{...base,id:'bank',name:'Bank',type:'bank',current_balance:'3000000'}]
  Object.defineProperty(window,'isTauri',{value:true})
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
   if(command==='plugin:app|version')return '0.0.1-alpha.3'
   if(command==='list_institutions')return {institutions:[],accounts:[]}
   if(command==='get_settings')return {currency:'THB',period_start_day:1}
   if(command==='list_accounts')return accounts
   if(command==='create_account'){sessionStorage.setItem('created-account',JSON.stringify(args.input));return {...args.input,id:'new',current_balance:args.input.opening_balance}}
   if(command==='list_transaction_options')return {payees:[],categories:[]}
   if(command==='get_loan_account')return {account:accounts[0],currency:'THB',facility:{account_id:'loan',credit_limit:'10000000'},contracts:[contract],transactions:[],payment_parts:[]}
   if(command==='save_loan_contract'||command==='record_loan_repayment'){
    sessionStorage.setItem(command,JSON.stringify(args.input));if(sessionStorage.getItem('fail-save'))throw 'Save failed. Please retry.';return
   }
   throw new Error(command)
  }}})
 },{contract})
 await page.goto('/#/accounts/loan/loans')
 await expect(page.getByRole('heading',{name:'Cash card ··7878'})).toBeVisible()
 await expect(page.getByText('70,000.00',{exact:false})).toBeVisible()
 await page.getByRole('tab',{name:'Contracts (1)'}).click()
 await page.getByRole('button',{name:'Add borrowing'}).click()
 await page.getByLabel('Borrowing entry').selectOption('new')
 await page.getByLabel('Contract name / reference').fill('Borrowing 2')
 await page.getByLabel('Original principal (THB)').fill('10000')
 await page.getByLabel('Borrowing date').fill('2024-01-31')
 await page.getByLabel('Annual interest rate (%)').fill('5.1234')
 await page.getByLabel('Lender monthly payment (THB)').fill('1800')
 await page.getByLabel('Number of installments').fill('6')
 await page.getByLabel('First due date').fill('2024-02-29')
 await page.evaluate(()=>sessionStorage.setItem('fail-save','1'))
 await page.getByRole('button',{name:'Save',exact:true}).click()
 await expect(page.getByRole('alert')).toHaveText('Save failed. Please retry.')
 await expect(page.getByLabel('Contract name / reference')).toHaveValue('Borrowing 2')
 await page.getByRole('button',{name:'Cancel',exact:true}).click()
 await expect(page.getByText('Discard unsaved changes?')).toBeVisible()
 await page.getByRole('button',{name:'Keep editing'}).click()
 await page.evaluate(()=>sessionStorage.removeItem('fail-save'))
 await page.getByRole('button',{name:'Save',exact:true}).click()
 await expect(page.getByRole('dialog')).not.toBeVisible()
 const saved=await page.evaluate(()=>JSON.parse(sessionStorage.getItem('save_loan_contract')!))
 expect(saved).toMatchObject({principal:'1000000',remaining_principal:'1000000',interest_rate:51234,borrowing_kind:'new',monthly_amount:'180000'})
 await page.getByRole('button',{name:'Repay',exact:true}).click()
 await page.getByLabel('Total payment (THB)').fill('2100')
 await page.getByLabel('Principal (THB)',{exact:true}).fill('2000')
 await page.getByLabel('Interest (THB)',{exact:true}).fill('90')
 await page.getByLabel('Fees (THB)').fill('10')
 await page.getByRole('button',{name:'Save',exact:true}).click()
 expect(await page.evaluate(()=>JSON.parse(sessionStorage.getItem('record_loan_repayment')!))).toMatchObject({principal:'200000',interest:'9000',fee:'1000',total:'210000'})
 await page.screenshot({path:'/tmp/heyday-loan-contracts.png',fullPage:true})
 await page.goto('/#/accounts')
 await page.getByRole('button',{name:'Add account',exact:true}).click()
 await page.getByLabel('Account type',{exact:true}).selectOption('loan')
 await page.getByLabel('Loan structure').selectOption('revolving')
 await page.getByLabel('Shared credit limit (THB)').fill('100000')
 await page.getByLabel('Account name',{exact:true}).fill('New cash card')
 await page.getByRole('button',{name:'Save account',exact:true}).click()
 await expect(page.getByRole('dialog')).not.toBeVisible()
 expect(await page.evaluate(()=>JSON.parse(sessionStorage.getItem('created-account')!))).toMatchObject({type:'loan',loan_type:'personal_loan',revolving_credit_limit:'10000000'})
})

test('Outlook expands reference contracts while an account override replaces the full total',async({page})=>{
 const data=planner();data.categories=[{id:'debt',name:'Debt Payments',subtotal:'Total Debt Payments'}];data.amounts=[{item_id:'debt:loan',month:'2024-02',amount:'0'}]
 await page.clock.setFixedTime(new Date(2024,1,29,12))
 await page.addInitScript(({data})=>{
  Object.defineProperty(window,'isTauri',{value:true})
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string)=>{
   if(command==='list_institutions')return {institutions:[],accounts:[]}
   if(command==='get_settings')return {currency:'THB',period_start_day:31}
   if(command==='list_accounts')return []
   if(command==='get_cashflow_planner')return data
   if(command==='plugin:app|version')return '0.0.1-alpha.3'
   throw new Error(command)
  }}})
 },{data})
 await page.goto('/#/outlook')
 await page.getByRole('button',{name:'Expand Cash card contracts'}).click()
 const detail=page.getByRole('row').filter({hasText:'Borrowing 1 · schedule'})
 await expect(detail).toBeVisible()
 await expect(detail.getByRole('cell').first()).toHaveText('Account override')
 await expect(page.getByRole('button',{name:'Edit Cash card 2024-02 amount',exact:true})).toContainText('0.00')
 await page.getByRole('link',{name:'Cash card',exact:true}).click()
 await expect(page).toHaveURL(/accounts\/loan\/loans/)
})
