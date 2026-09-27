import { invoke } from '@tauri-apps/api/core'
import type { Account, Transaction, Installment } from './desktop'
export interface CardStatement { id:string; account_id:string; start_date:string; end_date:string; due_date:string; amount:string; minimum:string; needs_review:boolean }
export interface CardPaymentPlan { statement_id:string; account_id:string; date:string; mode:'full'|'minimum'|'custom'; target:string; available:boolean }
export interface CardAllocation { statement_id:string; transaction_id:string; amount:string; date:string }
export interface CardCoverage { statement_id:string; installment_id:string; date:string }
export interface CardBillingData { statements:CardStatement[]; plans:CardPaymentPlan[]; allocations:CardAllocation[]; installments:CardCoverage[] }
export interface AccountBilling { account:Account; currency:string|null; billing:CardBillingData; transactions:Transaction[]; entries:{statement_id:string;transaction_id:string}[]; installments:Installment[] }
export const getCardBilling=(accountId:string)=>invoke<AccountBilling>('get_card_billing',{accountId})
async function mutate(command:string,args:Record<string,unknown>){await invoke(command,args);for(const name of ['accounts-changed','transactions-changed','plans-changed'])window.dispatchEvent(new Event(name))}
export const saveCardStatement=(input:Omit<CardStatement,'id'|'needs_review'> & {id:string|null;currency:string;installments:{installment_id:string;date:string}[]})=>mutate('save_card_statement',{input})
export const saveCardPaymentPlan=(input:Omit<CardPaymentPlan,'available'> & {currency:string})=>mutate('save_card_payment_plan',{input})
export const recordCardPayment=(input:{statement_id:string;transaction_id:string|null;account_id:string;date:string;amount:string;currency:string})=>mutate('record_card_payment',{input})
export const removeCardBillingRecord=(kind:'statement'|'plan'|'allocation',id:string)=>mutate('remove_card_billing_record',{kind,id})
export function localToday(){const d=new Date();return `${d.getFullYear().toString().padStart(4,'0')}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`}
const positive=(n:bigint)=>n>0n?n:0n
export function statementTotals(data:CardBillingData,s:CardStatement,today=localToday()){
 const paid=data.allocations.filter(a=>a.statement_id===s.id).reduce((n,a)=>n+BigInt(a.amount),0n)
 const plan=data.plans.find(p=>p.statement_id===s.id)
 const remaining=positive(BigInt(s.amount)-paid), minimum=positive(BigInt(s.minimum)-paid)
 const target=plan?plan.mode==='full'?BigInt(s.amount):plan.mode==='minimum'?BigInt(s.minimum):BigInt(plan.target):0n
 const planned=plan?positive((target<BigInt(s.amount)?target:BigInt(s.amount))-paid):0n
 const forecast=plan?.available&&!s.needs_review&&plan.date>=today?planned:0n
 const status=s.needs_review?'Needs review':remaining===0n?'Paid in full':s.due_date<today&&minimum>0n?'Minimum overdue':minimum===0n?'Minimum covered · balance remains':paid>0n?'Partially paid':'Unpaid'
 return {paid,remaining,minimum,plan,planned,forecast,afterPlan:positive(remaining-planned),status}
}
export function cardForecasts(data:CardBillingData|undefined,today=localToday()){
 return (data?.statements??[]).flatMap(s=>{const t=statementTotals(data!,s,today);return t.plan&&t.forecast>0n?[{statement_id:s.id,account_id:t.plan.account_id,card_id:s.account_id,date:t.plan.date,amount:t.forecast}]:[]})
}
export function installmentCovered(data:CardBillingData|undefined,id:string,date:string){return !!data?.installments.some(c=>c.installment_id===id&&c.date===date)}
function dateAt(month:string,day:number){const [y,m]=month.split('-').map(Number);const d=new Date(0);d.setUTCFullYear(y,m,0);return `${month}-${String(Math.min(day,d.getUTCDate())).padStart(2,'0')}`}
function shiftMonth(month:string,offset:number){const [y,m]=month.split('-').map(Number),i=y*12+m-1+offset;return `${Math.floor(i/12).toString().padStart(4,'0')}-${String(i%12+1).padStart(2,'0')}`}
export function draftBillingDates(statementDay:number|null,dueDay:number|null,today=localToday()){
 let month=today.slice(0,7);const day=statementDay??1
 if(dateAt(month,day)>today)month=shiftMonth(month,-1)
 const end_date=dateAt(month,day),previous=dateAt(shiftMonth(month,-1),day)
 const d=new Date(`${previous}T12:00:00Z`);d.setUTCDate(d.getUTCDate()+1)
 const start_date=d.toISOString().slice(0,10)
 let due_date=dateAt(month,dueDay??day);if(due_date<=end_date)due_date=dateAt(shiftMonth(month,1),dueDay??day)
 return {start_date,end_date,due_date}
}
export function cardChange(t:Transaction,cardId:string){return t.destination_account_id===cardId?-BigInt(t.amount):t.account_id===cardId?(t.type==='income'?-BigInt(t.amount):BigInt(t.amount)):0n}
export function statementLedger(account:Account,transactions:Transaction[],start:string,end:string){
 let previous=BigInt(account.opening_balance),purchases=0n,payments=0n,adjustments=0n
 for(const t of transactions){const change=cardChange(t,account.id);if(t.date<start)previous+=change;else if(t.date<=end){if(t.type==='expense'&&t.account_id===account.id)purchases+=change;else if(t.destination_account_id===account.id&&(t.type==='transfer'||t.type==='repayment'))payments-=change;else adjustments+=change}}
 return {previous,purchases,payments,adjustments,total:previous+purchases-payments+adjustments}
}
export function spendingCategories(transactions:{type:string;account_id:string;amount:string;date:string;category_name?:string|null;description?:string;id:string}[],cardId:string,start:string,endExclusive:string){
 const categories=new Map<string,{amount:bigint;transactions:typeof transactions}>()
 for(const t of transactions){if(t.type!=='expense'||t.account_id!==cardId||t.date<start||t.date>=endExclusive)continue;const name=t.category_name??'Uncategorized',entry=categories.get(name)??{amount:0n,transactions:[]};entry.amount+=BigInt(t.amount);entry.transactions.push(t);categories.set(name,entry)}return [...categories].map(([name,value])=>({name,...value}))
}
