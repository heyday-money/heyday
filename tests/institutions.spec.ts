import { test, expect } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('sidebar-collapsed', 'false')
    Object.defineProperty(window, 'isTauri', { value: true })
    const seed = [{id:'bank-002',name:'Bangkok Bank Public Company Limited',short_name:'BBL',bank_code:'002',swift_code:'BKKBTHBK',logo:'bbl.svg',is_archived:false}]
    const institutions = () => JSON.parse(localStorage.getItem('institutions') ?? JSON.stringify(seed))
    const accounts = () => JSON.parse(localStorage.getItem('accounts') ?? '[]')
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
      if(command==='plugin:app|version')return '0.0.1-alpha.5'
      if(command==='get_settings')return {currency:'THB',period_start_day:1}
      if(command==='list_transaction_options')return {payees:[],categories:[]}
      if(command==='list_institutions')return {institutions:institutions(),accounts:accounts()}
      if(command==='list_accounts')return accounts()
      if(command==='delete_institution'){
        if(accounts().some((a:any)=>a.institution_id===args.id))throw 'This institution has linked accounts. Archive it instead.'
        localStorage.setItem('institutions',JSON.stringify(institutions().filter((i:any)=>i.id!==args.id)))
        return null
      }
      if(command==='save_institution'){
        const item={...institutions().find((i:any)=>i.id===args.input.id),...args.input,id:args.input.id??crypto.randomUUID()}
        localStorage.setItem('institutions',JSON.stringify([...institutions().filter((i:any)=>i.id!==item.id),item]))
        localStorage.setItem('accounts',JSON.stringify(accounts().map((a:any)=>a.institution_id===item.id?{...a,institution:item.name}:a)))
        return item
      }
      if(command==='create_account'||command==='update_account'){
        let institution=institutions().find((i:any)=>i.id===args.input.institution_id)
        if(!institution && args.input.institution){institution={id:crypto.randomUUID(),name:args.input.institution,logo:null,is_archived:false};localStorage.setItem('institutions',JSON.stringify([...institutions(),institution]))}
        const account={...args.input,id:args.input.id??crypto.randomUUID(),current_balance:args.input.opening_balance??'0',institution_id:institution?.id??null,institution:institution?.name??null}
        localStorage.setItem('accounts',JSON.stringify([...accounts().filter((a:any)=>a.id!==account.id),account]));return account
      }
      if(command==='list_transactions'){
        const [a]=accounts();return a?[{id:'t',type:'expense',account_id:a.id,account_name:a.name,destination_account_id:null,destination_account_name:null,amount:'100',date:'2026-09-01',description:'Sample expense',payee_id:null,category_id:null}]:[]
      }
      throw new Error(command)
    }}})
  })
})

test('permanent deletion confirms, handles stale account links, and persists after reload', async ({page}) => {
  await page.goto('/#/settings')
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  const remove=page.getByRole('button',{name:'Delete institution Bangkok Bank Public Company Limited',exact:true})
  await remove.click()
  const dialog=page.getByRole('dialog')
  await expect(dialog).toContainText('This cannot be undone')
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await expect(remove).toBeVisible()
  await remove.click()
  await page.evaluate(()=>localStorage.setItem('accounts',JSON.stringify([{id:'a',name:'Archived',institution_id:'bank-002',is_archived:true}])))
  await dialog.getByRole('button',{name:'Delete permanently',exact:true}).click()
  await expect(dialog.getByRole('alert')).toContainText('linked accounts')
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await page.reload()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await expect(remove).toBeDisabled()
  await page.evaluate(()=>localStorage.setItem('accounts','[]'))
  await page.reload()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await remove.click()
  await dialog.getByRole('button',{name:'Delete permanently',exact:true}).click()
  await expect(dialog).toHaveCount(0)
  await expect(remove).toHaveCount(0)
  await page.reload()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await expect(remove).toHaveCount(0)
})

