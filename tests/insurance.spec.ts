import {test,expect} from '@playwright/test'
import {premiumOccurrences,premiumForecast,replaces,type Policy} from '../src/lib/insurance'
import {monthlyOutlook,netWorth} from '../src/lib/financial'
import {cycleTotals,plannerItems,itemAmount,openingForCycle,type PlannerData} from '../src/lib/cashflow'
import type {FinancialData,Account} from '../src/lib/desktop'
const policy=():Policy=>({id:'p',revision:'1',currency:'THB',details:{name:'Car cover',insurer:'Insurer',policy_type:'motor',number_suffix:'1234',coverage_start:'2024-01-01',coverage_end:null,renewal_date:null,coverage_amount:'70000000',deductible:'500000',notes:'',status:'active',status_date:'2024-01-01',asset_ids:['car'],cash_value_id:null,replaces:null},schedules:[{id:'s',effective_from:'2024-01-01',first_date:'2024-01-31',end_date:null,frequency:'monthly',amount:'100000',account_id:'bank',category_id:'insurance'}],payments:[]})
test('insurance recurrence clamps independently and preserves schedule version boundaries',()=>{
 const p=policy();expect(premiumOccurrences(p,'2024-01-01','2024-04-01').map(o=>o.date)).toEqual(['2024-01-31','2024-02-29','2024-03-31'])
 p.schedules[0].first_date='2024-02-29';p.schedules[0].frequency='yearly';expect(premiumOccurrences(p,'2025-01-01','2029-01-01').map(o=>o.date)).toEqual(['2025-02-28','2026-02-28','2027-02-28','2028-02-29'])
 p.schedules[0].frequency='once';expect(premiumOccurrences(p,'2024-01-01','2029-01-01')).toHaveLength(1)
 p.schedules.push({...p.schedules[0],id:'s2',effective_from:'2025-01-01',first_date:'2025-01-31',frequency:'quarterly',amount:'200000'})
 expect(premiumOccurrences(p,'2025-01-01','2025-08-01').map(o=>o.date)).toEqual(['2025-01-31','2025-04-30','2025-07-31'])
})
test('payments suppress forecasts, reversal restores them and expiry leaves asset values untouched',()=>{
 const p=policy(),payment={id:'paid',schedule_id:'s',due_date:'2024-02-29',date:'2024-02-28',amount:'100000',account_id:'bank',category_id:'insurance',notes:'',reversal_id:null}
 p.payments.push(payment);expect(premiumForecast([p],['bank'],'2024-02-01','2024-03-01')).toHaveLength(0)
 p.payments[0].reversal_id='reverse';expect(premiumForecast([p],['bank'],'2024-02-01','2024-03-01')).toHaveLength(1)
 expect(premiumForecast([p],[],'2024-02-01','2024-03-01')).toHaveLength(0)
 p.details.status='cancelled';p.details.status_date='2024-02-01';expect(premiumForecast([p],['bank'],'2024-02-01','2024-04-01')).toHaveLength(0)
 p.details.status='active';p.details.coverage_end='2024-02-29';expect(premiumOccurrences(p,'2024-01-01','2025-01-01')).toHaveLength(2)
})
test('both Outlook views replace linked forecasts once without adding coverage to Net Worth',()=>{
 const p=policy();p.details.replaces={kind:'subscription',id:'sub'}
 const account=(id:string,type:Account['type'],balance:string):Account=>({id,name:id,type,current_balance:balance,opening_balance:balance,is_archived:false,loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null})
 const data:FinancialData={settings:{currency:'THB',period_start_day:1},accounts:[account('bank','bank','10000000'),account('car','tracking','60000000')],incomes:[],transactions:[],plans:[],installments:[],categories:[],insurance:[p],subscriptions:[{id:'sub',name:'Premium',account_id:'bank',account_name:'Bank',account_type:'bank',category_id:'insurance',category_name:'Insurance',amount:'100000',frequency:'monthly',first_billing_date:'2024-01-31',end_date:null,is_active:true}]}
 expect(monthlyOutlook(data,new Date(2024,1,1))[0].buckets.expenses.forecast).toBe(100000n)
 expect(netWorth(data.accounts).total).toBe(70000000n)
 p.details.replaces={kind:'payment-plan',id:'plan'};data.subscriptions=[];data.plans=[{id:'plan',name:'Premium',type:'expense',account_id:'bank',account_name:'Bank',destination_account_id:null,destination_account_name:null,category_id:'insurance',category_name:'Insurance',amount:'100000',date:'2024-02-29'}]
 expect(monthlyOutlook(data,new Date(2024,1,1))[0].buckets.expenses.forecast).toBe(100000n)
 p.details.replaces={kind:'planner-item',id:'manual'}
 const planner:PlannerData={source_currency:'THB',period_start_day:1,categories:[],items:[{id:'manual',category_id:'expenses',name:'Old premium',description:'',card_name:'',transaction_category_id:null,schedule_amount:'100000',schedule_start:'2024-01',schedule_end:'2025-12'}],amounts:[],months:[],opening:{month:'2024-01',amount:'1000000'},incomes:[],income_deductions:[],debt_accounts:[],installments:[],credit_cards:[],card_transactions:[],expense_categories:[],insurance:[p],insurance_cash_accounts:['bank']}
 expect(replaces([p],'planner-item','manual')).toBe(true)
 expect(cycleTotals(planner,'2024-02','forecast').expenses).toBe(100000n)
 expect(itemAmount(planner,plannerItems(planner).find(i=>i.id==='manual')!,'2024-02','forecast').value).toBeNull()
 expect(openingForCycle(planner,'2024-03')).toBe(800000n)
})
