import { test, expect } from '@playwright/test'
import { cardOverview, type CardOverviewData } from '../src/lib/card-overview'
import type { Account, Transaction } from '../src/lib/desktop'

const account=(id:string,day:number|null,balance:string):Account=>({id,name:`Card ${id.toUpperCase()}`,type:'credit_card',loan_type:null,institution:null,last_four:null,notes:null,opening_balance:'0',current_balance:balance,credit_limit:'6000000',statement_day:day,payment_due_day:5,interest_rate_ten_thousandths:null,monthly_installment:null})
const expense=(id:string,card:string,date:string,amount:string):Transaction=>({id,type:'expense',account_id:card,account_name:card,destination_account_id:null,destination_account_name:null,amount,date,description:id,payee_id:null,payee_name:null,category_id:'household',category_name:'Household'})
const example=():CardOverviewData=>({currency:'THB',accounts:[account('a',20,'30000'),account('b',15,'30000')],archived_ids:[],billing:{statements:[],plans:[],allocations:[],installments:[]},transactions:[expense('A1','a','2026-09-25','10000'),expense('A2','a','2026-10-05','20000'),expense('B1','b','2026-10-01','20000'),expense('B2','b','2026-10-22','10000')]})

test('October 23 separates two independently closing cards into 500 latest purchases and 100 next purchases',()=>{
  const result=cardOverview(example(),'2026-10-23')
  expect(result.latestSpending).toBe(50000n);expect(result.nextSpending).toBe(10000n);expect(result.owed).toBe(60000n)
  expect(result.rows[0]).toMatchObject({start:'2026-09-21',end:'2026-10-20',nextStart:'2026-10-21',nextEnd:'2026-11-20',latestSpending:30000n,nextSpending:0n,estimatedClosing:30000n,due:'2026-11-05'})
  expect(result.rows[1]).toMatchObject({start:'2026-09-16',end:'2026-10-15',nextStart:'2026-10-16',nextEnd:'2026-11-15',latestSpending:20000n,nextSpending:10000n,estimatedClosing:20000n})
})
test('partial payment preserves debt and purchase totals; bank difference and older rolling bills are not summed',()=>{
  const data=example();data.accounts[0].current_balance='25000'
  data.billing.statements=[{id:'s',account_id:'a',start_date:'2026-09-21',end_date:'2026-10-20',due_date:'2026-11-05',amount:'35000',minimum:'5000',needs_review:false},{id:'old',account_id:'a',start_date:'2026-08-21',end_date:'2026-09-20',due_date:'2026-10-05',amount:'90000',minimum:'9000',needs_review:false}]
  data.transactions.push({...expense('payment','cash','2026-10-22','5000'),type:'repayment',destination_account_id:'a'})
  data.billing.allocations=[{statement_id:'s',transaction_id:'payment',amount:'5000',date:'2026-10-22'}]
  let r=cardOverview(data,'2026-10-23')
  expect(r.rows[0].totals).toMatchObject({remaining:30000n,status:'Minimum covered · balance remains'})
  expect(r.rows[0].difference).toBe(-5000n)
  expect(r.rows[0].older).toHaveLength(1)
  expect(r.owed).toBe(55000n);expect(r.latestSpending).toBe(50000n)
  data.billing.allocations=[]
  r=cardOverview(data,'2026-10-23');expect(r.rows[0].unassignedPayments).toHaveLength(1);expect(r.rows[0].totals!.remaining).toBe(35000n)
})
test('month-end clamping, leap years, closing-day inclusion and year boundaries',()=>{
  const data=example();data.accounts=[account('a',31,'30000')]
  data.transactions=[expense('close','a','2024-02-29','10000'),expense('new','a','2024-03-01','20000')]
  expect(cardOverview(data,'2024-03-01').rows[0]).toMatchObject({start:'2024-02-01',end:'2024-02-29',nextStart:'2024-03-01',nextEnd:'2024-03-31',latestSpending:10000n,nextSpending:20000n})
  expect(cardOverview(data,'2024-02-29').rows[0].latestSpending).toBe(10000n)
  expect(cardOverview(data,'2025-03-01').rows[0]).toMatchObject({end:'2025-02-28',nextEnd:'2025-03-31'})
  expect(cardOverview(data,'2026-01-01').rows[0]).toMatchObject({end:'2025-12-31',nextEnd:'2026-01-31'})
})
test('archived cards are hidden from rows and totals; active overpayments stay exact',()=>{
  const data=example();data.accounts=[account('a',null,'9223372036854775807'),account('b',15,'-9007199254740993')];data.archived_ids=['a']
  const r=cardOverview(data,'2026-10-23')
  expect(r.rows.map(row=>row.account.id)).toEqual(['b'])
  expect(r.unconfigured).toBe(0);expect(r.owed).toBe(0n);expect(r.credit).toBe(9007199254740993n)
})

