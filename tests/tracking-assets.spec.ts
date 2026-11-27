import { expect,test,type Page } from '@playwright/test'
async function mock(page:Page){await page.addInitScript(()=>{
 Object.defineProperty(window,'isTauri',{value:true})
 const base=(id:string,name:string,type:string,value:string)=>({id,name,type,current_balance:value,opening_balance:value,is_archived:false,loan_type:type==='loan'?'auto_loan':null,institution:null,last_four:null,notes:null,credit_limit:null,statement_day:null,payment_due_day:null,interest_rate_ten_thousandths:null,monthly_installment:null})
 const initial=[base('bank','Everyday bank','bank','10000000'),base('loan','Car loan','loan','35000000')]
 const read=()=>JSON.parse(localStorage.getItem('tracking-test')??JSON.stringify({accounts:initial,assets:{}}))
 const write=(s:any)=>localStorage.setItem('tracking-test',JSON.stringify(s))
 Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any={})=>{const s=read();switch(command){
 case'plugin:app|version':return'test'
 case'get_settings':return{currency:'THB',period_start_day:1}
 case'list_accounts':return s.accounts
 case'list_institutions':return{institutions:[],accounts:s.accounts.map((a:any)=>({...a,logo_asset_id:null,institution_id:null}))}
 case'list_card_limit_groups':return{groups:[],cards:[],currency:'THB'}
 case'list_transaction_options':return{payees:[],categories:[]}
 case'list_transactions':return[]
 case'get_financial_data':return{settings:{currency:'THB',period_start_day:1},accounts:s.accounts.filter((a:any)=>!a.is_archived),incomes:[],transactions:[],plans:[],installments:[],subscriptions:[],categories:[]}
 case'get_tracking_account':return s.assets[args.accountId]
 case'create_tracking_account':{const d=args.input;if(BigInt(d.opening_value)<0n)throw'Enter a non-negative integer amount.';const id=crypto.randomUUID(),asset={id,draft:d,current_value:d.opening_value,revision:'0',latest_valuation_date:d.opening_date,archived:false,events:[]};s.assets[id]=asset;s.accounts.push({...base(id,d.name,'tracking',d.opening_value),notes:d.notes});write(s);return asset}
 case'edit_tracking_account':{const asset=s.assets[args.accountId];asset.draft=args.input;asset.revision=String(Number(asset.revision)+1);s.accounts.find((a:any)=>a.id===asset.id).name=args.input.name;write(s);return asset}
 case'record_tracking_event':{const input=args.input,asset=s.assets[input.account_id];if(input.expected_revision!==asset.revision)throw'Tracking account changed or is archived. Reload before saving.';const prev=asset.current_value;let next=prev;if(input.action.kind==='value'){next=input.action.value;asset.latest_valuation_date=input.date}else if(input.action.kind==='receive'){if(BigInt(input.action.principal)>BigInt(prev))throw'Principal cannot exceed the outstanding receivable.';next=String(BigInt(prev)-BigInt(input.action.principal));const bank=s.accounts.find((a:any)=>a.id===input.cash_account_id);bank.current_balance=String(BigInt(bank.current_balance)+BigInt(input.action.principal)+BigInt(input.action.interest))}asset.revision=String(Number(asset.revision)+1);asset.current_value=next;asset.events.push({...input,revision:asset.revision,previous_value:prev,next_value:next});s.accounts.find((a:any)=>a.id===asset.id).current_balance=next;write(s);return asset}
 case'set_account_archived':{s.accounts.find((a:any)=>a.id===args.accountId).is_archived=args.archived;s.assets[args.accountId].archived=args.archived;write(s);return}
 default:throw Error('Unexpected command: '+command)
 }}}})
})}
async function create(page:Page,subtype:string,name:string,value:string){await page.goto('/#/accounts');await page.getByRole('button',{name:'Add account',exact:true}).click();await page.getByLabel('Account type',{exact:true}).selectOption('tracking');await page.getByLabel('Asset type',{exact:true}).selectOption(subtype);await page.getByLabel('Account name',{exact:true}).fill(name);await page.getByLabel('Opening value',{exact:true}).fill(value)}
test('Tracking car creation, equity, valuation confirmation and archive semantics',async({page})=>{
 await mock(page);await create(page,'vehicle','Family car','600000');await page.getByLabel('Linked loan (optional)').selectOption('loan');await page.getByRole('button',{name:'Save account',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0)
 await page.getByRole('link',{name:'Family car',exact:true}).first().click();const details=page.getByRole('region',{name:'Tracking details'});await expect(details.getByText('250,000.00 THB',{exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Update value',exact:true}).click();await page.getByLabel('New value',{exact:true}).fill('550000');await page.getByRole('button',{name:'Review change',exact:true}).click()
 const review=page.getByRole('dialog',{name:'Update value',exact:true}).getByRole('status')
 await expect(review).toContainText('Current value: 600,000.00 THB')
 await expect(review).toContainText('Value after change: 550,000.00 THB')
 // Reviewing must not persist a valuation before explicit confirmation.
 expect(await page.evaluate(()=>{
  const state=JSON.parse(localStorage.getItem('tracking-test')!)
  const asset=Object.values(state.assets).find((a:any)=>a.draft.name==='Family car') as any
  return {value:asset.current_value,events:asset.events.length}
 })).toEqual({value:'60000000',events:0})
 await page.getByRole('button',{name:'Confirm change',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect(details.getByText('200,000.00 THB',{exact:true})).toBeVisible();await expect(details.getByRole('cell',{name:'−50,000.00 THB Valuation change',exact:true})).toBeVisible()
 await page.getByRole('button',{name:'Archive account Family car',exact:true}).click();await expect(page.getByText('Archiving excludes this asset from Net Worth. Its history is kept. A linked loan remains unchanged.')).toBeVisible();await page.getByRole('dialog').getByRole('button',{name:'Archive account',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect(details.getByRole('button',{name:'Restore account Family car',exact:true})).toBeVisible();await expect(details.getByRole('button',{name:'Update value',exact:true})).toHaveCount(0)
 await page.goto('/#/net-worth')
 await expect(page.getByRole('heading',{name:'Your overall financial position.',exact:true})).toBeVisible()
 await expect(page.getByRole('region',{name:'Loans breakdown',exact:true})).toContainText('Car loan')
 await expect(page.getByRole('region',{name:'Tracking breakdown',exact:true})).toHaveCount(0)
})
test('Receivable repayment limits keep the draft and cash picker excludes assets and loans',async({page})=>{
 await mock(page);await create(page,'receivable','Friend loan','10000');await page.getByLabel('Debtor (optional)').fill('Friend');await page.getByRole('button',{name:'Save account',exact:true}).click();await page.getByRole('link',{name:'Friend loan',exact:true}).first().click();await page.getByRole('button',{name:'Receive repayment',exact:true}).click();const dialog=page.getByRole('dialog');await expect(dialog.getByLabel('Cash account').locator('option')).toHaveText(['Choose an account','Everyday bank']);await dialog.getByLabel('Principal amount').fill('11000');await dialog.getByLabel('Cash account').selectOption('bank');await dialog.getByRole('button',{name:'Review change'}).click();await dialog.getByRole('button',{name:'Confirm change'}).click();await expect(dialog.getByRole('alert')).toHaveText('Principal cannot exceed the outstanding receivable.');await expect(dialog.getByLabel('Principal amount')).toHaveValue('11000');await dialog.getByLabel('Principal amount').fill('2000');await dialog.getByRole('button',{name:'Review change'}).click();await dialog.getByRole('button',{name:'Confirm change'}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.getByRole('region',{name:'Tracking details'}).getByText('8,000.00 THB',{exact:true})).toBeVisible()
})
test('Tracking form protects drafts and supports Thai subtype labels',async({page})=>{
 await mock(page);await create(page,'insurance_cash_value','Policy cash value','1000');await expect(page.getByText('Enter cash surrender value only. Coverage and premiums do not increase this asset.')).toBeVisible();await page.getByRole('button',{name:'Cancel',exact:true}).click();await expect(page.getByRole('dialog',{name:'Discard changes?'})).toBeVisible();await page.getByRole('button',{name:'Keep editing'}).click();await expect(page.getByLabel('Account name',{exact:true})).toHaveValue('Policy cash value')
 await page.evaluate(()=>localStorage.setItem('heyday-language','th'));await page.reload();await page.goto('/#/accounts');await page.getByRole('button',{name:'เพิ่มบัญชี',exact:true}).click();await page.getByRole('dialog').getByRole('combobox',{name:'ประเภทบัญชี',exact:true}).selectOption('tracking');await expect(page.getByRole('option',{name:'มูลค่าเวนคืนประกัน'})).toHaveCount(1)
})

test('compact Tracking details expose stale values, keyboard review and cash-only filters',async({page})=>{
 await mock(page);await page.setViewportSize({width:640,height:720})
 await create(page,'vehicle','Older car','600000')
 await page.getByLabel('Opening date',{exact:true}).fill('2020-01-01')
 await page.getByRole('button',{name:'Save account',exact:true}).click()
 await page.getByRole('link',{name:'Older car',exact:true}).first().click()
 await expect(page.getByText('Value last reviewed at least 90 days ago. Review it against a current estimate; no automatic depreciation is applied.')).toBeVisible()
 const update=page.getByRole('button',{name:'Update value',exact:true});await update.focus();await page.keyboard.press('Enter')
 await expect(page.getByLabel('New value',{exact:true})).toBeFocused()
 await page.getByLabel('New value',{exact:true}).fill('550000')
 await page.getByRole('button',{name:'Review change'}).click()
 await expect(page.getByRole('dialog').getByRole('status')).toContainText('Value after change: 550,000.00 THB')
 await expect(page.getByRole('dialog').getByRole('status')).toContainText('Cash change: 0.00 THB')
 await page.keyboard.press('Escape');await expect(page.getByRole('dialog',{name:'Discard changes?'})).toBeVisible()
 await page.getByRole('button',{name:'Discard changes',exact:true}).click()
 await page.goto('/#/transactions');await page.getByLabel('Filter by account',{exact:true}).selectOption({label:'Older car'})
 await expect(page.getByText('Tracking filters show cash movements only. Valuations and write-offs are in asset history. Planned repayments do not change cash.')).toBeVisible()
 await expect(page.getByRole('link',{name:'Asset history',exact:true})).toBeVisible()
})
