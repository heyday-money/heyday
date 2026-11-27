import { test, expect } from '@playwright/test'
import { valuationNeedsReview, trackingPreview, type TrackingAccount } from '../src/lib/tracking'
const asset: TrackingAccount = {id:'car',draft:{name:'Car',subtype:'vehicle',currency:'THB',opening_value:'60000000',opening_date:'2026-01-01',notes:null,logo_asset_id:null,debtor:null,due_date:null,linked_loan_id:null},current_value:'60000000',revision:'0',latest_valuation_date:'2026-01-01',archived:false,events:[]}
test('valuation reminder uses calendar days and excludes closed assets and principal',()=>{
 expect(valuationNeedsReview(asset,'2026-03-31')).toBe(false)
 expect(valuationNeedsReview(asset,'2026-04-01')).toBe(true)
 expect(valuationNeedsReview({...asset,archived:true},'2026-04-01')).toBe(false)
 expect(valuationNeedsReview({...asset,draft:{...asset.draft,subtype:'receivable'}},'2026-04-01')).toBe(false)
 expect(valuationNeedsReview({...asset,latest_valuation_date:'2026-03-31'},'2026-04-01')).toBe(false)
})
test('cash previews retain exact large amounts and reverse both principal and interest',()=>{
 const state={...asset,current_value:'9007199254740993'}
 expect(trackingPreview(state,{kind:'value',value:'9007199254740992'})).toMatchObject({value:9007199254740992n,cash:0n})
 expect(trackingPreview(asset,{kind:'sell',proceeds:'55000000'})).toMatchObject({value:0n,cash:55000000n})
 const event={id:'paid',revision:'1',date:'2026-01-01',notes:'',previous_value:'1000000',next_value:'800000',action:{kind:'receive' as const,principal:'200000',interest:'10000'},cash_account_id:'bank'}
 expect(trackingPreview({...asset,current_value:'800000',events:[event]},{kind:'reverse',event_id:'paid'})).toEqual({value:1000000n,cash:-210000n,cashAccountId:'bank'})
})
