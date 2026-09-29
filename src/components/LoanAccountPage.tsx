import { MarkLoanPaidOffDialog } from './MarkLoanPaidOffDialog'
import { AccountLabel } from './InstitutionLogo'
import { AccountSelect } from './AccountSelect'
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { Link, useParams } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { toast } from 'sonner'
import { desktopAvailable, listAccounts, deleteTransaction, type Account, type Transaction } from '../lib/desktop'
import { getLoanAccount, saveLoanFacility, saveLoanContract, recordLoanRepayment, deleteLoanContract, loanTotals, loanSchedule, type LoanSnapshot, type LoanContract } from '../lib/loans'
import { decimalToInteger, fractionDigits, formatAmount } from '../lib/money'
import { interestRateText, installmentSchedule } from '../lib/installments'
import { dateKey } from '../lib/financial'
import { AccountFormDialog } from './AccountFormDialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from './ui/tabs'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { Textarea } from './ui/textarea'
import { DataTable } from './ui/data-table'
import { FormField as Field } from './FormField'
const message=(e:unknown)=>e instanceof Error?e.message:String(e)
function decimal(value:string,currency:string){const n=BigInt(value),scale=10n**BigInt(fractionDigits(currency));return `${n/scale}${scale>1n?'.'+(n%scale).toString().padStart(fractionDigits(currency),'0'):''}`}
type Editor={kind:'facility'}|{kind:'contract';contract?:LoanContract}|{kind:'repayment';contract:LoanContract}|{kind:'remove';contract:LoanContract}|{kind:'delete';transaction:Transaction}

