import { test, expect } from '@playwright/test'
import { plannerComparison, comparisonViews, cycleTotals, openingForCycle, plannerCycles, itemAmount, plannerItems, type PlannerData, type PlannerCardTransaction } from '../src/lib/cashflow'
const tx=(id:string,type:PlannerCardTransaction['type'],account_type:string,amount:string,destination_account_type:string|null=null,date='2024-02-29'):PlannerCardTransaction=>({id,type,account_id:account_type,account_name:account_type,account_type,amount,date,destination_account_id:destination_account_type,destination_account_type,category_id:'food',category_name:'Food'})
const data=():PlannerData=>({ledger_transactions:[],incomes:[],income_deductions:[],debt_accounts:[],installments:[],credit_cards:[],card_transactions:[],source_currency:'THB',categories:[],items:[{id:'budget',category_id:'expenses',name:'Food budget',description:'',card_name:'',transaction_category_id:'food',schedule_amount:'900',schedule_start:'2024-01',schedule_end:'2024-12'}],amounts:[{item_id:'budget',month:'2024-02',amount:'777'}],months:[],opening:{month:'2024-01',amount:'10000'},period_start_day:31,expense_categories:[]})
test('tracking uses actual cash flows, excludes purchases on cards and internal transfers, and never adds plans',()=>{
 const d=data();d.ledger_transactions=[tx('income','income','bank','1000'),tx('expense','expense','wallet','100'),tx('card-purchase','expense','credit_card','9000'),tx('card-payment','repayment','bank','200','credit_card'),tx('loan','transfer','cash','300','loan'),tx('internal','transfer','bank','800','wallet'),tx('invest','transfer','bank','50','investment'),tx('redeem','transfer','investment','70','bank')]
 const t=cycleTotals(d,'2024-02');expect(t).toMatchObject({netIncome:1000n,expenses:600n,otherIn:70n,otherOut:50n,outflows:650n,surplus:420n})
 expect(t.buckets).toEqual({income:1000n,deductions:0n,expenses:100n,cards:200n,debt:300n,installments:0n})
 expect(itemAmount(d,d.items[0],'2024-02').value).toBeNull()
 expect(itemAmount(d,plannerItems(d).find(i=>i.actual_key?.startsWith('actual:expense'))!,'2024-02').value).toBe(100n)
 expect(openingForCycle(d,'2024-03')).toBe(10420n)
 d.ledger_transactions=d.ledger_transactions.filter(t=>t.id!=='expense');expect(cycleTotals(d,'2024-02').surplus).toBe(520n)
 expect(d.amounts[0].amount).toBe('777')
})
test('forecast toggle restores saved entries and mixed hidden cycles carry precisely',()=>{
 const d=data();d.ledger_transactions=[tx('jan','expense','bank','100',null,'2024-01-31'),tx('feb','expense','bank','200'),tx('mar','expense','bank','300',null,'2024-03-31')]
 expect(openingForCycle(d,'2024-04')).toBe(9400n)
 d.months=[{month:'2024-02',status:'forecast'}]
 expect(cycleTotals(d,'2024-02').expenses).toBe(777n);expect(openingForCycle(d,'2024-04')).toBe(8823n)
 d.months=[{month:'2024-02',status:'complete'}];expect(cycleTotals(d,'2024-02').expenses).toBe(200n);expect(openingForCycle(d,'2024-04')).toBe(9400n)
 d.source_currency='USD';expect(plannerCycles(d,'2024-02')[0].available).toBe(false);expect(openingForCycle(d,'2024-03')).toBeNull()
})
test('boundaries are exclusive, exact totals exceed Number precision, future transactions excluded',()=>{
 const d=data();d.ledger_transactions=[tx('a','expense','bank','9007199254740993'),tx('b','expense','wallet','9007199254740993',null,'2024-03-30'),tx('c','expense','bank','1',null,'2024-03-31'),tx('future','expense','bank','999',null,'9999-01-01')]
 expect(cycleTotals(d,'2024-02').expenses).toBe(18014398509481986n)
 expect(cycleTotals(d,'2024-03').expenses).toBe(1n)
 d.months=[{month:'9999-01',status:'tracking'}];expect(cycleTotals(d,'9999-01').expenses).toBe(0n)
})
test('comparison UI shows forecast beside actual, refreshes payments, and preserves past plans',async({page})=>{
 await page.clock.setFixedTime(new Date(2024,2,10,12))
 const fixture=data();fixture.categories=[{id:'income',name:'Gross Income',subtotal:'Total Income'},{id:'deductions',name:'Income Deductions',subtotal:'Total Deductions'},{id:'debt',name:'Debt Payments',subtotal:'Total Debt'},{id:'installments',name:'Card Installments',subtotal:'Total Installments'},{id:'cards',name:'Credit Cards',subtotal:'Total Cards'},{id:'expenses',name:'General Expenses',subtotal:'Total General Expenses'}]
 fixture.ledger_transactions=[tx('income','income','bank','10000'),tx('food','expense','bank','500')];fixture.credit_cards=[{id:'credit_card',name:'Visa',is_archived:false}]
 await page.addInitScript(fixture=>{
  Object.defineProperty(window,'isTauri',{value:true});let state=fixture
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
   if(command==='get_settings')return {currency:'THB',period_start_day:31};if(command==='list_accounts')return []
   if(command==='get_cashflow_planner')return {...state,ledger_transactions:sessionStorage.getItem('changed')?[...state.ledger_transactions!,{id:'pay',type:'repayment',account_id:'bank',account_type:'bank',account_name:'Bank',destination_account_id:'credit_card',destination_account_type:'credit_card',amount:'200',date:'2024-03-01'}]:state.ledger_transactions}
   if(command==='save_cashflow_planner'){state={...state,months:[{month:args.input.month,status:args.input.status}]};return state}
   throw new Error(command)
  }}})
 },fixture)
 await page.goto('/#/outlook')
 await expect(page.getByLabel('Food 2024-02: Recorded actual',{exact:true})).toHaveText('5.00')
 await expect(page.getByRole('columnheader',{name:/Feb 2024/})).toHaveAttribute('colspan','2')
 await expect(page.getByLabel('Status 2024-02')).toHaveCount(0)
 await expect(page.getByRole('button',{name:'Edit Food budget 2024-02 amount'})).toContainText('7.77')
 await expect(page.getByLabel('Total Income 2024-02: Not recorded separately',{exact:true})).toHaveText('—Not recorded separately')
 await expect(page.getByLabel('Total Deductions 2024-02: Not recorded separately',{exact:true})).toHaveText('—Not recorded separately')
 await page.evaluate(()=>{sessionStorage.setItem('changed','1');window.dispatchEvent(new Event('transactions-changed'))})
 await expect(page.getByLabel('Visa 2024-02: Recorded actual',{exact:true})).toHaveText('2.00')
 await page.evaluate(()=>{sessionStorage.removeItem('changed');window.dispatchEvent(new Event('transactions-changed'))})
 await expect(page.getByLabel('Visa 2024-02: Recorded actual',{exact:true})).toHaveText('0.00')
 await expect(page.getByRole('region',{name:'Forecast summary',exact:true})).toBeVisible()
 await expect(page.getByRole('region',{name:'Actual summary',exact:true})).toBeVisible()
 await page.screenshot({path:'/tmp/heyday-comparison.png',animations:'disabled'})
 await page.getByLabel('First visible cycle').fill('2024-01')
 await expect(page.getByRole('button',{name:'Edit Food budget 2024-01 amount'})).toHaveCount(0)
 await page.getByLabel('Compare past forecasts',{exact:true}).check()
 await expect(page.getByRole('button',{name:'Edit Food budget 2024-01 amount'})).toContainText('9.00')
 await page.getByLabel('Compare past forecasts',{exact:true}).uncheck()
 await page.getByLabel('First visible cycle').fill('2024-03')
 await expect(page.getByRole('region',{name:'Actual summary',exact:true})).toHaveCount(0)
 await expect(page.getByRole('columnheader',{name:'Actual Recorded transactions',exact:true})).toHaveCount(0)
 await page.getByRole('button',{name:'Current cycle',exact:true}).click()
 await page.clock.setFixedTime(new Date(2024,2,31,12))
 await page.evaluate(()=>window.dispatchEvent(new Event('focus')))
 await expect(page.getByLabel('First visible cycle')).toHaveValue('2024-03')
 await expect(page.getByRole('columnheader',{name:/Mar 2024/})).toHaveAttribute('colspan','2')
 await page.setViewportSize({width:680,height:900})
 await page.evaluate(()=>document.documentElement.dataset.theme='dark')
 await page.screenshot({path:'/tmp/heyday-comparison-dark.png',animations:'disabled'})
 const tableRegion=page.getByRole('region',{name:'Scrollable seven-cycle cashflow planner in thb',exact:true})
 await expect(tableRegion).toBeVisible()
 expect(await page.locator('main').evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
 expect(await tableRegion.evaluate(el=>el.scrollWidth>el.clientWidth)).toBe(true)
 await tableRegion.scrollIntoViewIfNeeded()
 await tableRegion.evaluate(el=>el.scrollLeft=300)
 expect(await tableRegion.evaluate(el=>el.scrollLeft)).toBeGreaterThan(0)
 await page.screenshot({path:'/tmp/heyday-comparison-table-dark.png',animations:'disabled'})
})


