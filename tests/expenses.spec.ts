import { test, expect } from '@playwright/test'
import { expenseComparison, type ExpenseReport } from '../src/lib/expenses'
import type { Transaction } from '../src/lib/desktop'
const transaction=(id:string,account:string,date:string,amount:string,type:Transaction['type']='expense',category:string|null='food'):Transaction=>({id,type,account_id:account,account_name:account==='card'?'Old Visa':'Everyday bank',destination_account_id:type==='repayment'?'card':null,destination_account_name:null,amount,date,description:id,payee_id:null,payee_name:null,category_id:category,category_name:category?'Food':null})
const fixture=():ExpenseReport=>({settings:{currency:'THB',period_start_day:25},accounts:[{id:'bank',name:'Everyday bank',type:'bank',is_archived:false},{id:'card',name:'Old Visa',type:'credit_card',is_archived:true}],transactions:[transaction('Groceries','card','2026-09-25','200000'),transaction('Lunch','bank','2026-09-26','300000'),transaction('Previous','bank','2026-09-24','100000'),transaction('Uncategorized item','bank','2026-09-25','123', 'expense',null),transaction('Bill payment','bank','2026-09-26','200000','repayment'),transaction('Transfer','bank','2026-09-26','99999','transfer'),transaction('Income','bank','2026-09-26','800000','income')]})
test('expenses count purchases once, include archived card history, and intersect account/method filters',()=>{
 const data=fixture(),result=expenseComparison(data,'2026-09','2026-09-27')
 expect(result.current).toBe(500123n);expect(result.previous).toBe(100000n);expect(result.difference).toBe(400123n);expect(result.transactionCount).toBe(3)
 expect(result.rows.map(r=>r.name)).toEqual(['Food','Uncategorized'])
 expect(expenseComparison(data,'2026-09','2026-09-27','','credit_card').current).toBe(200000n)
 expect(expenseComparison(data,'2026-09','2026-09-27','bank','credit_card').current).toBe(0n)
 expect(expenseComparison(data,'2026-09','2026-09-27','card').rows[0].children![0].name).toBe('Groceries')
})
test('seven past cycles clamp independently, exclude future entries, and preserve exact BigInt totals',()=>{
 const data=fixture();data.settings.period_start_day=31;data.transactions=[transaction('leap','card','2024-02-29','9007199254740993'),transaction('end','card','2024-03-30','9007199254740993'),transaction('next','card','2024-03-31','100')]
 const r=expenseComparison(data,'2024-02','2024-03-30')
 expect(r.cycles.at(-1)).toMatchObject({start:'2024-02-29',end:'2024-03-31'})
 expect(r.cycles).toHaveLength(7);expect(r.current).toBe(18014398509481986n)
 expect(expenseComparison(data,'2024-03','2024-03-30').current).toBe(0n)
 expect(expenseComparison(data,'0001-01','2024-03-30').cycles).toHaveLength(1)
 data.settings.period_start_day=1
 expect(expenseComparison(data,'2024-01','2024-03-30').cycles[5].month).toBe('2023-12')
})
test('category IDs keep renamed and uncategorized history distinct without changing the ledger',()=>{
 const data=fixture(),before=JSON.stringify(data)
 expenseComparison(data,'2026-09','2026-09-27');expect(JSON.stringify(data)).toBe(before)
 data.transactions[0].category_name='Meals';data.transactions[1].category_name='Meals';data.transactions[2].category_name='Meals'
 const r=expenseComparison(data,'2026-09','2026-09-27');expect(r.rows[0].name).toBe('Meals');expect(r.current).toBe(500123n)
})
test('Expenses tab supports keyboard access, drilldown, filters, refresh, dates and narrow dark layout',async({page})=>{
 await page.clock.setFixedTime(new Date(2026,8,27,12))
 await page.addInitScript(data=>{
  Object.defineProperty(window,'isTauri',{value:true})
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string)=>{
   if(command==='get_settings')return data.settings
   if(command==='list_accounts')return []
   if(command==='get_cashflow_planner')return {categories:[],incomes:[],income_deductions:[],debt_accounts:[],installments:[],credit_cards:[],card_transactions:[],items:[],amounts:[],months:[],opening:null,expense_categories:[],source_currency:'THB',period_start_day:25}
   if(command==='get_expense_report'){if(sessionStorage.getItem('fail-expenses'))throw 'Failed';return {...data,transactions:sessionStorage.getItem('delete-expense')?data.transactions.filter(t=>t.id!=='Groceries'):data.transactions}}
   throw new Error(command)
  }}})
 },fixture())
 await page.goto('/#/outlook')
 await page.getByRole('tab',{name:'Cashflow Planner',exact:true}).focus()
 await page.keyboard.press('ArrowRight')
 await expect(page.getByRole('tab',{name:'Expenses',exact:true})).toHaveAttribute('aria-selected','true')
 await expect(page.getByRole('heading',{name:'Expenses',exact:true})).toBeVisible()
 await expect(page.getByLabel('Expense cycle',{exact:true})).toHaveValue('2026-09')
 const table=page.getByRole('table',{name:'Expenses by category and payday cycle'})
 await expect(table.getByRole('row').last()).toContainText('5,001.23 THB')
 await page.getByRole('button',{name:'Expand Food',exact:true}).click()
 await expect(table.getByText('Groceries',{exact:true})).toBeVisible()
 await expect(table.getByText('Bill payment',{exact:true})).toHaveCount(0)
 await page.getByLabel('Payment method',{exact:true}).selectOption('credit_card')
 await expect(table.getByRole('row').last()).toContainText('2,000.00 THB')
 await page.getByLabel('Expense account',{exact:true}).selectOption('bank')
 await expect(page.getByText('No recorded expenses match these accounts, methods and periods.')).toBeVisible()
 await page.getByRole('button',{name:'Clear filters'}).click()
 await page.getByRole('button',{name:'Previous cycle',exact:true}).click()
 await expect(page.getByLabel('Expense cycle',{exact:true})).toHaveValue('2026-08')
 await page.getByRole('button',{name:'Current cycle',exact:true}).click()
 await page.evaluate(()=>{sessionStorage.setItem('delete-expense','1');window.dispatchEvent(new Event('transactions-changed'))})
 await expect(table.getByRole('row').last()).toContainText('3,001.23 THB')
 await page.evaluate(()=>{sessionStorage.setItem('fail-expenses','1');window.dispatchEvent(new Event('accounts-changed'))})
 await expect(page.getByRole('alert')).toContainText('Could not load expenses')
 await page.evaluate(()=>sessionStorage.removeItem('fail-expenses'))
 await page.getByRole('button',{name:'Retry expenses'}).click()
 await expect(table).toBeVisible()
 await page.getByLabel('Expense cycle',{exact:true}).fill('2026-08')
 await page.clock.setFixedTime(new Date(2026,9,26,12))
 await page.evaluate(()=>window.dispatchEvent(new Event('focus')))
 await expect(page.getByLabel('Expense cycle',{exact:true})).toHaveValue('2026-08')
 await page.getByRole('button',{name:'Current cycle',exact:true}).click()
 await expect(page.getByLabel('Expense cycle',{exact:true})).toHaveValue('2026-10')
 await page.screenshot({path:'/tmp/heyday-expenses.png',animations:'disabled'})
 await page.setViewportSize({width:640,height:850})
 await page.evaluate(()=>document.documentElement.dataset.theme='dark')
 await page.screenshot({path:'/tmp/heyday-expenses-dark.png',animations:'disabled'})
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true)
})
