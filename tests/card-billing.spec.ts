import { test, expect } from '@playwright/test'
import { statementTotals, draftBillingDates, cardForecasts, type CardBillingData, type CardStatement } from '../src/lib/card-billing'
import { cycleTotals, openingForCycle, itemAmount, plannerItems, type PlannerData } from '../src/lib/cashflow'
import { monthlyOutlook } from '../src/lib/financial'
import type { Account, FinancialData } from '../src/lib/desktop'
const statement:CardStatement={id:'s',account_id:'card',start_date:'2098-12-01',end_date:'2098-12-31',due_date:'2099-01-10',amount:'1000000',minimum:'100000',needs_review:false}
const billing=():CardBillingData=>({statements:[statement],plans:[{statement_id:'s',account_id:'bank',date:'2099-01-10',mode:'custom',target:'400000',available:true}],allocations:[{statement_id:'s',transaction_id:'paid',amount:'150000',date:'2099-01-02'}],installments:[]})
const planner=():PlannerData=>({card_billing:billing(),source_currency:'THB',incomes:[],income_deductions:[],debt_accounts:[],installments:[],credit_cards:[{id:'card',name:'Visa',is_archived:false}],card_transactions:[{id:'paid',type:'repayment',account_id:'bank',account_type:'bank',destination_account_id:'card',destination_account_type:'credit_card',date:'2099-01-02',amount:'150000'}],categories:[],items:[],amounts:[],months:[],opening:{month:'2099-01',amount:'1000000'},period_start_day:1,expense_categories:[]})
test('full, minimum and custom plans subtract applied payments exactly and never imply paid',()=>{
 const b=billing();expect(statementTotals(b,statement,'2099-01-03')).toMatchObject({paid:150000n,remaining:850000n,minimum:0n,planned:250000n,afterPlan:600000n,status:'Minimum covered · balance remains'})
 b.plans[0].mode='full';expect(statementTotals(b,statement,'2099-01-03').planned).toBe(850000n)
 b.plans[0].mode='minimum';expect(statementTotals(b,statement,'2099-01-03').planned).toBe(0n)
 b.allocations=[];expect(statementTotals(b,statement,'2099-01-03').planned).toBe(100000n)
 b.statements[0]={...statement,amount:'9007199254740993'};b.plans[0].mode='full';expect(statementTotals(b,b.statements[0],'2099-01-03').planned).toBe(9007199254740993n)
})
test('past plans, unavailable accounts and review bills do not forecast or move dates',()=>{
 const b=billing();expect(cardForecasts(b,'2099-01-11')).toEqual([])
 expect(statementTotals(b,statement,'2099-01-11').status).toBe('Minimum covered · balance remains')
 expect(statementTotals({...b,allocations:[]},statement,'2099-01-11').status).toBe('Minimum overdue')
 b.plans[0].available=false;expect(cardForecasts(b,'2099-01-03')).toEqual([])
 b.plans[0].available=true;b.statements[0]={...statement,needs_review:true};expect(cardForecasts(b,'2099-01-03')).toEqual([])
})
test('billing boundaries clamp independently, separate from payday; due dates follow close',()=>{
 expect(draftBillingDates(31,10,'2024-03-01')).toEqual({start_date:'2024-02-01',end_date:'2024-02-29',due_date:'2024-03-10'})
 expect(draftBillingDates(31,31,'2024-04-01')).toEqual({start_date:'2024-03-01',end_date:'2024-03-31',due_date:'2024-04-30'})
 expect(draftBillingDates(20,10,'2026-09-27')).toEqual({start_date:'2026-08-21',end_date:'2026-09-20',due_date:'2026-10-10'})
})
test('planner counts actual plus remaining plan once and carries hidden cycles; purchases are informational',()=>{
 const d=planner();d.card_transactions.push({id:'buy',type:'expense',account_id:'card',account_type:'credit_card',destination_account_id:null,destination_account_type:null,date:'2099-01-01',amount:'999999',category_name:'Food'})
 expect(cycleTotals(d,'2099-01').expenses).toBe(400000n)
 expect(openingForCycle(d,'2099-04')).toBe(600000n)
 d.period_start_day=5;expect(cycleTotals(d,'2098-12').expenses).toBe(150000n);expect(cycleTotals(d,'2099-01').expenses).toBe(250000n)
 d.source_currency='USD';expect(cycleTotals(d,'2099-01').expenses).toBe(0n)
})
test('explicit installment coverage suppresses only its occurrence and preserves cycle overrides',()=>{
 const d=planner();d.installments=[{id:'i',name:'Phone',debt_account_id:'card',debt_account_name:'Visa',debt_account_type:'credit_card',account_name:'Bank',monthly_amount:'50000',first_due_date:'2099-01-10',installment_count:3,accounts_available:true}]
 // Managed cards do not discard every installment just because one payment exists.
 expect(cycleTotals(d,'2099-01').expenses).toBe(450000n)
 d.card_billing!.installments=[{statement_id:'s',installment_id:'i',date:'2099-01-10'}]
 d.amounts=[{item_id:'installment:i',month:'2099-01',amount:'88888'}]
 expect(cycleTotals(d,'2099-01').expenses).toBe(400000n)
 expect(openingForCycle(d,'2099-04')).toBe(500000n)
 d.card_billing!.installments=[];expect(cycleTotals(d,'2099-01').expenses).toBe(488888n)
})
test('SCB billing and payday cycles cover the selected September occurrence, not the due month',()=>{
 const d=planner();d.period_start_day=28;d.card_transactions=[];d.card_billing!.allocations=[]
 d.installments=[{id:'i',name:'ThinkPad',debt_account_id:'card',debt_account_name:'SCB CardX BEYOND',debt_account_type:'credit_card',account_name:'Bank',monthly_amount:'431570',first_due_date:'2099-09-28',installment_count:6,accounts_available:true}]
 d.card_billing!.statements=[{...statement,start_date:'2099-08-24',end_date:'2099-09-23',due_date:'2099-10-13',amount:'1380406'}]
 d.card_billing!.plans=[{statement_id:'s',account_id:'bank',date:'2099-10-13',mode:'full',target:'1380406',available:true}]
 d.card_billing!.installments=[{statement_id:'s',installment_id:'i',date:'2099-09-28'}]
 expect(cycleTotals(d,'2099-09').expenses).toBe(1380406n)
 expect(cycleTotals(d,'2099-10').expenses).toBe(431570n)
 // A changed schedule cannot leave phantom coverage suppressing a saved override.
 d.installments[0].first_due_date='2099-10-28'
 d.amounts=[{item_id:'installment:i',month:'2099-09',amount:'12000'}]
 expect(cycleTotals(d,'2099-09').expenses).toBe(1392406n)
 d.card_billing!.statements[0].needs_review=true
 const card=plannerItems(d).find(i=>i.credit_card)!
 expect(itemAmount(d,card,'2099-09')).toMatchObject({value:0n,source:'Statement needs review'})
 // Keep the saved plan unchanged while its statement awaits explicit confirmation.
 expect(d.card_billing!.plans[0].date).toBe('2099-10-13')
})
test('account outlook excludes covered installments and includes only remaining cash payment',()=>{
 const base={loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null,opening_balance:'0'}
 const accounts:Account[]=[{...base,id:'card',name:'Visa',type:'credit_card',current_balance:'850000'},{...base,id:'bank',name:'Bank',type:'bank',current_balance:'850000'}]
 const data:FinancialData={settings:{currency:'THB',period_start_day:1},accounts,incomes:[],transactions:[],plans:[],subscriptions:[],categories:[],installments:[],card_billing:billing()}
 expect(monthlyOutlook(data,new Date(2099,0,3))[0].buckets.repayments.forecast).toBe(250000n)
 expect(monthlyOutlook(data,new Date(2099,0,11))[0].buckets.repayments.forecast).toBe(0n)
})
test('billing UI confirms statement, plans partial payment, links existing payment, and preserves failed drafts',async({page})=>{
 await page.addInitScript(()=>{
  const base={loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:20,payment_due_day:10,interest_rate_ten_thousandths:null,monthly_installment:null,opening_balance:'1000000',current_balance:'1000000'}
  const accounts=[{...base,id:'card',name:'Visa',type:'credit_card',credit_limit:'5000000'},{...base,id:'bank',name:'Bank',type:'bank'}]
  let statements:any[]=[{id:'s',account_id:'card',start_date:'2024-01-01',end_date:'2024-01-31',due_date:'2024-02-10',amount:'1000000',minimum:'100000',needs_review:false}],plans:any[]=[],allocations:any[]=[]
  Object.defineProperty(window,'isTauri',{value:true});Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
   if(command==='plugin:app|version')return '0.0.1-alpha.3'
   if(command==='list_institutions')return {institutions:[],accounts:[]}
   if(command==='get_settings')return {currency:'THB',period_start_day:1}
   if(command==='list_card_limit_groups')return {groups:[],cards:[]}
   if(command==='list_accounts')return accounts
   if(command==='get_card_billing')return {account:accounts[0],currency:'THB',billing:{statements,plans,allocations,installments:[]},transactions:[{id:'purchase',type:'expense',account_id:'card',account_name:'Visa',destination_account_id:null,amount:'10000',date:'2024-01-10',description:'Groceries',category_name:'Food'},{id:'payment',type:'repayment',account_id:'bank',account_name:'Bank',destination_account_id:'card',amount:'150000',date:'2024-02-02',description:''}],entries:[{statement_id:'s',transaction_id:'purchase'}],installments:[{id:'i',name:'ThinkPad',account_id:'bank',debt_account_id:'card',monthly_amount:'1000',first_due_date:'2024-01-28',installment_count:6}]}
   if(command==='save_card_payment_plan'){sessionStorage.setItem(command,JSON.stringify(args.input));if(sessionStorage.getItem('fail-save'))throw 'Save failed. Retry.';plans=[{...args.input,available:true}];return}
   if(command==='save_card_statement'){sessionStorage.setItem(command,JSON.stringify(args.input));statements=[{...args.input,id:'s',needs_review:false}];return}
   if(command==='record_card_payment'){sessionStorage.setItem(command,JSON.stringify(args.input));allocations=[{statement_id:'s',transaction_id:'payment',date:'2024-02-02',amount:'150000'}];return}
   throw new Error(command)
  }}})
 })
 await page.goto('/#/accounts/card/billing')
 await expect(page.getByRole('heading',{name:'Visa · Billing'})).toBeVisible()
 await page.getByText('Bill summary and spending by category',{exact:true}).click()
 await expect(page.getByText('Food ·')).toBeVisible()
 await page.getByRole('button',{name:'Plan payment',exact:true}).click()
 await page.getByLabel('Payment option').selectOption('custom')
 await page.getByLabel('Total payment target').fill('4000')
 await page.getByLabel('Pay from',{exact:true}).selectOption('bank')
 await page.evaluate(()=>sessionStorage.setItem('fail-save','1'))
 await page.getByRole('button',{name:'Save',exact:true}).click()
 await expect(page.getByRole('alert')).toHaveText('Save failed. Retry.')
 await expect(page.getByLabel('Total payment target')).toHaveValue('4000')
 await page.getByRole('button',{name:'Cancel',exact:true}).click()
 await expect(page.getByText('Discard unsaved changes?')).toBeVisible()
 await page.getByRole('button',{name:'Keep editing'}).click()
 await page.evaluate(()=>sessionStorage.removeItem('fail-save'))
 await page.getByRole('button',{name:'Save',exact:true}).click()
 await expect(page.getByRole('dialog')).toHaveCount(0)
 await page.getByRole('button',{name:'Link existing payment'}).click()
 await page.getByLabel('Existing payment',{exact:true}).selectOption('payment')
 await page.getByRole('button',{name:'Save',exact:true}).click()
 await expect(page.getByText(/still planned.*2,500/)).toBeVisible()
 expect(await page.evaluate(()=>JSON.parse(sessionStorage.getItem('record_card_payment')!))).toMatchObject({transaction_id:'payment',statement_id:'s'})
 await page.getByRole('button',{name:'Review statement',exact:true}).click()
 await page.getByLabel('Statement amount',{exact:true}).fill('10000')
 await page.getByRole('checkbox',{name:'ThinkPad · 2024-01-28'}).check()
 await expect(page.getByRole('checkbox',{name:'ThinkPad · 2024-02-28'})).not.toBeChecked()
 await page.getByRole('button',{name:'Confirm statement'}).click()
 expect(await page.evaluate(()=>JSON.parse(sessionStorage.getItem('save_card_statement')!))).toMatchObject({amount:'1000000',minimum:'100000',installments:[{installment_id:'i',date:'2024-01-28'}]})
 await expect(page.getByRole('dialog')).toHaveCount(0)
 await page.getByRole('heading',{name:'Visa · Billing'}).scrollIntoViewIfNeeded()
 await page.screenshot({path:'/tmp/heyday-card-billing-light.png',fullPage:true,animations:'disabled'})
 await page.setViewportSize({width:640,height:800})
 await page.evaluate(()=>document.documentElement.dataset.theme='dark')
 await page.screenshot({path:'/tmp/heyday-card-billing-dark-narrow.png',fullPage:true,animations:'disabled'})
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true)
 await page.getByRole('button',{name:'Review statement',exact:true}).click()
 await expect(page.getByRole('dialog')).toBeVisible()
 await page.getByRole('checkbox',{name:'ThinkPad · 2024-06-28'}).scrollIntoViewIfNeeded()
 await expect(page.getByRole('button',{name:'Confirm statement'})).toBeInViewport()
 await page.screenshot({path:'/tmp/heyday-card-statement-dialog.png',fullPage:true,animations:'disabled'})
})
