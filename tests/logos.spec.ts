import { test, expect, type Page } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'isTauri', { value: true })
    const read = (key: string, fallback: any) => JSON.parse(localStorage.getItem(key) ?? JSON.stringify(fallback))
    const institutions = () => read('logo-institutions', [{id:'bank-002',name:'Bangkok Bank',logo:'bbl.svg',logo_mode:'default',logo_asset_id:null,is_archived:false}])
    const payees = () => read('logo-payees', [{id:'shop',name:'Corner Shop',is_archived:false,logo_asset_id:null}])
    const accounts = [{id:'bank',name:'Everyday',type:'bank',institution:'Bangkok Bank',institution_id:'bank-002',current_balance:'1000',opening_balance:'1000'}]
    const setLogo = (item: any, change: any) => {
      if (!change) return
      item.logo_mode = change.kind
      item.logo_asset_id = change.kind === 'custom' ? crypto.randomUUID() : null
      if (item.logo_asset_id) { const assets=read('logo-assets',{}); assets[item.logo_asset_id]=change.data; localStorage.setItem('logo-assets',JSON.stringify(assets)) }
    }
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:any)=>{
      if(command==='plugin:app|version')return '0.0.1-alpha.5'
      if(command==='get_settings')return {currency:'THB',period_start_day:1}
      if(command==='list_institutions')return {institutions:institutions(),accounts}
      if(command==='list_accounts')return accounts
      if(command==='list_transaction_options')return {payees:payees(),categories:[]}
      if(command==='get_logo_asset')return read('logo-assets',{})[args.id]
      if(command==='save_institution'||command==='save_transaction_option'){
        if(sessionStorage.getItem('fail-logo'))throw 'Could not save logo. Try again.'
        const key=command==='save_institution'?'logo-institutions':'logo-payees'
        const rows=command==='save_institution'?institutions():payees()
        const {logo_change,...input}=args.input
        const item={...rows.find((i:any)=>i.id===input.id),...input,id:input.id??crypto.randomUUID()}
        setLogo(item,logo_change)
        localStorage.setItem(key,JSON.stringify([...rows.filter((i:any)=>i.id!==item.id),item]))
        return item
      }
      if(command==='list_transactions')return [{id:'t',type:'expense',account_id:'bank',account_name:'Everyday',amount:'100',date:'2026-09-01',description:'Lunch',payee_id:'shop',payee_name:payees()[0].name,destination_account_id:null,category_id:null}]
      throw new Error(command)
    }}})
  })
})
async function source(page: Page, mimeType = 'image/png') {
  const data = await page.evaluate(mime => {
    const canvas=document.createElement('canvas');canvas.width=512;canvas.height=256
    const ctx=canvas.getContext('2d')!;ctx.fillStyle='#8942fe';ctx.fillRect(100,50,300,150)
    return canvas.toDataURL(mime).split(',')[1]
  },mimeType)
  return { name: mimeType==='image/png'?'logo.png':'logo.jpg', mimeType, buffer:Buffer.from(data,'base64') }
}

test('SVG logos rasterize with transparency and cannot execute scripts or load remote resources', async ({ page }) => {
  await page.goto('/#/settings')
  const requests: string[] = []
  page.on('request', request => { if (request.url().includes('svg-external.invalid')) requests.push(request.url()) })
  for (const kind of ['Institutions', 'Payees']) {
    await page.getByRole('tab', { name: kind, exact: true }).click()
    await page.getByRole('button', { name: kind === 'Institutions' ? 'Edit institution Bangkok Bank' : 'Edit Corner Shop', exact: true }).click()
    const dialog = page.getByRole('dialog')
    await dialog.getByLabel('Choose logo file').setInputFiles({ name: 'logo.svg', mimeType: 'image/svg+xml', buffer: Buffer.from(`<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 200"><script>window.svgExecuted=true</script><image href="https://svg-external.invalid/logo.png"/><rect x="100" y="50" width="200" height="100" fill="#8942fe"/></svg>`) })
    await expect(dialog.locator('img')).toHaveAttribute('src', /^data:image\/png;base64,/)
    await expect.poll(() => dialog.locator('img').evaluate((img: HTMLImageElement) => [img.naturalWidth, img.naturalHeight])).toEqual([128, 64])
    const pixels = await dialog.locator('img').evaluate((img: HTMLImageElement) => {
      const canvas = document.createElement('canvas'); canvas.width = 128; canvas.height = 64
      const context = canvas.getContext('2d')!; context.drawImage(img, 0, 0)
      return [context.getImageData(0, 0, 1, 1).data[3], ...context.getImageData(64, 32, 1, 1).data]
    })
    expect(pixels).toEqual([0, 137, 66, 254, 255])
    await dialog.getByRole('button', { name: kind === 'Institutions' ? 'Save institution' : 'Save', exact: true }).click()
    await expect(dialog).toHaveCount(0)
  }
  expect(requests).toEqual([])
  expect(await page.evaluate(() => (window as any).svgExecuted)).toBeUndefined()
  await page.reload()
  await page.getByRole('tab', { name: 'Payees', exact: true }).click()
  await expect(page.getByRole('listitem').filter({ hasText: 'Corner Shop' }).locator('img')).toHaveAttribute('src', /^data:image\/png;base64,/)
})

