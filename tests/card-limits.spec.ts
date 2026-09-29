import { test, expect } from '@playwright/test'
import { sharedCredit, sharedCreditAfter, type CardLimits } from '../src/lib/card-limits'

test('shared availability is exact, includes archived cards and overpayments, and counts transfers once', () => {
  const data: CardLimits = { groups:[{id:'g',name:'Bank',credit_limit:'10000000'}],cards:[{id:'a',name:'A',current_balance:'-500000',is_archived:true,group_id:'g'},{id:'b',name:'B',current_balance:'2000000',is_archived:false,group_id:'g'}] }
  expect(sharedCredit(data,'a')?.available).toBe(8500000n)
  expect(sharedCreditAfter(data,'g','repayment','cash','b',3000000n)).toBe(11500000n)
  expect(sharedCreditAfter(data,'g','expense','b','',1000000n)).toBe(7500000n)
  expect(sharedCreditAfter(data,'g','income','b','',1000000n)).toBe(9500000n)
  expect(sharedCreditAfter(data,'g','transfer','a','b',1000000n)).toBe(8500000n)
  data.cards.forEach(c => c.current_balance='9223372036854775807')
  expect(sharedCredit(data,'a')?.balance).toBe(18446744073709551614n)
  expect(sharedCredit(data,'missing')).toBeNull()
})

test.beforeEach(async ({page}) => {
  await page.addInitScript(() => {
    const base={loan_type:null,institution:null,last_four:null,notes:null,credit_limit:'5000000',statement_day:10,payment_due_day:30,interest_rate_ten_thousandths:null,monthly_installment:null,opening_balance:'0'}
    const initial=[{...base,id:'a',name:'Card A',type:'credit_card',current_balance:'2000000'},{...base,id:'b',name:'Card B',type:'credit_card',current_balance:'1500000',statement_day:25,payment_due_day:15},{...base,id:'cash',name:'Cash',type:'bank',current_balance:'10000000',credit_limit:null}]
    const read=(key:string,fallback:any)=>JSON.parse(localStorage.getItem(key)??JSON.stringify(fallback))
    const accounts=()=>read('limit-accounts',initial), groups=()=>read('limit-groups',[]), memberships=()=>read('limit-members',{})
    Object.defineProperty(window,'isTauri',{value:true})
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
      if(command==='plugin:app|version')return '0.0.1-alpha.5'
      if(command==='get_settings')return {currency:'THB',period_start_day:1}
      if(command==='list_institutions')return {institutions:[],accounts:[]}
      if(command==='list_transaction_options')return {payees:[],categories:[]}
      if(command==='list_accounts')return accounts()
      if(command==='update_account'){
        const rows=accounts().map((a:any)=>a.id===args.input.id?{...a,...args.input}:a)
        localStorage.setItem('limit-accounts',JSON.stringify(rows));return rows.find((a:any)=>a.id===args.input.id)
      }
      if(command==='list_card_limit_groups')return {groups:groups(),cards:accounts().filter((a:any)=>a.type==='credit_card').map((a:any)=>({...a,is_archived:false,group_id:memberships()[a.id]??null}))}
      if(command==='save_card_limit_group'){
        if(sessionStorage.getItem('fail-group'))throw 'Card belongs to another shared limit. Reload Accounts.'
        const {account_ids,currency,...g}=args.input;g.id??='g'
        localStorage.setItem('limit-groups',JSON.stringify([...groups().filter((x:any)=>x.id!==g.id),g]))
        const members=memberships();for(const id in members)if(members[id]===g.id)delete members[id]
        for(const id of account_ids)members[id]=g.id
        localStorage.setItem('limit-members',JSON.stringify(members));return
      }
      if(command==='delete_card_limit_group'){
        localStorage.setItem('limit-groups',JSON.stringify(groups().filter((g:any)=>g.id!==args.id)))
        const members=memberships();for(const id in members)if(members[id]===args.id)delete members[id]
        localStorage.setItem('limit-members',JSON.stringify(members));return
      }
      if(command==='create_transaction'){
        const input=args.input
        localStorage.setItem('last-limit-transaction',JSON.stringify(input))
        const rows=accounts().map((a:any)=>({...a,current_balance:(BigInt(a.current_balance)+(a.id===input.destination_account_id?-BigInt(input.amount):a.id===input.account_id?(a.type==='credit_card'?BigInt(input.amount):-BigInt(input.amount)):0n)).toString()}))
        localStorage.setItem('limit-accounts',JSON.stringify(rows));return {...input,id:'t'}
      }
      if(command==='get_card_billing')return {account:accounts().find((a:any)=>a.id===args.accountId),currency:'THB',billing:{statements:[],plans:[],allocations:[],installments:[]},transactions:[],entries:[],installments:[]}
      throw new Error(command)
    }}})
  })
})

