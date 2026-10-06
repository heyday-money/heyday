import { t as translate, useLanguage } from '../lib/i18n'
import { AccountLabel } from './InstitutionLogo'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useParams } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { desktopAvailable, listAccounts, type Account } from '../lib/desktop'
import { getLoanAccount, loanSchedule, type LoanSnapshot, type LoanContract } from '../lib/loans'
import { formatAmount } from '../lib/money'
import { interestRateText } from '../lib/installments'
import { dateKey } from '../lib/financial'
import { LoanEditor, type Editor } from './LoanEditor'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'
const message=(e:unknown)=>e instanceof Error?e.message:String(e)

export function LoanAccountPage(){
  useLanguage()

 const {accountId}=useParams({from:'/accounts/$accountId/loans'})
 return <LoanAccount key={accountId} accountId={accountId}/>
}
function LoanAccount({accountId}:{accountId:string}){
  useLanguage()

 const [data,setData]=useState<LoanSnapshot|null>(null),[accounts,setAccounts]=useState<Account[]>([]),[error,setError]=useState<string|null>(null),[editor,setEditor]=useState<Editor|null>(null)
 const request=useRef(0)
 const refresh=useCallback(async()=>{const id=++request.current;try{const [snapshot,active]=await Promise.all([getLoanAccount(accountId),listAccounts()]);if(id===request.current){setData(snapshot);setAccounts(active);setError(null)}}catch(e){if(id===request.current)setError(message(e))}},[accountId])
 useEffect(()=>{if(!desktopAvailable)return;void refresh();const listener=()=>{void refresh()};const events=['accounts-changed','transactions-changed','plans-changed'];events.forEach(e=>window.addEventListener(e,listener));return()=>{request.current++;events.forEach(e=>window.removeEventListener(e,listener))}},[refresh])
 const currency=data?.currency??'THB', money=(n:string|bigint)=>formatAmount(n.toString(),currency)
 const columns:ColumnDef<LoanContract>[]=[
  {header: translate("Contract"),accessorKey:'name',meta:{rowHeader:true},cell:({row:{original:c}})=><div className="max-w-56 whitespace-normal"><p className="font-semibold">{c.name}</p><p className="mt-1 text-xs font-normal text-muted">{translate("First due")}{" "}{c.first_due_date}<br/>{translate("Last scheduled")}{" "}{loanSchedule(c).at(-1)?.date??translate("Unavailable")}<br/>{translate("Pay from")}{" "}<AccountLabel id={c.payment_account_id} name={accounts.find(a=>a.id===c.payment_account_id)?.name??translate("Unavailable account")} /></p></div>},
  {header: translate("Original principal"),cell:({row})=>money(row.original.principal)},
  {header: translate("Remaining principal"),cell:({row})=>money(row.original.remaining_principal)},
  {header: translate("Annual interest"),cell:({row})=>`${interestRateText(String(row.original.interest_rate),4)}%`},
  {header: translate("Monthly payment"),cell:({row})=>money(row.original.monthly_amount)},
  {header: translate("Term"),cell:({row})=>`${row.original.installment_count} months`},
  {header: translate("Next scheduled"),cell:({row})=>loanSchedule(row.original).find(p=>p.date>=dateKey(new Date()))?.date??translate("None")},
  {header: translate("Status"),cell:({row})=>row.original.needs_review?translate("Borrowing deleted · review"):!row.original.accounts_available?translate("Account unavailable"):row.original.schedule_end&&row.original.schedule_end<dateKey(new Date())?translate("Schedule stopped"):!loanSchedule(row.original).some(p=>p.date>=dateKey(new Date()))?translate("Schedule ended (not proof of payoff)"):translate("Scheduled")},
  {header: translate("Actions"),cell:({row:{original:c}})=><div className="flex gap-2"><Button size="xs" variant="outline" disabled={(!!data?.paid_off_on || !!data?.account.is_archived)} onClick={()=>setEditor({kind:'contract',contract:c})}>{translate("Edit")}</Button><Button size="xs" disabled={c.needs_review || (!!data?.paid_off_on || !!data?.account.is_archived)} onClick={()=>setEditor({kind:'repayment',contract:c})}>{translate("Repay")}</Button><Button size="xs" variant="ghost" disabled={(!!data?.paid_off_on || !!data?.account.is_archived)} onClick={()=>setEditor({kind:'remove',contract:c})}>{translate("Remove")}</Button></div>},
 ]
 const table=useReactTable({data:data?.contracts??[],columns: columns.map(column => ({...column,meta:{...column.meta,headerClassName:`px-4 py-3 whitespace-nowrap ${column.meta?.rowHeader?'sticky left-0 z-10 min-w-[200px] bg-card':''}`,cellClassName:`px-4 py-3 whitespace-nowrap tabular-nums ${column.meta?.rowHeader?'sticky left-0 z-10 min-w-[200px] bg-card':''}`}})),getCoreRowModel:getCoreRowModel()})
 if(!desktopAvailable)return <p>{translate("Open the desktop app to manage loan contracts.")}</p>
 if(!data)return <p role={error?'alert':'status'}>{error?translate(error):translate("Loading loan account…")}</p>
 return <section className="space-y-5">
   <nav aria-label={translate("Breadcrumb")} className="text-sm">
     <ol className="flex flex-wrap items-center gap-2 text-muted">
       <li><Link to="/accounts" className="text-brand hover:underline">{translate("Accounts")}</Link></li>
       <li aria-hidden="true">/</li>
       <li><Link to="/accounts/$accountId/details" params={{accountId}} className="text-brand hover:underline">{data.account.name}{data.account.last_four ? ` · •••• ${data.account.last_four}` : ''}</Link></li>
       <li aria-hidden="true">/</li>
       <li aria-current="page">{translate("Borrowings & schedules")}</li>
     </ol>
   </nav>
   <div className="flex flex-wrap items-center justify-between gap-3">
     <h2 className="text-2xl font-semibold">{translate("Borrowings & schedules")}</h2>
     <Button disabled={!data.facility || !!data.paid_off_on || !!data.account.is_archived} onClick={()=>setEditor({kind:'contract'})}>{translate("Add borrowing")}</Button>
   </div>
   {data.paid_off_on ? <p>{translate("Paid off")} {data.paid_off_on}</p> : data.account.is_archived && <p>{translate("Archived")}</p>}
   {error&&<p role="alert">{translate(error)}</p>}
   {!data.facility&&<p>{translate("Set up revolving credit in Account details first.")}</p>}
   <DataTable table={table} label={translate("Loan contracts")}/>
   <p className="text-sm">{translate("Dates are scheduled payments, not proof of payment. Set an inclusive schedule end date when a plan ends early; this never clears debt. Referenced contracts retain repayment history.")}</p>
   {editor&&data.currency&&<LoanEditor key={editor.kind+('contract'in editor?editor.contract?.id??'':'')} editor={editor} data={data} accounts={accounts} close={()=>setEditor(null)} saved={()=>{setEditor(null);void refresh()}}/>}
 </section>
}