test('bank search, shared rename, history logos, and custom institution fallback',async({page})=>{
  await page.goto('/#/accounts')
  await page.getByRole('button',{name:'Add account',exact:true}).click()
  await page.getByLabel('Account type',{exact:true}).selectOption('bank')
  await page.getByLabel('Account name',{exact:true}).fill('Everyday')
  await page.getByText('Optional details',{exact:true}).click()
  const picker=page.getByRole('combobox',{name:'Institution (optional)',exact:true})
  await picker.fill('002')
  const bank=page.getByRole('option',{name:'Bangkok Bank Public Company Limited',exact:true})
  await expect(bank.locator('img')).toHaveAttribute('src','/institutions/bbl.svg')
  await picker.press('ArrowDown')
  await picker.press('ArrowUp')
  await picker.press('Enter')
  await expect(picker).toHaveValue('Bangkok Bank Public Company Limited')
  await page.getByRole('button',{name:'Save account',exact:true}).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Settings',exact:true}).click()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await page.getByRole('button',{name:'Edit institution Bangkok Bank Public Company Limited',exact:true}).click()
  await page.getByLabel('Institution name',{exact:true}).fill('My renamed bank')
  await page.getByRole('button',{name:'Save institution',exact:true}).click()
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Transactions',exact:true}).click()
  const row=page.getByRole('row').filter({hasText:'Sample expense'})
  await expect(row).toContainText('My renamed bank')
  await expect(row.locator('img')).toHaveAttribute('src','/institutions/bbl.svg')
  await page.reload()
  await expect(row).toContainText('My renamed bank')
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Accounts',exact:true}).click()
  await page.getByRole('button',{name:'Add account',exact:true}).click()
  await page.getByLabel('Account type',{exact:true}).selectOption('wallet')
  await page.getByLabel('Account name',{exact:true}).fill('Wallet')
  await page.getByText('Optional details',{exact:true}).click()
  await picker.fill('custom tools')
  await page.getByRole('option',{name:'Add “custom tools”',exact:true}).click()
  await expect(page.getByText('New institution will be added when you save this account.')).toBeVisible()
  await page.getByRole('button',{name:'Save account',exact:true}).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  const card=page.getByRole('table',{name:'Accounts',exact:true}).getByRole('row').filter({has:page.getByRole('button',{name:'Edit account Wallet',exact:true})})
  await expect(card).toContainText('custom tools')
  await expect(card.locator('[aria-hidden=true]').first()).toHaveText('C')
  await card.scrollIntoViewIfNeeded()
  await page.screenshot({path:'/tmp/heyday-institutions-accounts.png'})
  await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'Settings',exact:true}).click()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await page.getByRole('button',{name:'Archive institution custom tools',exact:true}).click()
  await expect(page.getByRole('button',{name:'Restore institution custom tools',exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Restore institution custom tools',exact:true}).click()
  await expect(page.getByRole('button',{name:'Archive institution custom tools',exact:true})).toBeVisible()
  await page.getByRole('tab',{name:'General',exact:true}).click()
  await page.getByLabel('Theme',{exact:true}).selectOption('dark')
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await page.screenshot({path:'/tmp/heyday-institutions-dark.png',animations:'disabled'})
})

test('institution selection is available for every non-cash account and preserves drafts',async({page})=>{
  await page.goto('/#/accounts')
  await page.getByRole('button',{name:'Add account',exact:true}).click()
  await page.getByText('Optional details',{exact:true}).click()
  await expect(page.getByLabel('Institution (optional)',{exact:true})).toHaveCount(0)
  for(const kind of ['bank','wallet','credit_card','loan','investment']){
    await page.getByLabel('Account type',{exact:true}).selectOption(kind)
    await expect(page.getByRole('combobox',{name:'Institution (optional)',exact:true})).toBeVisible()
  }
  await page.getByLabel('Account name',{exact:true}).fill('Draft')
  await page.getByLabel('Institution (optional)',{exact:true}).fill('Unselected custom')
  await page.getByRole('button',{name:'Save account',exact:true}).click()
  await expect(page.getByRole('alert')).toContainText('Select or add an institution')
  await page.getByRole('button',{name:'Cancel',exact:true}).click()
  await expect(page.getByRole('alert').filter({hasText:'Discard your unsaved account'})).toBeVisible()
  await page.getByRole('button',{name:'Keep editing',exact:true}).click()
  await expect(page.getByLabel('Institution (optional)',{exact:true})).toHaveValue('Unselected custom')
  await page.setViewportSize({width:760,height:560})
  await expect.poll(() => page.getByRole('dialog').evaluate(e => { const rect=e.getBoundingClientRect(); return rect.top >= 0 && rect.bottom <= innerHeight })).toBe(true)
  await page.getByLabel('Institution (optional)',{exact:true}).fill('BBL')
  await page.getByRole('option',{name:'Bangkok Bank Public Company Limited',exact:true}).scrollIntoViewIfNeeded()
  await expect(page.getByRole('button',{name:'Save account',exact:true})).toBeInViewport()
  await page.screenshot({path:'/tmp/heyday-institutions-picker.png',animations:'disabled'})
})
