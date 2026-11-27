import { getTracking, type TrackingAccount } from '../lib/tracking'
import { useEffect,useState,type FormEvent } from 'react'
import { Link,useSearch } from '@tanstack/react-router'
import { toast } from 'sonner'
import { t,useLanguage } from '../lib/i18n'
import { listAccounts,getFinancialData,type Account,type FinancialData } from '../lib/desktop'
import { getPlanner,type PlannerData } from '../lib/cashflow'
import { listInsurance,saveInsurance,payInsurance,reverseInsurance,premiumOccurrences,type Policy,type PremiumSchedule,type Occurrence,type PremiumPayment,type PolicyDetails } from '../lib/insurance'
import { dateKey } from '../lib/financial'
import { decimalToInteger,fractionDigits,formatAmount } from '../lib/money'
import { decimal } from './TrackingFormDialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { Textarea } from './ui/textarea'
import { FormField as Field } from './FormField'
import { Dialog,DialogContent,DialogHeader,DialogTitle,DialogDescription,DialogFooter } from './ui/dialog'
const types={life:'Life insurance',health:'Health insurance',motor:'Motor insurance',property:'Property insurance',travel:'Travel insurance',other:'Other insurance'}
const statuses={active:'Active',expired:'Expired',cancelled:'Cancelled',archived:'Archived'}
const frequencies={once:'One time',monthly:'Monthly',quarterly:'Quarterly',yearly:'Yearly'}
const label=(map:Record<string,string>,v:string)=>t(map[v]??v)
interface Data {tracking:TrackingAccount[];policies:Policy[];accounts:Account[];financial:FinancialData;planner:PlannerData}
export function InsurancePage(){
 useLanguage();const [data,setData]=useState<Data|null>(null),[error,setError]=useState(''),[attempt,setAttempt]=useState(0),[editor,setEditor]=useState<Policy|null|undefined>(),[payment,setPayment]=useState<{policy:Policy;occurrence?:Occurrence;payment?:PremiumPayment}|null>(null)
 const search=useSearch({strict:false}) as {asset?:string},today=dateKey(new Date()),until=`${Math.min(Number(today.slice(0,4))+2,9999)}-01-01`
 useEffect(()=>{let active=true;const load=()=>Promise.all([listInsurance(),listAccounts(true,true),getFinancialData(),getPlanner()]).then(async([policies,accounts,financial,planner])=>{const tracking=await Promise.all(accounts.filter(a=>a.type==='tracking').map(a=>getTracking(a.id)));if(active){setData({tracking,policies,accounts,financial,planner});setError('')}}).catch(e=>{if(active)setError(String(e))});void load();window.addEventListener('plans-changed',load);window.addEventListener('focus',load);return()=>{active=false;window.removeEventListener('plans-changed',load);window.removeEventListener('focus',load)}},[attempt])
 if(error)return <div role="alert"><p>{t(error)}</p><Button onClick={()=>setAttempt(n=>n+1)}>{t('Retry')}</Button></div>
 if(!data)return <p role="status">{t('Loading insurance…')}</p>
 const currency=data.financial.settings.currency
 const policies=data.policies.filter(p=>!search.asset||p.details.asset_ids.includes(search.asset)||p.details.cash_value_id===search.asset)
 return <div className="space-y-6"><div className="flex flex-wrap items-start justify-between gap-4"><div><h2 className="text-2xl font-semibold">{t('Insurance')}</h2><p className="mt-2 text-sm text-muted">{t('Coverage is informational. Only cash surrender values count toward Net Worth. Premiums are expenses, never asset growth.')}</p></div><Button disabled={!currency} onClick={()=>setEditor(null)}>{t('Add policy')}</Button></div>
 {!currency&&<Link to="/settings">{t('Choose your currency first')}</Link>}
 {search.asset&&<Link to="/insurance">{t('All policies')}</Link>}
 {!policies.length&&<p className="rounded-xl border border-line p-6">{t('No insurance policies yet.')}</p>}
 {policies.map(p=>{const d=p.details,horizon=Math.min(9999,Math.max(Number(until.slice(0,4)),Number(d.coverage_start.slice(0,4))+2,...p.schedules.map(s=>Number(s.first_date.slice(0,4))+2))),occurrences=premiumOccurrences(p,d.coverage_start,`${horizon}-12-31`).filter(o=>!o.payment),overdue=occurrences.filter(o=>o.date<today),next=occurrences.find(o=>o.date>=today),state=d.status==='active'&&d.coverage_end&&d.coverage_end<today?'expired':d.status
 return <section key={p.id} className="min-w-0 break-words space-y-4 rounded-2xl border border-line bg-card p-5" aria-label={d.name}><div className="flex flex-wrap items-center justify-between gap-3"><div><h3 className="text-xl font-semibold">{d.name}</h3><p className="text-sm text-muted">{d.insurer} · {label(types,d.policy_type)} · {label(statuses,state)} {d.number_suffix&&`· •••• ${d.number_suffix}`}</p></div><Button variant="outline" onClick={()=>setEditor(p)}>{t('Edit policy')}</Button></div>
 <dl className="grid gap-3 text-sm sm:grid-cols-3">{[[t('Coverage amount'),formatAmount(d.coverage_amount,p.currency)],[t('Deductible'),formatAmount(d.deductible,p.currency)],[t('Coverage dates'),`${d.coverage_start} – ${d.coverage_end??'—'}`],[t('Renewal date'),d.renewal_date??'—'],[t('Next premium'),next?`${next.date} · ${formatAmount(next.schedule.amount,p.currency)}`:'—'],[t('Overdue premiums'),String(overdue.length)]].map(([k,v])=><div key={k}><dt className="text-muted">{k}</dt><dd>{v}</dd></div>)}</dl>
 <div className="flex flex-wrap gap-3">{[...new Set([...d.asset_ids,...(d.cash_value_id?[d.cash_value_id]:[])])].map(id=><Link key={id} to="/accounts/$accountId/details" params={{accountId:id}} className="text-brand">{data.accounts.find(a=>a.id===id)?.name??t('Account unavailable')}</Link>)}</div>
 {d.replaces&&<p className="text-sm">{t('Linked source forecasts are replaced by this policy. Unlinking restores the original forecast.')}</p>}
 {d.notes&&<p className="whitespace-pre-wrap text-sm">{d.notes}</p>}
 <details><summary className="cursor-pointer font-medium">{t('Premium schedule and history')}</summary><p className="my-3 text-xs text-muted">{t('Scheduled dates do not prove payment. Confirm only payments actually made. Reconciled payments cannot be reversed here.')}</p>
 <div tabIndex={0} role="region" aria-label={t('Premium schedule and history')} className="max-h-72 overflow-auto"><table className="w-full text-left text-sm"><thead><tr>{[t('Effective date'),t('Frequency'),t('Amount'),t('Account')].map(x=><th key={x} className="p-2">{x}</th>)}</tr></thead><tbody>{p.schedules.map(s=><tr key={s.id} className="border-t border-line"><td className="p-2">{s.effective_from}</td><td>{label(frequencies,s.frequency)} · {s.first_date}{s.end_date&&` – ${s.end_date}`}</td><td>{formatAmount(s.amount,p.currency)}</td><td>{data.accounts.find(a=>a.id===s.account_id)?.name}</td></tr>)}</tbody></table></div>
 {!p.schedules.length&&<p>{t('No premium schedule.')}</p>}
 <ul className="my-4 max-h-80 divide-y divide-line overflow-auto">{occurrences.slice(0,120).map(o=><li key={o.schedule.id+o.date} className="flex flex-wrap items-center justify-between gap-3 py-2"><span>{o.date} · {formatAmount(o.schedule.amount,p.currency)} {o.date<today&&<span className="text-red-600">{t('Overdue')}</span>}</span><Button size="sm" disabled={d.status==='archived'} onClick={()=>setPayment({policy:p,occurrence:o})}>{t('Record premium')}</Button></li>)}</ul>
 <h4 className="font-semibold">{t('Payment history')}</h4>{!p.payments.length&&<p>{t('No recorded premiums.')}</p>}<ul className="max-h-80 divide-y divide-line overflow-auto">{[...p.payments].reverse().map(v=><li key={v.id} className="flex flex-wrap items-center justify-between gap-3 py-2"><div>{v.date} · {formatAmount(v.amount,p.currency)}<p className="text-xs text-muted">{t('Due date')}: {v.due_date} · {v.reversal_id?t('Reversed'):t('Paid')}</p>{v.notes&&<p className="text-sm">{v.notes}</p>}</div>{!v.reversal_id&&<Button size="sm" variant="outline" disabled={d.status==='archived'} onClick={()=>setPayment({policy:p,payment:v})}>{t('Reverse premium')}</Button>}</li>)}</ul>
 </details></section>})}
 {editor!==undefined&&currency&&<PolicyDialog policy={editor} data={data} currency={currency} close={()=>setEditor(undefined)} saved={()=>setAttempt(n=>n+1)}/>}
 {payment&&<PremiumDialog {...payment} accounts={data.accounts} close={()=>setPayment(null)} saved={()=>setAttempt(n=>n+1)}/>}
 </div>
}
function PolicyDialog({policy,data,currency,close,saved}:{policy:Policy|null;data:Data;currency:string;close:()=>void;saved:()=>void}){
 const [busy,setBusy]=useState(false),[error,setError]=useState(''),[dirty,setDirty]=useState(false),[discard,setDiscard]=useState(false),[schedule,setSchedule]=useState(!policy),[identity]=useState(()=>policy?.id??crypto.randomUUID()),[scheduleId]=useState(()=>crypto.randomUUID())
 const d=policy?.details,last=policy?.schedules.at(-1),today=dateKey(new Date()),money=(s?:string)=>decimal(s??'0',currency)
 const requestClose=()=>{if(!busy){if(dirty)setDiscard(true);else close()}}
 async function submit(e:FormEvent<HTMLFormElement>){e.preventDefault();if(busy)return;const form=new FormData(e.currentTarget),text=(k:string)=>String(form.get(k)??'').trim(),optional=(k:string)=>text(k)||null,amount=(k:string)=>decimalToInteger(text(k),fractionDigits(currency));setBusy(true);setError('');try{
 const source=text('source'),colon=source.indexOf(':')
 const details:PolicyDetails={name:text('name'),insurer:text('insurer'),policy_type:text('type'),number_suffix:optional('suffix'),coverage_start:text('coverage_start'),coverage_end:optional('coverage_end'),renewal_date:optional('renewal'),coverage_amount:amount('coverage'),deductible:amount('deductible'),notes:text('notes'),status:text('status'),status_date:d?.status===text('status')?d.status_date:today,asset_ids:form.getAll('assets').map(String),cash_value_id:optional('cash_value'),replaces:source?{kind:source.slice(0,colon) as 'subscription'|'payment-plan'|'planner-item',id:source.slice(colon+1)}:null}
 const premium:PremiumSchedule|null=schedule?{id:scheduleId,effective_from:text('effective'),first_date:text('first'),end_date:optional('end'),frequency:text('frequency') as PremiumSchedule['frequency'],amount:amount('premium'),account_id:text('account'),category_id:text('category')}:null
 await saveInsurance({id:identity,expected_revision:policy?.revision??null,currency,details,schedule:premium});toast.success(t('Policy saved.'));saved();close()
 }catch(e){setError(String(e).replace(/^Error: /,''))}finally{setBusy(false)}}
 return <Dialog open onOpenChange={o=>{if(!o)requestClose()}}><DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl" onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(busy)e.preventDefault()}} showCloseButton={!busy}><DialogHeader><DialogTitle>{policy?t('Edit policy'):t('Add policy')}</DialogTitle><DialogDescription>{t('Policy details and premiums are included in local backups and encrypted financial sync. Store only a masked policy-number suffix.')}</DialogDescription></DialogHeader>
 <form onSubmit={submit} onChange={()=>setDirty(true)}><fieldset disabled={busy} className="space-y-4"><div className="grid gap-4 sm:grid-cols-2">
 <Field label={t('Policy name')}><Input name="name" required maxLength={100} defaultValue={d?.name} autoFocus/></Field><Field label={t('Insurer')}><Input name="insurer" required maxLength={100} defaultValue={d?.insurer}/></Field>
 <Field label={t('Policy type')}><NativeSelect name="type" defaultValue={d?.policy_type??'motor'}>{Object.entries(types).map(([v,l])=><option value={v} key={v}>{t(l)}</option>)}</NativeSelect></Field>
 <Field label={t('Policy number suffix (optional)')}><Input name="suffix" maxLength={4} pattern="[A-Za-z0-9]{1,4}" defaultValue={d?.number_suffix??''}/></Field>
 <Field label={t('Coverage start')}><Input name="coverage_start" type="date" required defaultValue={d?.coverage_start??today}/></Field><Field label={t('Coverage end (optional)')}><Input name="coverage_end" type="date" defaultValue={d?.coverage_end??''}/></Field>
 <Field label={t('Renewal date')}><Input name="renewal" type="date" defaultValue={d?.renewal_date??''}/></Field><Field label={t('Status')}><NativeSelect name="status" defaultValue={d?.status??'active'}>{Object.entries(statuses).map(([v,l])=><option key={v} value={v}>{t(l)}</option>)}</NativeSelect></Field>
 <Field label={t('Coverage amount')}><Input name="coverage" required inputMode="decimal" defaultValue={money(d?.coverage_amount)}/></Field><Field label={t('Deductible')}><Input name="deductible" required inputMode="decimal" defaultValue={money(d?.deductible)}/></Field>
 <Field label={t('Insurance cash-value account')}><NativeSelect name="cash_value" defaultValue={d?.cash_value_id??''}><option value="">{t('None')}</option>{data.accounts.filter(a=>data.tracking.some(s=>s.id===a.id&&s.draft.subtype==='insurance_cash_value')).map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></Field>
 <Field label={t('Replace an existing forecast')}><NativeSelect name="source" defaultValue={d?.replaces?`${d.replaces.kind}:${d.replaces.id}`:''}><option value="">{t('None')}</option>{data.financial.subscriptions.map(s=><option key={s.id} value={'subscription:'+s.id}>{t('Subscription')}: {s.name}</option>)}{data.financial.plans.filter(p=>p.type==='expense').map(p=><option key={p.id} value={'payment-plan:'+p.id}>{t('Payment plan')}: {p.name}</option>)}{data.planner.items.filter(i=>i.category_id==='expenses').map(i=><option key={i.id} value={'planner-item:'+i.id}>{t('Planner item')}: {i.name}</option>)}</NativeSelect></Field>
 </div><fieldset className="rounded-xl border border-line p-3"><legend>{t('Insured assets')}</legend>{data.accounts.filter(a=>a.type==='tracking').map(a=><label className="mr-4 inline-flex items-center gap-2" key={a.id}><input type="checkbox" name="assets" value={a.id} defaultChecked={d?.asset_ids.includes(a.id)}/>{a.name}</label>)}</fieldset>
 <p className="text-xs text-muted">{t('Linked source forecasts are replaced by this policy. Unlinking restores the original forecast.')}</p>
 <label className="flex items-center gap-2"><input type="checkbox" checked={schedule} onChange={e=>setSchedule(e.target.checked)}/>{policy?t('Change premium schedule'):t('Add premium schedule')}</label>
 {schedule&&<div className="grid gap-4 rounded-xl border border-line p-4 sm:grid-cols-2"><Field label={t('Effective date')}><Input name="effective" type="date" required min={policy?today:undefined} defaultValue={today}/></Field>
 <Field label={t('First premium date')}><Input name="first" type="date" required defaultValue={last?.first_date??today}/></Field><Field label={t('Premium end date (optional)')}><Input name="end" type="date" defaultValue={last?.end_date??''}/></Field>
 <Field label={t('Frequency')}><NativeSelect name="frequency" defaultValue={last?.frequency??'yearly'}>{Object.entries(frequencies).map(([v,l])=><option key={v} value={v}>{t(l)}</option>)}</NativeSelect></Field>
 <Field label={t('Premium amount')}><Input name="premium" inputMode="decimal" required defaultValue={money(last?.amount)}/></Field>
 <Field label={t('Paying account')}><NativeSelect name="account" required defaultValue={last?.account_id??''}><option value="">{t('Choose an account')}</option>{data.accounts.filter(a=>!a.is_archived&&['cash','bank','wallet','credit_card'].includes(a.type)).map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</NativeSelect></Field>
 <Field label={t('Expense category')}><NativeSelect name="category" required defaultValue={last?.category_id??'planner-expense-6'}>{data.financial.categories.filter(c=>!c.is_archived||c.id===last?.category_id).map(c=><option key={c.id} value={c.id}>{c.name}</option>)}</NativeSelect></Field>
 <p className="text-xs text-muted sm:col-span-2">{t('Schedule changes replace unpaid occurrences from the effective date. Earlier versions and payments stay in history. Month-end dates clamp independently; cancellation stops premiums from today.')}</p></div>}
 <Field label={t('Notes')}><Textarea name="notes" maxLength={2000} defaultValue={d?.notes}/></Field>
 {error&&<p role="alert" className="text-red-600">{t(error)}</p>}<DialogFooter><Button type="button" variant="outline" onClick={requestClose}>{t('Cancel')}</Button><Button type="submit">{busy?t('Saving…'):t('Save policy')}</Button></DialogFooter>
 </fieldset></form>
 {discard&&<Discard close={()=>setDiscard(false)} discard={close}/>}
 </DialogContent></Dialog>
}
function Discard({close,discard}:{close:()=>void;discard:()=>void}){return <Dialog open onOpenChange={close}><DialogContent><DialogHeader><DialogTitle>{t('Discard changes?')}</DialogTitle></DialogHeader><DialogFooter><Button variant="outline" onClick={close}>{t('Keep editing')}</Button><Button onClick={discard}>{t('Discard changes')}</Button></DialogFooter></DialogContent></Dialog>}
function PremiumDialog({policy,occurrence,payment,accounts,close,saved}:{policy:Policy;occurrence?:Occurrence;payment?:PremiumPayment;accounts:Account[];close:()=>void;saved:()=>void}){
 const [id]=useState(()=>crypto.randomUUID()),[busy,setBusy]=useState(false),[error,setError]=useState(''),[dirty,setDirty]=useState(false),[discard,setDiscard]=useState(false)
 const value=payment??occurrence!.schedule,requestClose=()=>{if(!busy){if(dirty)setDiscard(true);else close()}}
 async function submit(e:FormEvent<HTMLFormElement>){e.preventDefault();if(busy)return;const f=new FormData(e.currentTarget);setBusy(true);setError('');try{
 if(payment)await reverseInsurance({id,policy_id:policy.id,expected_revision:policy.revision,payment_id:payment.id})
 else await payInsurance({id,policy_id:policy.id,expected_revision:policy.revision,schedule_id:occurrence!.schedule.id,due_date:occurrence!.date,date:String(f.get('date')),notes:String(f.get('notes')??'').trim()})
 toast.success(payment?t('Premium reversed.'):t('Premium recorded.'));saved();close()
 }catch(e){setError(String(e).replace(/^Error: /,''))}finally{setBusy(false)}}
 return <Dialog open onOpenChange={o=>{if(!o)requestClose()}}><DialogContent onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(busy)e.preventDefault()}} showCloseButton={!busy}><DialogHeader><DialogTitle>{payment?t('Reverse premium'):t('Record premium')}</DialogTitle><DialogDescription>{t('This changes the paying account and expense history. It never changes coverage or cash surrender value.')}</DialogDescription></DialogHeader>
 <p>{policy.details.name} · {formatAmount(value.amount,policy.currency)} · {accounts.find(a=>a.id===value.account_id)?.name}</p><p>{t('Due date')}: {payment?.due_date??occurrence?.date}</p>
 <form onSubmit={submit} onChange={()=>setDirty(true)}><fieldset disabled={busy} className="space-y-4">{!payment&&<><Field label={t('Payment date')}><Input name="date" type="date" required max={dateKey(new Date())} defaultValue={dateKey(new Date())}/></Field><Field label={t('Notes')}><Textarea name="notes" maxLength={1000}/></Field></>}
 <label className="flex items-start gap-2 text-sm"><input type="checkbox" required/>{payment?t('I confirm reversal of this recorded premium.'):t('I confirm this premium was paid and has not already been recorded elsewhere.')}</label>
 {error&&<p role="alert" className="text-red-600">{t(error)}</p>}<DialogFooter><Button type="button" variant="outline" onClick={requestClose}>{t('Cancel')}</Button><Button type="submit">{busy?t('Saving…'):t('Confirm')}</Button></DialogFooter></fieldset></form>
 {discard&&<Discard close={()=>setDiscard(false)} discard={close}/>}
 </DialogContent></Dialog>
}