export function LoanAccountPage(){
 const {accountId}=useParams({from:'/accounts/$accountId/loans'})
 return <LoanAccount key={accountId} accountId={accountId}/>
}
function LoanAccount({accountId}:{accountId:string}){
 const [data,setData]=useState<LoanSnapshot|null>(null),[accounts,setAccounts]=useState<Account[]>([]),[error,setError]=useState<string|null>(null),[editor,setEditor]=useState<Editor|null>(null),[editing,setEditing]=useState(false)
 const [paidOff,setPaidOff]=useState(false)
 const request=useRef(0)
 const refresh=useCallback(async()=>{const id=++request.current;try{const [snapshot,active]=await Promise.all([getLoanAccount(accountId),listAccounts()]);if(id===request.current){setData(snapshot);setAccounts(active);setError(null)}}catch(e){if(id===request.current)setError(message(e))}},[accountId])
 useEffect(()=>{if(!desktopAvailable)return;void refresh();const listener=()=>{void refresh()};const events=['accounts-changed','transactions-changed','plans-changed'];events.forEach(e=>window.addEventListener(e,listener));return()=>{request.current++;events.forEach(e=>window.removeEventListener(e,listener))}},[refresh])
 const currency=data?.currency??'THB', money=(n:string|bigint)=>formatAmount(n.toString(),currency)
 const columns:ColumnDef<LoanContract>[]=[
  {header:'Contract',accessorKey:'name',meta:{rowHeader:true},cell:({row:{original:c}})=><div className="max-w-56 whitespace-normal"><p className="font-semibold">{c.name}</p><p className="mt-1 text-xs font-normal text-muted">First due {c.first_due_date}<br/>Last scheduled {loanSchedule(c).at(-1)?.date??'Unavailable'}<br/>Pay from <AccountLabel id={c.payment_account_id} name={accounts.find(a=>a.id===c.payment_account_id)?.name??'Unavailable account'} /></p></div>},
  {header:'Original principal',cell:({row})=>money(row.original.principal)},
  {header:'Remaining principal',cell:({row})=>money(row.original.remaining_principal)},
  {header:'Annual interest',cell:({row})=>`${interestRateText(String(row.original.interest_rate),4)}%`},
  {header:'Monthly payment',cell:({row})=>money(row.original.monthly_amount)},
  {header:'Term',cell:({row})=>`${row.original.installment_count} months`},
  {header:'Next scheduled',cell:({row})=>loanSchedule(row.original).find(p=>p.date>=dateKey(new Date()))?.date??'None'},
  {header:'Status',cell:({row})=>row.original.needs_review?'Borrowing deleted · review':!row.original.accounts_available?'Account unavailable':row.original.schedule_end&&row.original.schedule_end<dateKey(new Date())?'Schedule stopped':!loanSchedule(row.original).some(p=>p.date>=dateKey(new Date()))?'Schedule ended (not proof of payoff)':'Scheduled'},
  {header:'Actions',cell:({row:{original:c}})=><div className="flex gap-2"><Button size="xs" variant="outline" disabled={!!data?.paid_off_on} onClick={()=>setEditor({kind:'contract',contract:c})}>Edit</Button><Button size="xs" disabled={c.needs_review || !!data?.paid_off_on} onClick={()=>setEditor({kind:'repayment',contract:c})}>Repay</Button><Button size="xs" variant="ghost" onClick={()=>setEditor({kind:'remove',contract:c})}>Remove</Button></div>},
 ]
 const table=useReactTable({data:data?.contracts??[],columns: columns.map(column => ({...column,meta:{...column.meta,headerClassName:`px-4 py-3 whitespace-nowrap ${column.meta?.rowHeader?'sticky left-0 z-10 min-w-[200px] bg-card':''}`,cellClassName:`px-4 py-3 whitespace-nowrap tabular-nums ${column.meta?.rowHeader?'sticky left-0 z-10 min-w-[200px] bg-card':''}`}})),getCoreRowModel:getCoreRowModel()})
 const historyColumns:ColumnDef<Transaction>[]=[{header:'Date',accessorKey:'date'},{header:'Description',cell:({row})=>row.original.description||row.original.type},{header:'Type',accessorKey:'type'},{header:'Amount',cell:({row})=>money(row.original.amount)},{header:'Actions',cell:({row})=><Button size="xs" variant="ghost" onClick={()=>setEditor({kind:'delete',transaction:row.original})}>Delete</Button>}]
 const history=useReactTable({data:data?.transactions??[],columns:historyColumns.map(column => ({...column,meta:{headerClassName:'px-4 py-3 whitespace-nowrap',cellClassName:'px-4 py-3'}})),getCoreRowModel:getCoreRowModel()})
 if(!desktopAvailable)return <p>Open the desktop app to manage loan contracts.</p>
 if(!data)return <p role={error?'alert':'status'}>{error??'Loading loan account…'}</p>
 const totals=loanTotals(data)
 return <section className="space-y-5"><Link to="/accounts" className="text-brand">← Accounts</Link><div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-2xl font-semibold"><AccountLabel id={data.account.id} name={data.account.name} />{data.account.last_four?` ··${data.account.last_four}`:''}</h2><p>{data.facility?'Revolving credit / cash card':'Single loan'}</p></div><div className="flex gap-2">{data.paid_off_on ? <span>Paid off {data.paid_off_on}</span> : <><Button variant="outline" onClick={()=>setEditing(true)}>Edit account</Button><Button variant="outline" onClick={()=>setPaidOff(true)}>Mark as paid off</Button></>}</div></div>
 {error&&<p role="alert">{error}</p>}
 <Tabs defaultValue="overview"><TabsList><TabsTrigger value="overview">Overview</TabsTrigger><TabsTrigger value="contracts">Contracts ({data.contracts.length})</TabsTrigger><TabsTrigger value="transactions">Transactions</TabsTrigger></TabsList>
 <TabsContent value="overview" className="space-y-5"><div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">{[['Outstanding Balance',BigInt(data.account.current_balance)],['Assigned outstanding principal',totals.principal],['Unassigned debt / credit',totals.unassigned],[data.facility?'Credit Limit':'Initial Loan Amount',data.facility?BigInt(data.facility.credit_limit):data.account.initial_loan_amount!=null?BigInt(data.account.initial_loan_amount):null],['Estimated available credit',totals.available]].map(([label,value])=><div key={String(label)} className="rounded-2xl border border-line bg-card p-5"><p className="text-sm">{label}</p><p className="mt-2 text-xl font-semibold">{typeof value==='bigint'?money(value):'Not configured'}</p></div>)}</div>
 <p className="text-sm">Available credit uses current debt, including unassigned debt, conservatively. Your lender may apply other charges or holds. Allocate only outstanding principal already included in this account; never count the same borrowing twice.</p>
 {data.account.loan_type==='personal_loan'?<Button disabled={!!data.paid_off_on} onClick={()=>setEditor({kind:'facility'})}>{data.facility?'Edit shared credit limit':'Set up revolving credit / cash card'}</Button>:<p>Choose Personal Loan in Edit account to use revolving credit.</p>}
 {data.facility&&<p className="text-sm">Contract schedules replace this account’s monthly installment and legacy loan schedule forecasts. Saved cycle overrides remain on the account row. Review manual plans and payroll deductions for duplicates. Interest and fees paid with contract repayments are cash expenses; accrued interest is not calculated automatically.</p>}
 </TabsContent>
 <TabsContent value="contracts" className="space-y-4"><Button disabled={!data.facility || !!data.paid_off_on} onClick={()=>setEditor({kind:'contract'})}>Add borrowing</Button>{!data.facility&&<p>Set up revolving credit in Overview first.</p>}<DataTable table={table} label="Loan contracts"/><p className="text-sm">Dates are scheduled payments, not proof of payment. Set an inclusive schedule end date when a plan ends early; this never clears debt. Referenced contracts retain repayment history.</p></TabsContent>
 <TabsContent value="transactions" className="space-y-4"><DataTable table={history} label="Loan transactions"/><p className="text-sm">Contract payments show principal, interest and fees separately. Deleting any linked component reverses the entire payment, including all account balance changes.</p></TabsContent>
 </Tabs>
 {paidOff&&data.currency&&<MarkLoanPaidOffDialog account={data.account} currency={data.currency} onClose={()=>setPaidOff(false)}/>}
 {editing&&data.currency&&<AccountFormDialog account={data.account} initialType="loan" currency={data.currency} onClose={()=>setEditing(false)} onSaved={()=>{void refresh()}}/>}
 {editor&&data.currency&&<LoanEditor key={editor.kind+('contract'in editor?editor.contract?.id??'':'')} editor={editor} data={data} accounts={accounts} close={()=>setEditor(null)} saved={()=>{setEditor(null);void refresh()}}/>}
 </section>
}
function LoanEditor({editor,data,accounts,close,saved}:{editor:Editor;data:LoanSnapshot;accounts:Account[];close:()=>void;saved:()=>void}){
 const currency=data.currency!,digits=fractionDigits(currency),c='contract'in editor?editor.contract:undefined
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[dirty,setDirty]=useState(false),[discard,setDiscard]=useState(false),[kind,setKind]=useState(c?.borrowing_kind??''),[confirmed,setConfirmed]=useState(false)
 const lock=useRef(false)
 const requestClose=()=>{if(lock.current)return;if(dirty)setDiscard(true);else close()}
 const cashAccounts=accounts.filter(a=>a.type==='cash'||a.type==='bank')
 const accountOptions=(savedId?:string)=><>{savedId&&!cashAccounts.some(a=>a.id===savedId)&&<option value={savedId}>Unavailable saved account</option>}{cashAccounts.map(a=><option key={a.id} value={a.id}>{a.name}</option>)}</>
 async function submit(e:FormEvent<HTMLFormElement>){e.preventDefault();if(lock.current||discard)return;lock.current=true;setBusy(true);setError(null)
 const form=new FormData(e.currentTarget),text=(key:string)=>String(form.get(key)??'').trim(),minor=(key:string)=>decimalToInteger(text(key),digits)
 try{
  if(editor.kind==='facility')await saveLoanFacility(data.account.id,minor('limit'),currency)
  if(editor.kind==='contract'){
   const principal=c?.principal??minor('principal')
   const input={id:c?.id??null,account_id:data.account.id,name:text('name'),principal,remaining_principal:c?.remaining_principal??(kind==='new'?principal:minor('remaining')),borrowing_date:c?.borrowing_date??text('borrowed'),receiving_account_id:c?.receiving_account_id??text('receiving'),payment_account_id:text('payment'),interest_rate:Number(decimalToInteger(text('interest'),4)),monthly_amount:minor('monthly'),installment_count:Number(text('count')),first_due_date:text('first'),schedule_end:text('end')||null,notes:text('notes'),borrowing_kind:(c?.borrowing_kind??kind) as 'new'|'existing',currency,acknowledge_over_limit:form.get('over')==='on'}
   if (!input.borrowing_kind) throw new Error('Choose how to record this borrowing.')
   if (BigInt(input.principal) <= 0n || BigInt(input.remaining_principal) < 0n || BigInt(input.remaining_principal) > BigInt(input.principal)) throw new Error('Outstanding principal must be between zero and the positive original principal.')
   installmentSchedule(input)
   await saveLoanContract(input)
  }
  if(editor.kind==='repayment') {
   const input={contract_id:editor.contract.id,account_id:text('payment'),date:text('date'),total:minor('total'),principal:minor('principal'),interest:minor('interest'),fee:minor('fee'),currency}
   const amounts=[input.principal,input.interest,input.fee].map(BigInt)
   if (amounts.some(n=>n<0n) || BigInt(input.total)<=0n || amounts.reduce((n,v)=>n+v,0n)!==BigInt(input.total)) throw new Error('Principal, interest and fees must equal the positive total payment.')
   if (amounts[0]>BigInt(editor.contract.remaining_principal)) throw new Error('Principal exceeds this contract’s remaining principal.')
   await recordLoanRepayment(input)
  }
  if(editor.kind==='remove')await deleteLoanContract(editor.contract.id)
  if(editor.kind==='delete')await deleteTransaction(editor.transaction.id,confirmed)
  toast.success(editor.kind==='delete'?'Payment deleted. Balances updated.':editor.kind==='remove'?'Contract removed. Transactions preserved.':'Loan saved.');saved()
 }catch(e){setError(message(e))}finally{lock.current=false;setBusy(false)}}
 const field=(label:string,name:string,value?:string,props:Record<string,unknown>={})=><Field label={label}><Input name={name} defaultValue={value} required {...props}/></Field>
 let fields:ReactNode
 if(editor.kind==='facility')fields=<>{field(`Shared credit limit (${currency})`,'limit',data.facility?decimal(data.facility.credit_limit,currency):'',{inputMode:'decimal'})}<p className="text-sm">Use contracts for each borrowing. This replaces account-level and legacy schedule forecasts without changing balances or entered cycle overrides.</p></>
 else if(editor.kind==='contract')fields=<>
 {!c&&<Field label="Borrowing entry"><NativeSelect required value={kind} onChange={e=>setKind(e.target.value as typeof kind)}><option value="" disabled>Choose how to record this borrowing</option><option value="new">New borrowing — record transfer</option><option value="existing">Already recorded — schedule and allocate only</option></NativeSelect></Field>}
 {field('Contract name / reference','name',c?.name,{maxLength:100})}
 {!c&&<>{field(`Original principal (${currency})`,'principal',undefined,{inputMode:'decimal'})}{kind==='existing'&&<>{field(`Outstanding principal to allocate (${currency})`,'remaining',undefined,{inputMode:'decimal'})}<p className="text-xs">Must already be included in account debt. Prior repayments are not recreated. Available unassigned debt: {formatAmount(loanTotals(data).unassigned.toString(),currency)}.</p></>}{field('Borrowing date','borrowed',dateKey(new Date()),{type:'date',max:dateKey(new Date())})}<Field label="Receiving account"><AccountSelect name="receiving" required>{accountOptions()}</AccountSelect></Field></>}
 {c&&<p className="text-sm">Borrowed {formatAmount(c.principal,currency)} on {c.borrowing_date}. Borrowing details are locked. {c.needs_review?'Borrowing was deleted: remove this contract and create the corrected borrowing.':''}</p>}
 <Field label="Payment source"><AccountSelect name="payment" defaultValue={c?.payment_account_id} required>{accountOptions(c?.payment_account_id)}</AccountSelect></Field>
 {field('Annual interest rate (%)','interest',c?interestRateText(String(c.interest_rate),4):'0',{type:'number',min:0,max:100,step:'0.0001'})}
 {field(`Lender monthly payment (${currency})`,'monthly',c?decimal(c.monthly_amount,currency):undefined,{inputMode:'decimal'})}
 {field('Number of installments','count',c?String(c.installment_count):undefined,{type:'number',min:1,max:600,step:1})}
 {field('First due date','first',c?.first_due_date,{type:'date'})}
 {field('Schedule end date (optional, inclusive)','end',c?.schedule_end??'',{type:'date',required:false})}
 <Field label="Notes (optional)"><Textarea name="notes" defaultValue={c?.notes} maxLength={1000}/></Field>
 <p className="text-xs">Enter the lender’s payment including interest and fees. No interest calculation or paid status is inferred. Schedule edits affect past and future estimates; account cycle overrides remain. An end date stops estimates only.</p>
 {!c&&kind==='new'&&<label className="flex gap-2 text-sm"><Input type="checkbox" name="over" className="size-4"/>I acknowledge if this borrowing exceeds estimated available credit ({formatAmount((loanTotals(data).available??0n).toString(),currency)}).</label>}
 </>
 else if(editor.kind==='repayment')fields=<><p>Repay {editor.contract.name}. Remaining principal: {formatAmount(editor.contract.remaining_principal,currency)}.</p><Field label="Payment source"><NativeSelect name="payment" required defaultValue={editor.contract.payment_account_id}>{accountOptions(editor.contract.payment_account_id)}</NativeSelect></Field>{field('Payment date','date',dateKey(new Date()),{type:'date',max:dateKey(new Date())})}{field(`Total payment (${currency})`,'total',undefined,{inputMode:'decimal'})}{field(`Principal (${currency})`,'principal',undefined,{inputMode:'decimal'})}{field(`Interest (${currency})`,'interest','0',{inputMode:'decimal'})}{field(`Fees (${currency})`,'fee','0',{inputMode:'decimal'})}<p className="text-xs">The split must equal the total. Principal reduces loan debt; interest and fees are separate expenses from the same cash account. Use this split only for interest and fees not already recorded on the loan. Repay previously recorded charges separately against unassigned debt. No accrued-interest calculation or schedule matching occurs.</p></>
 else fields=<><p>{editor.kind==='remove'?'Remove this contract and its schedule? Recorded borrowing transactions remain as unassigned debt. Contracts with repayment history cannot be removed; set a schedule end date instead.':'Delete this entry? For a linked loan payment, all its principal, interest and fee entries are reversed together. Deleting a borrowing requires removing its repayments first and leaves its contract marked for review.'}</p>{editor.kind==='delete'&&<label className="flex gap-2"><Input type="checkbox" className="size-4" checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/>I confirm deletion, including any linked components and affected reconciliation history.</label>}</>
 return <Dialog open onOpenChange={open=>{if(!open)requestClose()}}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[620px]" showCloseButton={!busy} onInteractOutside={e=>e.preventDefault()} onEscapeKeyDown={e=>{if(busy)e.preventDefault()}}><DialogHeader className="px-6 pt-6"><DialogTitle>{editor.kind==='facility'?'Revolving credit':editor.kind==='contract'?c?'Edit contract':'Add borrowing':editor.kind==='repayment'?'Record contract repayment':editor.kind==='remove'?'Remove contract?':'Delete transaction?'}</DialogTitle><DialogDescription><AccountLabel id={data.account.id} name={data.account.name} /> · {currency}</DialogDescription></DialogHeader><form onSubmit={submit} onChange={()=>setDirty(true)} className="flex min-h-0 flex-col"><fieldset disabled={busy} className="min-h-0 space-y-4 overflow-y-auto p-6">{fields}{error&&<p role="alert">{error}</p>}</fieldset><DialogFooter className="shrink-0 border-t border-line p-4">{discard?<><p>Discard unsaved changes?</p><Button type="button" variant="outline" onClick={()=>setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard</Button></>:<><Button type="button" variant="outline" disabled={busy} onClick={requestClose}>Cancel</Button><Button type="submit" disabled={busy||(editor.kind==='delete'&&!confirmed)}>{busy?'Saving…':editor.kind==='delete'?'Delete transaction':editor.kind==='remove'?'Remove contract':'Save'}</Button></>}</DialogFooter></form></DialogContent></Dialog>
}