test('institution logos resize, protect drafts, persist, remove and restore the bundled default',async({page})=>{
  await page.goto('/#/settings')
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await page.getByRole('button',{name:'Edit institution Bangkok Bank',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await dialog.getByLabel('Choose logo file').setInputFiles(await source(page))
  await expect.poll(()=>dialog.locator('img').evaluate((img:HTMLImageElement)=>[img.naturalWidth,img.naturalHeight])).toEqual([128,64])
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await expect(dialog.getByRole('alert')).toContainText('Discard your unsaved institution')
  await dialog.getByRole('button',{name:'Discard changes',exact:true}).click()
  await expect(page.locator('img[src^="data:"]')).toHaveCount(0)
  await page.getByRole('button',{name:'Edit institution Bangkok Bank',exact:true}).click()
  await dialog.getByLabel('Choose logo file').setInputFiles(await source(page,'image/jpeg'))
  await expect(dialog.locator('img')).toHaveAttribute('src',/^data:image\/png;base64,/)
  await page.evaluate(()=>sessionStorage.setItem('fail-logo','yes'))
  await dialog.getByRole('button',{name:'Save institution',exact:true}).click()
  await expect(dialog.getByRole('alert')).toContainText('Could not save logo')
  await expect(dialog.locator('img')).toHaveAttribute('src',/^data:/)
  await page.evaluate(()=>sessionStorage.removeItem('fail-logo'))
  await dialog.getByRole('button',{name:'Save institution',exact:true}).click()
  await page.reload()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await expect(page.locator('li').filter({hasText:'Bangkok Bank'}).locator('img')).toHaveAttribute('src',/^data:/)
  await page.getByRole('button',{name:'Edit institution Bangkok Bank',exact:true}).click()
  await expect(dialog.getByRole('button',{name:'Use default logo',exact:true})).toBeVisible()
  await dialog.getByRole('button',{name:'Remove logo',exact:true}).click()
  await expect(dialog.locator('img')).toHaveCount(0)
  await dialog.getByRole('button',{name:'Save institution',exact:true}).click()
  await page.reload()
  await page.getByRole('tab',{name:'Institutions',exact:true}).click()
  await expect(page.locator('li').filter({hasText:'Bangkok Bank'}).locator('img')).toHaveCount(0)
  await page.getByRole('button',{name:'Edit institution Bangkok Bank',exact:true}).click()
  await dialog.getByRole('button',{name:'Use default logo',exact:true}).click()
  await dialog.getByRole('button',{name:'Save institution',exact:true}).click()
  await expect(page.locator('li').filter({hasText:'Bangkok Bank'}).locator('img')).toHaveAttribute('src','/institutions/bbl.svg')
})

test('payee logos persist through rename/archive, appear in history and entry, and reject invalid images',async({page})=>{
  await page.goto('/#/settings')
  await page.getByRole('tab',{name:'Payees',exact:true}).click()
  await page.getByRole('button',{name:'Edit Corner Shop',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await dialog.getByLabel('Choose logo file').setInputFiles({name:'bad.svg',mimeType:'image/svg+xml',buffer:Buffer.from('<svg/>')})
  await expect(dialog.getByRole('alert')).toContainText('This SVG could not be read')
  await dialog.getByLabel('Choose logo file').setInputFiles({name:'bad.png',mimeType:'image/png',buffer:Buffer.from([137,80,78,71,0])})
  await expect(dialog.getByRole('alert')).toContainText('could not be read')
  await dialog.getByLabel('Choose logo file').setInputFiles({name:'big.png',mimeType:'image/png',buffer:Buffer.alloc(5*1024*1024+1)})
  await expect(dialog.getByRole('alert')).toContainText('smaller than 5 MB')
  await dialog.getByLabel('Choose logo file').setInputFiles(await source(page))
  await expect(dialog.locator('img')).toHaveAttribute('src',/^data:/)
  await dialog.getByLabel('Payee name',{exact:true}).fill('New Shop')
  await page.setViewportSize({width:760,height:560})
  await expect(dialog.getByRole('button',{name:'Save',exact:true})).toBeVisible()
  await page.screenshot({path:'/tmp/heyday-custom-logo.png',animations:'disabled'})
  await dialog.getByRole('button',{name:'Save',exact:true}).click()
  await page.getByRole('button',{name:'Archive New Shop',exact:true}).click()
  await page.getByRole('button',{name:'Restore New Shop',exact:true}).click()
  await page.reload()
  await page.getByRole('tab',{name:'Payees',exact:true}).click()
  await expect(page.getByRole('listitem').filter({hasText:'New Shop'}).locator('img')).toHaveAttribute('src',/^data:/)
  await page.getByRole('navigation').getByRole('link',{name:'Transactions',exact:true}).click()
  await expect(page.getByRole('row').filter({hasText:'Lunch'}).getByRole('cell').filter({hasText:'New Shop'}).locator('img')).toHaveAttribute('src',/^data:/)
  await page.getByRole('button',{name:'Add transaction',exact:true}).click()
  await page.getByLabel('Payee (optional)',{exact:true}).fill('New')
  await expect(page.getByRole('option',{name:'New Shop',exact:true}).locator('img')).toHaveAttribute('src',/^data:/)
})