test('Outlook credit cards shows cycle totals, partial payments, details, refresh and bank review link',async({page})=>{
  await page.clock.setFixedTime(new Date(2026,9,23,12))
  await page.addInitScript((data)=>{
    Object.defineProperty(window,'isTauri',{value:true})
    localStorage.setItem('overview',JSON.stringify(data))
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string)=>{
      if(command==='plugin:app|version')return '0.0.1-alpha.5'
      if(command==='get_settings')return {currency:'THB',period_start_day:28}
      if(command==='list_accounts')return data.accounts
      if(command==='list_institutions')return {accounts:[],institutions:[]}
      if(command==='list_transaction_options')return {payees:[],categories:[]}
      if(command==='get_card_overview'){if(sessionStorage.getItem('fail-overview'))throw 'offline';return JSON.parse(localStorage.getItem('overview')!)}
      throw new Error(command)
    }}})
  },example())
  await page.goto('/#/outlook')
  await page.getByRole('tab',{name:'Credit Cards',exact:true}).click()
  const table=page.getByRole('table',{name:'Credit cards by statement cycle'})
  const a=table.getByRole('row').filter({hasText:'Card A'}),b=table.getByRole('row').filter({hasText:'Card B'})
  await expect(a).toContainText('2026-09-21 – 2026-10-20')
  await expect(a).toContainText('Estimated closing balance: 300.00 THB')
  await expect(b).toContainText('Purchases so far: 100.00 THB')
  await a.getByText('Details for Card A',{exact:true}).click()
  await expect(a).toContainText('2026-09-25 · A1 · Household · 100.00 THB')
  await expect(a.getByRole('link',{name:'Review bank statement & payments'})).toHaveAttribute('href','/#/accounts/a/billing')
  await page.evaluate(()=>{
    const d=JSON.parse(localStorage.getItem('overview')!);d.accounts[0].current_balance='25000'
    d.billing.statements=[{id:'s',account_id:'a',start_date:'2026-09-21',end_date:'2026-10-20',due_date:'2026-11-05',amount:'30000',minimum:'5000',needs_review:false}]
    d.billing.allocations=[{statement_id:'s',transaction_id:'p',amount:'5000',date:'2026-10-23'}]
    d.transactions.push({id:'p',type:'repayment',account_id:'cash',destination_account_id:'a',amount:'5000',date:'2026-10-23'})
    localStorage.setItem('overview',JSON.stringify(d));window.dispatchEvent(new Event('transactions-changed'))
  })
  await expect(a).toContainText('Remaining: 250.00 THB')
  await expect(a).toContainText('Minimum covered · balance remains')
  await expect(a).toContainText('Purchases: 300.00 THB')
  await page.setViewportSize({width:760,height:560})
  await expect(table).toBeVisible()
  await page.screenshot({path:'/tmp/heyday-credit-cards-outlook.png',animations:'disabled'})
  await page.evaluate(()=>{sessionStorage.setItem('fail-overview','1');window.dispatchEvent(new Event('accounts-changed'))})
  await expect(page.getByRole('alert')).toContainText('Could not load credit cards')
  await page.evaluate(()=>sessionStorage.removeItem('fail-overview'))
  await page.getByRole('button',{name:'Retry credit cards'}).click()
  await expect(table).toBeVisible()
})
