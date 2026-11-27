import { toast } from 'sonner'
import { useEffect,useState,type FormEvent } from 'react'
import { t,useLanguage } from '../lib/i18n'
import { listAccounts,type Account } from '../lib/desktop'
import { getTracking,saveTracking,trackingTypes,type TrackingAccount,type TrackingDraft,type TrackingSubtype } from '../lib/tracking'
import { decimalToInteger,fractionDigits } from '../lib/money'
import { dateKey } from '../lib/financial'
import { LogoPicker } from './LogoPicker'
import { useLogoAsset } from './LogoImage'
import type { LogoChange } from '../lib/logos'
import { Dialog,DialogContent,DialogHeader,DialogTitle,DialogDescription,DialogFooter } from './ui/dialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { Textarea } from './ui/textarea'
import { FormField as Field } from './FormField'
export function decimal(value:string,currency:string){const digits=fractionDigits(currency),n=BigInt(value),scale=10n**BigInt(digits);return `${n/scale}${digits?'.'+(n%scale).toString().padStart(digits,'0'):''}`}
export function TrackingFormDialog({account,currency,onClose,onSaved,onOtherType}:{account?:Account;currency:string;onClose:()=>void;onSaved:(account:Account)=>void;onOtherType?:()=>void}){
 useLanguage();const [stored,setStored]=useState<TrackingAccount|null>(null),[accounts,setAccounts]=useState<Account[]>([]),[attempt,setAttempt]=useState(0),[loadFailed,setLoadFailed]=useState(false),[loading,setLoading]=useState(true),[error,setError]=useState(''),[saving,setSaving]=useState(false),[dirty,setDirty]=useState(false),[discard,setDiscard]=useState(false),[switching,setSwitching]=useState(false),[logo,setLogo]=useState<LogoChange>(),[logoBusy,setLogoBusy]=useState(false)
 const [subtype,setSubtype]=useState<TrackingSubtype>('receivable');const image=useLogoAsset(stored?.draft.logo_asset_id)
 useEffect(()=>{let active=true;setLoading(true);setLoadFailed(false);Promise.all([listAccounts(true,true),account?getTracking(account.id):Promise.resolve(null)]).then(([a,s])=>{if(active){setError('');setAccounts(a);setStored(s);if(s)setSubtype(s.draft.subtype);setLoading(false)}}).catch(e=>{if(active){setError(String(e));setLoadFailed(true);setLoading(false)}});return()=>{active=false}},[account?.id,attempt])
 const busy=saving||logoBusy;const close=()=>{if(!busy){setSwitching(false);if(dirty)setDiscard(true);else onClose()}}
 async function save(e:FormEvent<HTMLFormElement>){e.preventDefault();const data=new FormData(e.currentTarget);const text=(key:string)=>String(data.get(key)??'').trim();setSaving(true);setError('');try{const input:TrackingDraft={name:text('name'),subtype,currency,opening_value:stored?.draft.opening_value??decimalToInteger(text('value'),fractionDigits(currency)),opening_date:stored?.draft.opening_date??text('date'),notes:text('notes')||null,logo_asset_id:stored?.draft.logo_asset_id??null,debtor:subtype==='receivable'?text('debtor')||null:null,due_date:subtype==='receivable'?text('due')||null:null,linked_loan_id:['vehicle','property','other_asset'].includes(subtype)?text('loan')||null:null};const saved=await saveTracking(input,stored,logo);setStored(saved);setLogo(undefined);const all=await listAccounts(true,true);const result=all.find(a=>a.id===saved.id);if(!result)throw Error('Could not load accounts.');onSaved(result);onClose();toast.success(stored?t('Account updated.'):t('Account added.'))}catch(e){setError(String(e))}finally{setSaving(false)}}
 return <Dialog open onOpenChange={open=>{if(!open)close()}}><DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-[640px]" onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(busy)e.preventDefault()}} showCloseButton={!busy}><DialogHeader><DialogTitle>{account?t('Edit Tracking account'):t('Add Tracking account')}</DialogTitle><DialogDescription>{t('Track assets in Net Worth without adding spendable cash. Tracking details and history are included in backups and encrypted financial sync.')}</DialogDescription></DialogHeader>
 {loading?<p role="status">{t('Loading accounts…')}</p>:!loadFailed&&(!account||stored)&&<form key={stored?.id??'new'} onSubmit={save} onChange={()=>setDirty(true)} aria-label={t('Tracking account')}><fieldset disabled={busy}><div className="grid gap-4 sm:grid-cols-2">
 {!account&&<Field label={t('Account type')}><NativeSelect value="tracking" onChange={()=>{if(dirty){setSwitching(true);setDiscard(true)}else onOtherType?.()}}><option value="tracking">{t('Tracking')}</option><option value="other">{t('Other account type')}</option></NativeSelect></Field>}
 <Field label={t('Asset type')}><NativeSelect value={subtype} disabled={!!stored} onChange={e=>setSubtype(e.target.value as TrackingSubtype)}>{trackingTypes.map(x=><option key={x.value} value={x.value}>{x.label}</option>)}</NativeSelect></Field>
 <Field label={t('Account name')}><Input name="name" required maxLength={100} defaultValue={stored?.draft.name??''} autoFocus /></Field>
 <Field label={t('Opening value')}><Input name="value" required inputMode="decimal" readOnly={!!stored} defaultValue={stored?decimal(stored.draft.opening_value,currency):'0'} /></Field>
 <Field label={t('Opening date')}><Input name="date" type="date" required readOnly={!!stored} max={dateKey(new Date())} defaultValue={stored?.draft.opening_date??dateKey(new Date())} /></Field>
 {subtype==='receivable'&&<><Field label={t('Debtor (optional)')}><Input name="debtor" maxLength={100} defaultValue={stored?.draft.debtor??''}/></Field><Field label={t('Due date (optional)')}><Input name="due" type="date" defaultValue={stored?.draft.due_date??''}/></Field></>}
 {['vehicle','property','other_asset'].includes(subtype)&&<Field label={t('Linked loan (optional)')}><NativeSelect name="loan" defaultValue={stored?.draft.linked_loan_id??''}><option value="">{t('None')}</option>{accounts.filter(a=>a.type==='loan'&&(!a.is_archived||a.id===stored?.draft.linked_loan_id)).map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></Field>}
 </div><div className="mt-4"><LogoPicker name={stored?.draft.name??t('Tracking')} savedSource={image} value={logo} hasSavedLogo={!!stored?.draft.logo_asset_id} onChange={l=>{setLogo(l);setDirty(true)}} onBusyChange={setLogoBusy}/></div>
 <Field label={t('Notes')}><Textarea name="notes" maxLength={1000} defaultValue={stored?.draft.notes??''}/></Field>
 <p className="my-3 text-sm text-muted">{subtype==='insurance_cash_value'?t('Enter cash surrender value only. Coverage and premiums do not increase this asset.'):t('Opening values are locked after creation. Record dated actions to change the current value.')}</p>
 <DialogFooter><Button type="button" variant="outline" onClick={close}>{t('Cancel')}</Button><Button type="submit">{saving?t('Saving…'):t('Save account')}</Button></DialogFooter></fieldset></form>}
 {error&&<div role="alert"><p className="text-red-600">{t(error.replace(/^Error: /,''))}</p>{loadFailed&&<Button onClick={()=>setAttempt(x=>x+1)}>{t('Retry')}</Button>}</div>}
 {discard&&<Dialog open onOpenChange={setDiscard}><DialogContent><DialogHeader><DialogTitle>{t('Discard changes?')}</DialogTitle></DialogHeader><DialogFooter><Button variant="outline" onClick={()=>setDiscard(false)}>{t('Keep editing')}</Button><Button onClick={()=>switching?onOtherType?.():onClose()}>{t('Discard changes')}</Button></DialogFooter></DialogContent></Dialog>}
 </DialogContent></Dialog>
}
