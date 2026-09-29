import { useEffect, useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { desktopAvailable } from '../lib/desktop'
import { cardOverview, getCardOverview, type CardOverviewData, type CardOverviewRow } from '../lib/card-overview'
import { formatAmount } from '../lib/money'
import { dateKey } from '../lib/financial'
import { useLocalDate } from '../lib/useLocalDate'
import { AccountLabel } from './InstitutionLogo'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'

export function CreditCardsOutlook() {
  const [data,setData]=useState<CardOverviewData|null>(null), [error,setError]=useState(false), [loading,setLoading]=useState(true), [attempt,setAttempt]=useState(0)
  const today=dateKey(useLocalDate())
  useEffect(() => {
    if (!desktopAvailable) return
    let active=true, sequence=0
    const load=async () => { const request=++sequence; setLoading(true); try { const result=await getCardOverview(); if(active && request===sequence){setData(result);setError(false)} } catch {if(active && request===sequence)setError(true)} finally {if(active && request===sequence)setLoading(false)} }
    void load()
    const events=['accounts-changed','transactions-changed','plans-changed','transaction-options-changed','focus']
    events.forEach(e=>window.addEventListener(e,load))
    return()=>{active=false;events.forEach(e=>window.removeEventListener(e,load))}
  },[attempt,today])
  if(!desktopAvailable)return <p>Open the desktop app to view credit cards.</p>
  if(error)return <div role="alert">Could not load credit cards. <Button variant="outline" onClick={()=>setAttempt(n=>n+1)}>Retry credit cards</Button></div>
  if(!data)return <p role="status">Loading credit cards…</p>
  if(!data.currency)return <p>Choose your currency in <Link to="/settings" className="text-brand">Settings</Link> to view credit cards.</p>
  const result=cardOverview(data,today), money=(n:bigint)=>formatAmount(n.toString(),data.currency!)
  return <section className="space-y-5" aria-label="Credit card billing overview">
    <div><h2 className="text-2xl font-semibold">Credit Cards</h2><p className="mt-2 text-sm">As of {today}. Each card uses its own statement dates, independently of your payday period. Purchases count whether cleared or uncleared.</p></div>
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">{[['Total currently owed',result.owed],['Card credits / overpayments',result.credit],['Latest closed-cycle purchases',result.latestSpending],['Next bills · purchases so far',result.nextSpending]].map(([label,value])=><div key={String(label)} className="rounded-2xl border border-line bg-card p-4"><p className="text-sm">{String(label)}</p><p className="mt-2 text-xl font-semibold tabular-nums">{money(value as bigint)}</p></div>)}</div>
    <p className="text-sm">Purchase totals cover different date ranges for each card and are not added to total owed. Repayments reduce debt, not recorded spending. Archived cards remain included.</p>
    {result.unconfigured>0&&<p role="status" className="text-sm">{result.unconfigured} card(s) need a statement day in Edit account. Missing cycle figures are unavailable, not zero.</p>}
    <OverviewTable rows={result.rows} currency={data.currency} loading={loading}/>
    {!result.rows.length&&<p>No credit card accounts yet. <Link to="/accounts" className="text-brand">Add a credit card</Link>.</p>}
    <p className="text-xs">Unconfirmed closing balances are estimates from recorded balances and activity, including manual starting balances. Confirm dates, interest, fees and amounts against your bank statement. This overview does not create payment plans or change balances.</p>
  </section>
}

function OverviewTable({rows,currency,loading}:{rows:CardOverviewRow[];currency:string;loading:boolean}) {
  const columns=useMemo<ColumnDef<CardOverviewRow>[]>(()=>{
    const money=(n:bigint|null)=>n===null?'Unavailable':formatAmount(n.toString(),currency)
    const definitions: ColumnDef<CardOverviewRow>[] = [
      {id:'card',header:'Card',cell:({row})=><div className="min-w-40"><AccountLabel id={row.original.account.id} name={row.original.account.name}/>{row.original.archived&&<p className="mt-1 text-xs">Archived</p>}</div>},
      {id:'latest',header:'Latest closed bill',cell:({row})=>{const r=row.original;return <div className="min-w-56"><p>{r.start&&r.end?`${r.start} – ${r.end}`:'Set statement day'}</p><p className="mt-1 text-xs">Due: {r.due??'Not configured'}</p><p className="mt-2">Purchases: {money(r.latestSpending)}</p>{r.statement?<><p className="mt-2 font-semibold">Remaining: {money(r.totals!.remaining)}</p><p className="text-xs">Bank statement: {money(BigInt(r.statement.amount))} · Applied payments: {money(r.totals!.paid)}</p><p className="mt-1 text-xs">{r.totals!.status}</p></>:<p className="mt-2 font-semibold">Estimated closing balance: {money(r.estimatedClosing)}</p>}</div>}},
      {id:'next',header:'Next bill accumulating',cell:({row})=>{const r=row.original;return <div className="min-w-52"><p>{r.nextStart??'Unknown'} – {r.nextEnd??'Set statement day'}</p><p className="mt-1 text-xs">Expected due: {r.nextDue??'Not configured'}</p><p className="mt-2 font-semibold">Purchases so far: {money(r.nextSpending)}</p><p className="mt-1 text-xs">Spending estimate, not the final bill.</p></div>}},
      {id:'owed',header:'Current owed / credit',cell:({row})=><div className="min-w-36 text-right"><p className="font-semibold tabular-nums">{money(row.original.balance)}</p><p className="mt-1 text-xs">Includes previous unpaid debt. Negative means credit.</p></div>},
      {id:'details',header:'Review',cell:({row})=>{const r=row.original;return <div className="min-w-64"><Link to="/accounts/$accountId/billing" params={{accountId:r.account.id}} className="font-medium text-brand">Review bank statement & payments</Link>
        <details className="mt-3"><summary className="cursor-pointer font-medium">Details for {r.account.name}</summary>
          {r.difference!==null&&<p className="mt-3 text-sm">Recorded closing balance − bank statement: {money(r.difference)}{r.difference!==0n?' · Review missing transactions, fees or starting balance.':' · Matches recorded closing balance.'}</p>}
          {r.older.length>0&&<div className="mt-3"><p className="font-medium">Older bills with unapplied balances</p><p className="mt-1 text-xs">May roll into later statements. Reference only; do not add these amounts to current debt.</p>{r.older.map(({statement,totals})=><p key={statement.id} className="mt-2 text-xs">Closed {statement.end_date} · Remaining {money(totals.remaining)} · {totals.status}</p>)}</div>}
          {r.unassignedPayments.length>0&&<p className="mt-3 text-xs">{r.unassignedPayments.length} repayment/transfer(s) are not linked to a statement. They already reduce current debt; use Billing & payments to review eligible payments.</p>}
          {[['Latest closed cycle',r.latestPurchases],['Next bill',r.nextPurchases]].map(([label,items])=><div key={String(label)} className="mt-3"><p className="font-medium">{String(label)} purchases</p>{(items as typeof r.latestPurchases).length?(items as typeof r.latestPurchases).map(t=><p key={t.id} className="mt-2 text-xs">{t.date} · {t.description||'Expense'} · {t.category_name??'Uncategorized'} · {money(BigInt(t.amount))}</p>):<p className="mt-1 text-xs">No recorded purchases in this range.</p>}</div>)}
        </details></div>}}
    ]
    return definitions.map(column=>({...column,meta:{headerClassName:'px-4 py-3 bg-soft whitespace-nowrap',cellClassName:'border-t border-line p-4 align-top'}}))
  },[currency])
  const table=useReactTable({data:rows,columns,getCoreRowModel:getCoreRowModel(),getRowId:r=>r.account.id})
  return <DataTable table={table} label="Credit cards by statement cycle" busy={loading}/>
}
