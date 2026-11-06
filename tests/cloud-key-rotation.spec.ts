import { expect, test, type Page } from '@playwright/test'
test.use({ channel:'chrome' })
const recovery='HDK1.public-test-recovery-key.only-a-ui-fixture', code='HDV1.public-test-history-code'
async function setup(page:Page,initial='unlocked',verified=true) {
  await page.addInitScript(({initial,verified,recovery,code})=>{
    let state=initial, historyVerified=verified, unverifiedHistory=false, rotationPending=false
    const sync={bound:true,enabled:false,bootstrapped:true,financialEnabled:true,incomeEnabled:true,plannerEnabled:true,fullEnabled:true,pending:2,conflictCount:0,lastSuccess:'2026-11-01T00:00:00Z',conflicts:[]}
    const status=()=>({state,historyVerified,unverifiedHistory,rotationPending,rememberEncryption:true})
    Object.defineProperty(window,'isTauri',{value:true})
    Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:async(command:string,args:Record<string,unknown>={})=>{
      if(command==='get_settings') return {currency:null,period_start_day:1}
      if(command==='plugin:app|version') return 'test'
      if(command==='cloud_account') return {account:{id:'alice',email:'alice@example.test'},online:true,deviceRegistered:true}
      if(command==='cloud_sync_status') return sync
      if(command==='cloud_sync_now'||command==='cloud_enable_sync') { sessionStorage.setItem('unexpected-sync','yes');return sync }
      if(command==='cloud_encryption_status') return status()
      if(command==='cloud_cancel_encryption') return
      if(command==='cloud_prepare_key_rotation') {sessionStorage.setItem('prepared','yes'); return {recoveryKey:recovery}}
      if(command==='cloud_confirm_key_rotation') {
        if(!args.saved||args.recoveryKey!==recovery) throw 'The recovery key is incorrect or the encrypted key package was changed.'
        rotationPending=true
        if(sessionStorage.getItem('network-error')) throw 'Could not reach encryption setup. Check your connection and try again.'
        rotationPending=false;sessionStorage.setItem('rotated','yes');return status()
      }
      if(command==='cloud_cancel_key_rotation') {rotationPending=false;sessionStorage.setItem('replacement-cancelled','yes');return status()}
      if(command==='cloud_recover_encryption') {
        if(args.recoveryKey!==recovery) throw 'The recovery key is incorrect or the encrypted key package was changed.'
        if(!historyVerified&&!unverifiedHistory) {
          if(args.historyCode===code) historyVerified=true
          else if(args.allowUnverifiedHistory===true&&!args.historyCode) unverifiedHistory=true
          else throw 'Verify cloud history with a code from a trusted device before syncing.'
        }
        state='unlocked';sessionStorage.setItem('recovered','yes');return status()
      }
      if(command==='cloud_history_verification_code') return code
      if(command==='cloud_save_recovery_key') return true
      throw new Error(`Unexpected command: ${command}`)
    }}})
  },{initial,verified,recovery,code})
  await page.goto('/#/settings?tab=account')
  await page.getByRole('tab',{name:'Heyday account',exact:true}).click()
  await page.getByText('Advanced security',{exact:true}).click()
}
test('rotation requires saving the new key, supports durable retry and preserves paused sync',async({page})=>{
  await setup(page)
  await page.getByRole('button',{name:'Replace recovery key',exact:true}).click()
  const dialog=page.getByRole('dialog'),submit=dialog.getByRole('button',{name:'Replace key and rotate encryption',exact:true})
  await expect(submit).toBeDisabled()
  await expect(dialog).toContainText('Other devices must restore with the new key')
  await dialog.getByLabel('Re-enter your saved recovery key',{exact:true}).fill(recovery)
  await dialog.getByRole('checkbox',{name:'I saved my recovery key and understand that support cannot recover my encrypted data.',exact:true}).check()
  await page.screenshot({path:'test-results/encryption-rotation.png',fullPage:true,animations:'disabled'})
  await page.evaluate(()=>sessionStorage.setItem('network-error','yes'))
  await submit.click();await expect(dialog.getByRole('alert')).toContainText('Could not reach encryption')
  await expect(dialog.getByLabel('Re-enter your saved recovery key',{exact:true})).toHaveValue(recovery)
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click()
  await page.getByRole('button',{name:'Finish key replacement',exact:true}).click()
  await expect(page.locator('#generated-recovery')).toHaveCount(0)
  await dialog.getByLabel('Re-enter your saved recovery key',{exact:true}).fill(recovery)
  await dialog.getByRole('checkbox',{name:'I saved my recovery key and understand that support cannot recover my encrypted data.',exact:true}).check()
  await page.evaluate(()=>sessionStorage.removeItem('network-error'))
  await submit.click();await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button',{name:'Resume sync',exact:true})).toBeVisible()
  expect(await page.evaluate(()=>sessionStorage.getItem('unexpected-sync'))).toBeNull()
  expect(await page.evaluate(()=>JSON.stringify({...sessionStorage,...localStorage}))).not.toContain(recovery)
})
test('new-device recovery requires a trusted code or a separate explicit acknowledgment',async({page})=>{
  await setup(page,'locked',false)
  await page.getByRole('button',{name:'Restore encryption key',exact:true}).click()
  const dialog=page.getByRole('dialog'),submit=dialog.getByRole('button',{name:'Restore encryption key',exact:true})
  await dialog.getByLabel('Recovery key',{exact:true}).fill(recovery)
  await expect(submit).toBeDisabled()
  await page.screenshot({path:'test-results/encryption-history-recovery.png',fullPage:true,animations:'disabled'})
  await dialog.getByLabel('History verification code',{exact:true}).fill('invalid-code')
  await submit.click();await expect(dialog.getByRole('alert')).toContainText('Verify cloud history')
  await dialog.getByLabel('History verification code',{exact:true}).fill(code)
  await submit.click();await expect(dialog).toHaveCount(0)
  await expect(page.getByText('History recovered without independent verification',{exact:true})).toHaveCount(0)
  expect(await page.evaluate(()=>sessionStorage.getItem('unexpected-sync'))).toBeNull()
})
test('lost-device recovery retains an honest history notice and propagates it to verification codes',async({page})=>{
  await setup(page,'locked',false)
  await page.getByRole('button',{name:'Restore encryption key',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await dialog.getByLabel('Recovery key',{exact:true}).fill(recovery)
  await dialog.getByRole('checkbox',{name:'I no longer have a trusted device or verification code.',exact:false}).check()
  await dialog.getByRole('button',{name:'Restore encryption key',exact:true}).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('History recovered without independent verification',{exact:true})).toBeVisible()
  await expect(page.getByRole('button',{name:'Resume sync',exact:true})).toBeEnabled()
  await page.getByRole('button',{name:'Verify another device',exact:true}).click()
  await expect(dialog.getByRole('textbox',{name:'History verification code',exact:true})).toHaveValue(code)
  await expect(dialog).toContainText('The new device will keep the same history notice.')
  expect(await page.evaluate(()=>JSON.stringify({...sessionStorage,...localStorage}))).not.toContain(code)
})
test('a rotated device asks for the latest recovery key without repeating sync consent',async({page})=>{
  await setup(page,'changed',true)
  await expect(page.getByText('Encryption changed on another device. Restore with the latest recovery key.',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Restore encryption key',exact:true}).click()
  const dialog=page.getByRole('dialog')
  await expect(dialog.getByLabel('History verification code',{exact:true})).toHaveCount(0)
  await dialog.getByLabel('Recovery key',{exact:true}).fill(recovery)
  await dialog.getByRole('button',{name:'Restore encryption key',exact:true}).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('button',{name:'Resume sync',exact:true})).toBeVisible()
})