test('manage shared groups, protect drafts, preview repayments, persist and retain separate billing', async ({page}) => {
  await page.goto('/#/accounts')
  await page.getByRole('tab',{name:'Credit cards (2)',exact:true}).click()
  await page.getByRole('button',{name:'Add shared limit',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await dialog.getByLabel('Group name').fill('Shared bank')
  await dialog.getByLabel('Shared credit limit (THB)',{exact:true}).fill('100000')
  await dialog.getByLabel('Card A',{exact:true}).check()
  await dialog.getByLabel('Card B',{exact:true}).check()
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await expect(dialog).toContainText('Discard your unsaved shared limit?')
  await dialog.getByRole('button',{name:'Keep editing',exact:true}).click()
  await page.evaluate(() => window.dispatchEvent(new Event('accounts-changed')))
  await expect(dialog.getByLabel('Group name')).toHaveValue('Shared bank')
  await page.setViewportSize({width:760,height:560})
  await expect(dialog.getByRole('button',{name:'Save shared limit',exact:true})).toBeInViewport()
  await page.screenshot({path:'/tmp/heyday-shared-credit-limit.png',animations:'disabled'})
  await page.setViewportSize({width:1280,height:900})
  await page.evaluate(()=>sessionStorage.setItem('fail-group','1'))
  await dialog.getByRole('button',{name:'Save shared limit',exact:true}).click()
  await expect(dialog.getByRole('alert')).toContainText('belongs to another')
  await expect(dialog.getByLabel('Group name')).toHaveValue('Shared bank')
  await page.evaluate(()=>sessionStorage.removeItem('fail-group'))
  await dialog.getByRole('button',{name:'Save shared limit',exact:true}).click()
  const section=page.getByRole('region',{name:'Shared credit limits',exact:true})
  await expect(section).toContainText('65,000.00 THB')
  await page.reload()
  await expect(section).toContainText('65,000.00 THB')
  await page.getByRole('button',{name:'Edit account Card A',exact:true}).click()
  await expect(dialog.getByLabel('Shared credit limit (THB)',{exact:true})).toHaveValue('100000.00')
  await expect(dialog.getByLabel('Shared credit limit (THB)',{exact:true})).toHaveAttribute('readonly','')
  await expect(dialog).toContainText('Shared group: Shared bank')
  await expect(dialog.getByLabel('Credit limit (THB, optional)',{exact:true})).toHaveCount(0)
  await dialog.getByLabel('Notes (optional)',{exact:true}).fill('Group card')
  await dialog.getByRole('button',{name:'Save account',exact:true}).click()
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('limit-accounts')!).find((a:any)=>a.id==='a').credit_limit)).toBe('5000000')
  await page.getByRole('button',{name:'Add transaction',exact:true}).click()
  await dialog.getByLabel('Amount (THB)',{exact:true}).fill('40000')
  await dialog.getByLabel('Transaction type',{exact:true}).selectOption('repayment')
  await dialog.getByLabel('From account',{exact:true}).fill('Cash'); await dialog.getByLabel('From account',{exact:true}).press('Enter')
  await dialog.getByLabel('To account',{exact:true}).fill('Card A'); await dialog.getByLabel('To account',{exact:true}).press('Enter')
  await expect(dialog).toContainText('Estimated shared available credit after transaction: 105,000.00 THB')
  await dialog.getByRole('button',{name:'Save transaction',exact:true}).click()
  await expect(section).toContainText('105,000.00 THB')
  await page.goto('/#/accounts/b/billing')
  await expect(page.getByText('Estimated shared available credit: 105,000.00 THB',{exact:true})).toBeVisible()
  await expect(page.getByText('15,000.00 THB',{exact:true})).toBeVisible()
  await page.goto('/#/accounts')
  await page.getByRole('button',{name:'Edit shared limit Shared bank',exact:true}).click()
  await dialog.getByLabel('Card B',{exact:true}).uncheck()
  await dialog.getByRole('button',{name:'Save shared limit',exact:true}).click()
  await expect(section).toContainText('120,000.00 THB')
  await page.getByRole('button',{name:'Edit shared limit Shared bank',exact:true}).click()
  await dialog.getByRole('button',{name:'Remove shared limit',exact:true}).click()
  await expect(dialog.getByRole('alert')).toContainText('saved individual limits')
  await dialog.getByRole('button',{name:'Remove group',exact:true}).click()
  await expect(section.getByRole('listitem')).toHaveCount(0)
  await page.reload()
  await expect(section.getByRole('listitem')).toHaveCount(0)
  await page.getByRole('button',{name:'Edit account Card A',exact:true}).click()
  await expect(dialog.getByLabel('Credit limit (THB, optional)',{exact:true})).toHaveValue('50000.00')
  await expect(dialog.getByLabel('Shared credit limit (THB)',{exact:true})).toHaveCount(0)
})