test('comparison uses one actual opening, separate full-cycle totals, and carries the current forecast once',()=>{
 const d=data();d.ledger_transactions=[tx('jan','expense','bank','100',null,'2024-01-31'),tx('feb','expense','bank','200')]
 d.months=[{month:'2024-01',status:'forecast'},{month:'2024-02',status:'complete'},{month:'2024-03',status:'tracking'}]
 const before=JSON.stringify(d)
 const [feb,mar]=plannerComparison(d,'2024-02','2024-02')
 expect(feb.forecast).toMatchObject({opening:9900n,expenses:777n,closing:9123n})
 expect(feb.actual).toMatchObject({opening:9900n,expenses:200n,closing:9700n})
 expect(mar.forecast).toMatchObject({opening:9123n,closing:8223n})
 expect(plannerComparison(d,'2024-01','2024-02')[0].forecast.opening).toBe(10000n)
 expect(comparisonViews('2024-01','2024-02',false)).toEqual(['actual'])
 expect(comparisonViews('2024-01','2024-02',true)).toEqual(['forecast','actual'])
 expect(comparisonViews('2024-02','2024-02',false)).toEqual(['forecast','actual'])
 expect(comparisonViews('2024-03','2024-02',true)).toEqual(['forecast'])
 expect(JSON.stringify(d)).toBe(before)
 d.opening=null;expect(plannerComparison(d,'2024-02','2024-02')[0].actual.closing).toBeNull()
})
test('net receipt does not infer gross salary or deduct estimated payroll twice in comparison',()=>{
 const d=data();d.incomes=[{id:'salary',name:'Salary',estimated_amount:'5000000',recurrence_day_of_month:1,is_active:true,destination_account_name:'Bank',account_archived:false}]
 d.income_deductions=[{id:'tax',income_id:'salary',name:'Tax',amount:'500000',description:''}]
 d.ledger_transactions=[tx('net','income','bank','4500000')]
 const c=plannerComparison(d,'2024-02','2024-02')[0]
 expect(c.forecast.netIncome).toBe(4500000n);expect(c.actual.netIncome).toBe(4500000n)
 const gross=plannerItems(d).find(i=>i.id==='income:salary')!,tax=plannerItems(d).find(i=>i.id==='deduction:tax')!
 expect(itemAmount(d,gross,'2024-02','actual').value).toBeNull()
 expect(itemAmount(d,tax,'2024-02','actual').value).toBeNull()
 expect(c.actual.buckets.deductions).toBe(0n)
 d.source_currency='USD';expect(plannerComparison(d,'2024-02','2024-02')[0].actual.available).toBe(false)
})


test('actual income groups linked sources across accounts without counting definitions or deductions again',()=>{
 const d=data();d.ledger_transactions=[{...tx('one','income','bank','4500000'),income_source_id:'salary',income_source_name:'Company salary'},{...tx('two','income','cash','10000'),income_source_id:'salary',income_source_name:'Company salary'},tx('other','income','bank','200')]
 const rows=plannerItems(d).filter(i=>i.actual_key?.startsWith('actual:income:'))
 expect(rows).toHaveLength(2)
 const salary=rows.find(i=>i.name==='Received · Company salary')!
 expect(itemAmount(d,salary,'2024-02','actual').value).toBe(4510000n)
 expect(cycleTotals(d,'2024-02','actual').netIncome).toBe(4510200n)
})
