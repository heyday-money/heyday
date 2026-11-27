import {test,expect,type Page} from '@playwright/test'
async function setup(page:Page){
 await page.addInitScript(()=>{
  Object.defineProperty(window,'isTauri',{value:true})
  const accounts=[{id:'bank',name:'Bank',type:'bank',current_balance:'1000000',opening_balance:'1000000',is_archived:false,loan_type:null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null}]
  const settings={currency:'THB',period_start_day:1},category={id:'planner-expense-6',name:'Insurance',is_archived:false},policies:any[]=[]
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any={})=>{
   switch(command){
    case'plugin:app|version':return'test'
    case'get_settings':return settings
    case'list_accounts':return accounts
    case'list_insurance':return policies
    case'list_institutions':return{institutions:[],accounts:[]}
    case'list_transaction_options':return{payees:[],categories:[category]}
    case'get_financial_data':return{settings,accounts,incomes:[],transactions:[],plans:[],installments:[],subscriptions:[],categories:[category],insurance:policies}
    case'get_cashflow_planner':return{source_currency:'THB',period_start_day:1,categories:[],items:[],amounts:[],months:[],opening:null,incomes:[],income_deductions:[],debt_accounts:[],installments:[],credit_cards:[],card_transactions:[],expense_categories:[category]}
    case'save_insurance':{const input=args.input;const p={id:input.id,revision:'1',currency:input.currency,details:input.details,schedules:input.schedule?[input.schedule]:[],payments:[]};policies.push(p);return p}
    case'pay_insurance':{const i=args.input,p=policies.find(p=>p.id===i.policy_id),s=p.schedules.find((s:any)=>s.id===i.schedule_id);if(p.payments.some((v:any)=>v.due_date===i.due_date&&!v.reversal_id))throw'This premium occurrence is already paid.';p.payments.push({...i,amount:s.amount,account_id:s.account_id,category_id:s.category_id,reversal_id:null});p.revision='2';accounts[0].current_balance=String(BigInt(accounts[0].current_balance)-BigInt(s.amount));return p}
    case'reverse_insurance':{const i=args.input,p=policies.find(p=>p.id===i.policy_id),v=p.payments.find((v:any)=>v.id===i.payment_id);v.reversal_id=i.id;p.revision='3';accounts[0].current_balance=String(BigInt(accounts[0].current_balance)+BigInt(v.amount));return p}
    default:throw Error(command)
   }
  }}})
 })
}
test('insurance creation, confirmed payment and reversal preserve the visible history',async({page})=>{
 await setup(page);await page.goto('/#/insurance');await page.getByRole('button',{name:'Add policy',exact:true}).click()
 const dialog=page.getByRole('dialog')
 await dialog.getByLabel('Policy name',{exact:true}).fill('Car cover')
 await dialog.getByLabel('Insurer',{exact:true}).fill('Insurer')
 await dialog.getByLabel('Coverage amount',{exact:true}).fill('700000')
 await dialog.getByLabel('Premium amount',{exact:true}).fill('1000')
 await dialog.getByLabel('Paying account',{exact:true}).selectOption('bank')
 await dialog.getByLabel('Frequency',{exact:true}).selectOption('once')
 await dialog.getByRole('button',{name:'Save policy',exact:true}).click()
 await expect(page.getByRole('dialog')).toHaveCount(0)
 const policy=page.getByRole('region',{name:'Car cover',exact:true})
 await expect(policy.getByText('700,000.00 THB',{exact:true})).toBeVisible()
 await policy.getByText('Premium schedule and history',{exact:true}).click()
 await policy.getByRole('button',{name:'Record premium',exact:true}).click()
 await page.getByRole('checkbox',{name:'I confirm this premium was paid and has not already been recorded elsewhere.'}).check()
 await page.getByRole('dialog').getByRole('button',{name:'Confirm',exact:true}).click()
 await expect(policy.getByRole('button',{name:'Record premium',exact:true})).toHaveCount(0)
 await policy.getByRole('button',{name:'Reverse premium',exact:true}).click()
 await page.getByRole('checkbox',{name:'I confirm reversal of this recorded premium.'}).check()
 await page.getByRole('dialog').getByRole('button',{name:'Confirm',exact:true}).click()
 await expect(policy.getByRole('button',{name:'Record premium',exact:true})).toHaveCount(1)
 await expect(policy.getByText(/Reversed/)).toBeVisible()
})
test('policy forms retain dirty drafts and mask policy-number entry',async({page})=>{
 await setup(page);await page.goto('/#/insurance');await page.getByRole('button',{name:'Add policy',exact:true}).click()
 await page.getByLabel('Policy name',{exact:true}).fill('Keep this draft')
 await expect(page.getByLabel('Policy number suffix (optional)',{exact:true})).toHaveAttribute('maxlength','4')
 await page.getByRole('button',{name:'Cancel',exact:true}).click()
 await expect(page.getByRole('dialog',{name:'Discard changes?'})).toBeVisible()
 await page.getByRole('button',{name:'Keep editing',exact:true}).click()
 await expect(page.getByLabel('Policy name',{exact:true})).toHaveValue('Keep this draft')
})
