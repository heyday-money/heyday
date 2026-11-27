import { invoke } from '@tauri-apps/api/core'
export interface InsuranceSource { kind:'subscription'|'payment-plan'|'planner-item';id:string }
export interface PolicyDetails {
 name:string;insurer:string;policy_type:string;number_suffix:string|null;coverage_start:string;coverage_end:string|null;renewal_date:string|null;
 coverage_amount:string;deductible:string;notes:string;status:string;status_date:string;asset_ids:string[];cash_value_id:string|null;replaces:InsuranceSource|null
}
export interface PremiumSchedule {id:string;effective_from:string;first_date:string;end_date:string|null;frequency:'once'|'monthly'|'quarterly'|'yearly';amount:string;account_id:string;category_id:string}
export interface PremiumPayment {id:string;schedule_id:string;due_date:string;date:string;amount:string;account_id:string;category_id:string;notes:string;reversal_id:string|null}
export interface Policy {id:string;revision:string;currency:string;details:PolicyDetails;schedules:PremiumSchedule[];payments:PremiumPayment[]}
export interface Occurrence {policy:Policy;schedule:PremiumSchedule;date:string;payment?:PremiumPayment}
export const listInsurance=()=>invoke<Policy[]>('list_insurance')
async function change(command:string,input:unknown){const p=await invoke<Policy>(command,{input});for(const e of ['plans-changed','accounts-changed','transactions-changed'])window.dispatchEvent(new Event(e));return p}
export const saveInsurance=(input:{id:string;expected_revision:string|null;currency:string;details:PolicyDetails;schedule:PremiumSchedule|null})=>change('save_insurance',input)
export const payInsurance=(input:{id:string;policy_id:string;expected_revision:string;schedule_id:string;due_date:string;date:string;notes:string})=>change('pay_insurance',input)
export const reverseInsurance=(input:{id:string;policy_id:string;expected_revision:string;payment_id:string})=>change('reverse_insurance',input)
const month=(d:string)=>Number(d.slice(0,4))*12+Number(d.slice(5,7))-1
const at=(index:number,day:number)=>{const y=Math.floor(index/12),m=index%12+1,leap=y%4===0&&(y%100!==0||y%400===0),days=[31,leap?29:28,31,30,31,30,31,31,30,31,30,31];return `${String(y).padStart(4,'0')}-${String(m).padStart(2,'0')}-${String(Math.min(day,days[m-1])).padStart(2,'0')}`}
export function premiumOccurrences(p:Policy,from:string,to:string):Occurrence[]{
 if(p.details.status==='archived')return []
 const result:Occurrence[]=[]
 for(let i=0;i<p.schedules.length;i++){
  const s=p.schedules[i],first=month(s.first_date),step=s.frequency==='yearly'?12:s.frequency==='quarterly'?3:1
  const start=Math.max(first,month(from)),limit=s.frequency==='once'?first+1:Math.min(month(to)+1,120000)
  for(let m=first+Math.ceil((start-first)/step)*step;m<limit;m+=step){
   const d=at(m,Number(s.first_date.slice(8)))
   if(d<from||d>=to||d<s.first_date||d<s.effective_from||d<p.details.coverage_start||s.end_date&&d>s.end_date||p.details.coverage_end&&d>p.details.coverage_end||p.schedules[i+1]&&d>=p.schedules[i+1].effective_from||['cancelled','expired'].includes(p.details.status)&&d>=p.details.status_date)continue
   result.push({policy:p,schedule:s,date:d,payment:p.payments.find(v=>v.schedule_id===s.id&&v.due_date===d&&!v.reversal_id)})
  }
 }
 return result.sort((a,b)=>a.date.localeCompare(b.date))
}
export function replaces(policies:Policy[]|undefined,kind:InsuranceSource['kind'],id:string){return !!policies?.some(p=>p.details.replaces?.kind===kind&&p.details.replaces.id===id)}
export function premiumForecast(policies:Policy[]|undefined,cashIds:string[],from:string,to:string){
 return (policies??[]).flatMap(p=>premiumOccurrences(p,from,to)).filter(o=>!o.payment&&cashIds.includes(o.schedule.account_id))
}
