import { t as translate, useLanguage } from '../lib/i18n'
import { useRef, useState, type FormEvent, type ReactNode } from 'react'
import { toast } from 'sonner'
import type { Account } from '../lib/desktop'
import { saveLoanFacility, saveLoanContract, recordLoanRepayment, deleteLoanContract, loanTotals, type LoanSnapshot, type LoanContract } from '../lib/loans'
import { decimalToInteger, fractionDigits, formatAmount } from '../lib/money'
import { interestRateText, installmentSchedule } from '../lib/installments'
import { dateKey } from '../lib/financial'
import { AccountLabel } from './InstitutionLogo'
import { AccountSelect } from './AccountSelect'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { Textarea } from './ui/textarea'
import { FormField as Field } from './FormField'
const message=(e:unknown)=>e instanceof Error?e.message:String(e)
function decimal(value:string,currency:string){const n=BigInt(value),scale=10n**BigInt(fractionDigits(currency));return `${n/scale}${scale>1n?'.'+(n%scale).toString().padStart(fractionDigits(currency),'0'):''}`}
export type Editor={kind:'facility'}|{kind:'contract';contract?:LoanContract}|{kind:'repayment';contract:LoanContract}|{kind:'remove';contract:LoanContract}

export function LoanEditor({editor,data,accounts,close,saved}:{editor:Editor;data:LoanSnapshot;accounts:Account[];close:()=>void;saved:()=>void}){
  useLanguage()

 const currency=data.currency!,digits=fractionDigits(currency),c='contract'in editor?editor.contract:undefined
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[dirty,setDirty]=useState(false),[discard,setDiscard]=useState(false),[kind,setKind]=useState(c?.borrowing_kind??'')
 const lock=useRef(false)
 const requestClose=()=>{if(lock.current)return;if(dirty)setDiscard(true);else close()}
 const cashAccounts=accounts.filter(a=>a.type==='cash'||a.type==='bank')
 const accountOptions=(savedId?:string)=><>{savedId&&!cashAccounts.some(a=>a.id===savedId)&&<option value={savedId}>{translate("Unavailable saved account")}</option>}{cashAccounts.map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</>
 async function submit(e:FormEvent<HTMLFormElement>){e.preventDefault();if(lock.current||discard)return;lock.current=true;setBusy(true);setError(null)
 const form=new FormData(e.currentTarget),text=(key:string)=>String(form.get(key)??'').trim(),minor=(key:string)=>decimalToInteger(text(key),digits)
 try{
  if(editor.kind==='facility')await saveLoanFacility(data.account.id,minor('limit'),currency)
  if(editor.kind==='contract'){
   const principal=c?.principal??minor('principal')
   const input={id:c?.id??null,account_id:data.account.id,name:text('name'),principal,remaining_principal:c?.remaining_principal??(kind==='new'?principal:minor('remaining')),borrowing_date:c?.borrowing_date??text('borrowed'),receiving_account_id:c?.receiving_account_id??text('receiving'),payment_account_id:text('payment'),interest_rate:Number(decimalToInteger(text('interest'),4)),monthly_amount:minor('monthly'),installment_count:Number(text('count')),first_due_date:text('first'),schedule_end:text('end')||null,notes:text('notes'),borrowing_kind:(c?.borrowing_kind??kind) as 'new'|'existing',currency,acknowledge_over_limit:form.get('over')==='on'}
   if (!input.borrowing_kind) throw new Error("Choose how to record this borrowing.")
   if (BigInt(input.principal) <= 0n || BigInt(input.remaining_principal) < 0n || BigInt(input.remaining_principal) > BigInt(input.principal)) throw new Error("Outstanding principal must be between zero and the positive original principal.")
   installmentSchedule(input)
   await saveLoanContract(input)
  }
  if(editor.kind==='repayment') {
   const input={contract_id:editor.contract.id,account_id:text('payment'),date:text('date'),total:minor('total'),principal:minor('principal'),interest:minor('interest'),fee:minor('fee'),currency}
   const amounts=[input.principal,input.interest,input.fee].map(BigInt)
   if (amounts.some(n=>n<0n) || BigInt(input.total)<=0n || amounts.reduce((n,v)=>n+v,0n)!==BigInt(input.total)) throw new Error("Principal, interest and fees must equal the positive total payment.")
   if (amounts[0]>BigInt(editor.contract.remaining_principal)) throw new Error("Principal exceeds this contract’s remaining principal.")
   await recordLoanRepayment(input)
  }
  if(editor.kind==='remove')await deleteLoanContract(editor.contract.id)
  toast.success(editor.kind==='remove'?translate("Contract removed. Transactions preserved."):translate("Loan saved."));saved()
 }catch(e){setError(message(e))}finally{lock.current=false;setBusy(false)}}
 const field=(label:string,name:string,value?:string,props:Record<string,unknown>={})=><Field label={label}><Input name={name} defaultValue={value} required {...props}/></Field>
 let fields:ReactNode
 if(editor.kind==='facility')fields=<>{field(`Shared credit limit (${currency})`,'limit',data.facility?decimal(data.facility.credit_limit,currency):'',{inputMode:'decimal'})}<p className="text-sm">{translate("Use contracts for each borrowing. This replaces account-level and legacy schedule forecasts without changing balances or entered cycle overrides.")}</p></>
 else if(editor.kind==='contract')fields=<>
 {!c&&<Field label={translate("Borrowing entry")}><NativeSelect required value={kind} onChange={e=>setKind(e.target.value as typeof kind)}><option value="" disabled>{translate("Choose how to record this borrowing")}</option><option value="new">{translate("New borrowing — record transfer")}</option><option value="existing">{translate("Already recorded — schedule and allocate only")}</option></NativeSelect></Field>}
 {field(translate('Contract name / reference'),'name',c?.name,{maxLength:100})}
 {!c&&<>{field(`Original principal (${currency})`,'principal',undefined,{inputMode:'decimal'})}{kind==='existing'&&<>{field(`Outstanding principal to allocate (${currency})`,'remaining',undefined,{inputMode:'decimal'})}<p className="text-xs">{translate("Must already be included in account debt. Prior repayments are not recreated. Available unassigned debt:")}{" "}{formatAmount(loanTotals(data).unassigned.toString(),currency)}.</p></>}{field(translate("Borrowing date"),'borrowed',dateKey(new Date()),{type:'date',max:dateKey(new Date())})}<Field label={translate("Receiving account")}><AccountSelect name="receiving" required>{accountOptions()}</AccountSelect></Field></>}
 {c&&<p className="text-sm">{translate("Borrowed")}{" "}{formatAmount(c.principal,currency)} {" "}{translate("on")}{" "}{c.borrowing_date}{translate(". Borrowing details are locked.")}{" "}{c.needs_review?translate("Borrowing was deleted: remove this contract and create the corrected borrowing."):''}</p>}
 <Field label={translate("Payment source")}><AccountSelect name="payment" defaultValue={c?.payment_account_id} required>{accountOptions(c?.payment_account_id)}</AccountSelect></Field>
 {field(translate("Annual interest rate (%)"),'interest',c?interestRateText(String(c.interest_rate),4):'0',{type:'number',min:0,max:100,step:'0.0001'})}
 {field(translate('Lender monthly payment ({value0})', { value0: currency }),'monthly',c?decimal(c.monthly_amount,currency):undefined,{inputMode:'decimal'})}
 {field(translate("Number of installments"),'count',c?String(c.installment_count):undefined,{type:'number',min:1,max:600,step:1})}
 {field(translate("First due date"),'first',c?.first_due_date,{type:'date'})}
 {field(translate("Schedule end date (optional, inclusive)"),'end',c?.schedule_end??'',{type:'date',required:false})}
 <Field label={translate("Notes (optional)")}><Textarea name="notes" defaultValue={c?.notes} maxLength={1000}/></Field>
 <p className="text-xs">{translate("Enter the lender’s payment including interest and fees. No interest calculation or paid status is inferred. Schedule edits affect past and future estimates; account cycle overrides remain. An end date stops estimates only.")}</p>
 {!c&&kind==='new'&&<label className="flex gap-2 text-sm"><Input type="checkbox" name="over" className="size-4"/>{translate("I acknowledge if this borrowing exceeds estimated available credit (")}{formatAmount((loanTotals(data).available??0n).toString(),currency)}).</label>}
 </>
 else if(editor.kind==='repayment')fields=<><p>{translate("Repay")}{" "}{editor.contract.name}{translate(". Remaining principal:")}{" "}{formatAmount(editor.contract.remaining_principal,currency)}.</p><Field label={translate("Payment source")}><NativeSelect name="payment" required defaultValue={editor.contract.payment_account_id}>{accountOptions(editor.contract.payment_account_id)}</NativeSelect></Field>{field(translate("Payment date"),'date',dateKey(new Date()),{type:'date',max:dateKey(new Date())})}{field(`Total payment (${currency})`,'total',undefined,{inputMode:'decimal'})}{field(`Principal (${currency})`,'principal',undefined,{inputMode:'decimal'})}{field(`Interest (${currency})`,'interest','0',{inputMode:'decimal'})}{field(`Fees (${currency})`,'fee','0',{inputMode:'decimal'})}<p className="text-xs">{translate("The split must equal the total. Principal reduces loan debt; interest and fees are separate expenses from the same cash account. Use this split only for interest and fees not already recorded on the loan. Repay previously recorded charges separately against unassigned debt. No accrued-interest calculation or schedule matching occurs.")}</p></>
 else fields=<p>{translate("Remove this contract and its schedule? Recorded borrowing transactions remain as unassigned debt. Contracts with repayment history cannot be removed; set a schedule end date instead.")}</p>
 return <Dialog open onOpenChange={open=>{if(!open)requestClose()}}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[620px]" showCloseButton={!busy} onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(busy)e.preventDefault()}}><DialogHeader className="px-6 pt-6"><DialogTitle>{editor.kind==='facility'?translate("Revolving credit"):editor.kind==='contract'?c?translate("Edit contract"):translate("Add borrowing"):editor.kind==='repayment'?translate("Record contract repayment"):translate("Remove contract?")}</DialogTitle><DialogDescription><AccountLabel id={data.account.id} name={data.account.name} /> · {currency}</DialogDescription></DialogHeader><form onSubmit={submit} onChange={()=>setDirty(true)} className="flex min-h-0 flex-col"><fieldset disabled={busy} className="min-h-0 space-y-4 overflow-y-auto p-6">{fields}{error&&<p role="alert">{translate(error)}</p>}</fieldset><DialogFooter className="shrink-0 border-t border-line p-4">{discard?<><p>{translate("Discard unsaved changes?")}</p><Button type="button" variant="outline" onClick={()=>setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard")}</Button></>:<><Button type="button" variant="outline" disabled={busy} onClick={requestClose}>{translate("Cancel")}</Button><Button type="submit" disabled={busy}>{busy?translate("Saving…"):editor.kind==='remove'?translate("Remove contract"):translate("Save")}</Button></>}</DialogFooter></form></DialogContent></Dialog>
}
