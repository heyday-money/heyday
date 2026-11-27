import { invoke } from '@tauri-apps/api/core'
import { t } from './i18n'
import type { LogoChange } from './logos'
export const trackingTypes = [
 { value:'receivable',get label(){return t('Receivable')} },{value:'vehicle',get label(){return t('Vehicle')}},
 {value:'property',get label(){return t('Property')}},{value:'insurance_cash_value',get label(){return t('Insurance cash value')}},
 {value:'other_asset',get label(){return t('Other asset')}},
] as const
export type TrackingSubtype = typeof trackingTypes[number]['value']
export interface TrackingDraft {name:string;subtype:TrackingSubtype;currency:string;opening_value:string;opening_date:string;notes:string|null;logo_asset_id:string|null;debtor:string|null;due_date:string|null;linked_loan_id:string|null}
export type TrackingAction = {kind:'value';value:string}|{kind:'lend'|'write_off';principal:string}|{kind:'receive';principal:string;interest:string}|{kind:'buy';cost:string}|{kind:'sell';proceeds:string}|{kind:'reverse';event_id:string}
export interface TrackingEvent {id:string;revision:string;date:string;notes:string;previous_value:string;next_value:string;action:TrackingAction;cash_account_id:string|null}
export interface TrackingAccount {id:string;draft:TrackingDraft;current_value:string;revision:string;latest_valuation_date:string;archived:boolean;events:TrackingEvent[]}
export const getTracking = (accountId:string) => invoke<TrackingAccount>('get_tracking_account',{accountId})
function changed(){for(const event of ['accounts-changed','transactions-changed','institutions-changed'])window.dispatchEvent(new Event(event))}
export async function saveTracking(input:TrackingDraft,existing:TrackingAccount|null,logoChange?:LogoChange){const result=await invoke<TrackingAccount>(existing?'edit_tracking_account':'create_tracking_account',{input,accountId:existing?.id,expectedRevision:existing?.revision,logoChange});changed();return result}
export async function recordTracking(input:{id:string;account_id:string;expected_revision:string;currency:string;date:string;notes:string;action:TrackingAction;cash_account_id:string|null}){const result=await invoke<TrackingAccount>('record_tracking_event',{input});changed();return result}
export function activeTrackingEvents(events:TrackingEvent[]){const reversed=new Set(events.flatMap(e=>e.action.kind==='reverse'?[e.action.event_id]:[]));return events.filter(e=>e.action.kind!=='reverse'&&!reversed.has(e.id))}
export function trackingGain(event:TrackingEvent):{kind:'valuation'|'realized';amount:bigint}|null {if(event.action.kind==='value')return {kind:'valuation',amount:BigInt(event.next_value)-BigInt(event.previous_value)};if(event.action.kind==='sell')return {kind:'realized',amount:BigInt(event.action.proceeds)-BigInt(event.previous_value)};if(event.action.kind==='write_off')return {kind:'realized',amount:-BigInt(event.action.principal)};return null}

// Calendar-day age avoids DST and timezone-dependent warning boundaries.
export function valuationNeedsReview(state: TrackingAccount, today: string): boolean {
 if (state.archived || state.draft.subtype === 'receivable' || activeTrackingEvents(state.events).at(-1)?.action.kind === 'sell') return false
 return Date.parse(today + 'T00:00:00Z') - Date.parse(state.latest_valuation_date + 'T00:00:00Z') >= 90 * 86400000
}
export function trackingPreview(state: TrackingAccount, action: TrackingAction): {value: bigint; cash: bigint; cashAccountId: string | null} {
 const current = BigInt(state.current_value)
 if (action.kind === 'reverse') {
  const event = state.events.find(e => e.id === action.event_id)
  if (!event || event.action.kind === 'reverse') throw Error('Use the latest active action to reverse.')
  const original = trackingPreview({...state, current_value: event.previous_value}, event.action)
  return {value: BigInt(event.previous_value), cash: -original.cash, cashAccountId: event.cash_account_id}
 }
 switch (action.kind) {
  case 'value': return {value: BigInt(action.value), cash: 0n, cashAccountId: null}
  case 'lend': return {value: current + BigInt(action.principal), cash: -BigInt(action.principal), cashAccountId: null}
  case 'receive': return {value: current - BigInt(action.principal), cash: BigInt(action.principal) + BigInt(action.interest), cashAccountId: null}
  case 'write_off': return {value: current - BigInt(action.principal), cash: 0n, cashAccountId: null}
  case 'buy': return {value: BigInt(action.cost), cash: -BigInt(action.cost), cashAccountId: null}
  case 'sell': return {value: 0n, cash: BigInt(action.proceeds), cashAccountId: null}
 }
}
