import { AccountLabel } from './InstitutionLogo'
import { CardOutlookDetails } from './CardOutlookDetails'
import { type LoanContract } from '../lib/loans'
import { useLocalDate } from '../lib/useLocalDate'
import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { toast } from 'sonner'
import { Link } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'
import { desktopAvailable } from '../lib/desktop'
import { comparisonViews, plannerComparison, loanContractAmount, addMonths, currentCycle, cycleLabel, editableSatang, getPlanner, itemAmount, monthIndex, monthLabel, parsePlannerAmount, plannerItems, plannerMoney, savePlanner, type PlannerView, type PlannerCategoryId, type PlannerChange, type PlannerData, type PlannerItem } from '../lib/cashflow'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { NativeSelect } from './ui/native-select'
import { DataTable } from './ui/data-table'
import { FormField as Field } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog'

type Editor = { kind: 'item'; category: PlannerCategoryId; item?: PlannerItem } | { kind: 'amount'; item: PlannerItem; month: string } | { kind: 'opening' } | { kind: 'delete'; item: PlannerItem }
type Row = { contract?: LoanContract; parentId?: string; id: string; label: string; kind: 'section' | 'category' | 'item' | 'subtotal' | 'summary'; group?: 'income' | 'expenses' | 'cash'; category?: PlannerCategoryId; item?: PlannerItem; total?: 'opening' | 'netIncome' | 'expenses' | 'outflows' | 'surplus' | 'closing' | 'otherIn' | 'otherOut' }
const summaries = [
  ['opening', 'Opening Cash'], ['surplus', 'Cycle Surplus / Deficit'], ['closing', 'Cumulative Closing Cash'],
] as const
const errorMessage = (e: unknown) => typeof e === 'string' ? e : e instanceof Error ? e.message : 'Could not save your changes.'

function HelpTooltip({ label, text }: { label: string; text: string }) {
  return <Tooltip><TooltipTrigger asChild><Button type="button" variant="ghost" size="icon-xs" className="shrink-0 rounded-full text-muted" aria-label={`Help: ${label}`}><span aria-hidden="true" className="flex size-4 items-center justify-center rounded-full border border-current text-[10px]">?</span></Button></TooltipTrigger><TooltipContent>{text}</TooltipContent></Tooltip>
}
const categoryHelp: Record<PlannerCategoryId, string> = {
  income: 'Forecast uses gross estimates from Income plus manual forecast rows. Gross Actual is not recorded separately: net cash receipts appear below under Net Income / Cash Received. No gross amount is inferred from a net receipt.',
  deductions: 'Manage deductions on Salary sources in Income. Each deduction follows its salary’s recurrence and appears here separately from gross income. Cycle overrides replace estimates. Payroll loan payments belong here only. Retained manual deductions stay additive; avoid entering them again on a salary.',
  debt: 'Loan accounts and monthly installment estimates come from Accounts. The monthly amount applies once per payday cycle, including past cycles, unless overridden here. Clearing an override restores the estimate. A configured monthly installment replaces legacy schedules in that row. Enter only additional payments outside payroll for payroll-linked loans. Balances are reference-only; review duplicate manual rows.',
  installments: 'For cards with confirmed bills, explicitly included installment occurrences are shown in the bill instead. Saved cycle overrides are retained but suppressed for covered cycles. Other saved schedules are grouped by due date into payday cycles. They stop at the final payment. If a card has recorded payments in a cycle, its recorded total replaces linked installment forecasts for that cycle. This does not mark installments paid. Without payments, cycle overrides take precedence over schedules.',
  cards: 'Recorded cash/bank/wallet payments plus the remaining explicit statement payment plan. Partial payments reduce the plan, not a second expense. Plans needing review or with past dates/unavailable accounts are excluded. Cards without statements retain the legacy recorded-total rule. Manual rows remain additive.',
  expenses: 'Actual shows recorded cash/bank/wallet expenses by category through today. Credit card purchases belong in Expenses, not cash outflows. Forecast uses saved manual amounts and schedules; these never add to actuals.',
}
const summaryHelp = {
  opening: 'Initial cash, bank, and wallet balance, then the previous cycle’s closing cash. Missing opening cash is unavailable; zero is valid.',
  netIncome: 'Forecast: gross income minus payroll deductions. Actual: cash income received; payroll deductions are not subtracted again from net receipts.',
  otherIn: 'Actual transfers into cash/bank/wallet from outside that group. These are cash movements, not income or spending.',
  otherOut: 'Actual transfers out of cash/bank/wallet to investments or other non-cash accounts, excluding loan/card repayments already counted above.',
  expenses: 'Debt payments + card installments + other card payments + general expenses.',
  outflows: 'Income deductions + expenses paid from net income.',
  surplus: 'Forecast: net income minus planned expenses. Actual: recorded cash income plus other cash inflows minus expenses, repayments and other cash outflows. Internal cash transfers cancel; no planned amounts are added.',
  closing: 'Opening cash + cycle surplus or deficit. Includes intervening cycles outside this window and may include incomplete estimates.',
}

export function CashflowPlanner() {
  const [data, setData] = useState<PlannerData | null>(null)
  const [selected, setSelected] = useState('')
  const today = useLocalDate()
  const activeCycle = data ? currentCycle(data.period_start_day, today) : ''
  const visibleCycle = selected || activeCycle
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [editor, setEditor] = useState<Editor | null>(null)
  const request = useRef(0)
  async function load() {
    const current = ++request.current
    setError(null)
    try { const result = await getPlanner(); if (current === request.current) { setData(result) } }
    catch (e) { if (current === request.current) setError(errorMessage(e)) }
  }
  useEffect(() => {
    if (!desktopAvailable) return
    void load()
    const refresh = () => { void load() }
    window.addEventListener('transactions-changed', refresh)
    window.addEventListener('accounts-changed', refresh)
    window.addEventListener('incomes-changed', refresh)
    window.addEventListener('plans-changed', refresh)
    window.addEventListener('transaction-options-changed', refresh)
    window.addEventListener('focus', refresh)
    return () => {
      request.current++
      window.removeEventListener('transactions-changed', refresh)
      window.removeEventListener('accounts-changed', refresh)
      window.removeEventListener('incomes-changed', refresh)
      window.removeEventListener('plans-changed', refresh)
      window.removeEventListener('transaction-options-changed', refresh)
      window.removeEventListener('focus', refresh)
    }
  }, [])
  async function save(change: PlannerChange) {
    setBusy(true)
    const current = ++request.current
    try { const result = await savePlanner(change); if (current === request.current) setData(result); else await load(); toast.success('Planner saved.'); }
    finally { setBusy(false) }
  }
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">Open the desktop app to use your local cashflow planner.</p>
  if (!data) return error ? <div role="alert">{error}<Button onClick={load}>Retry planner</Button></div> : <p role="status">Loading planner…</p>
  return <section className="min-w-0" aria-labelledby="planner-title">
    {error && <div role="alert" className="mb-3 rounded-xl bg-soft p-3">Could not refresh the planner. Displayed figures may be out of date. <Button variant="outline" onClick={load}>Retry planner</Button></div>}
    <div className="mb-4 flex flex-wrap items-end justify-between gap-4"><div><h2 id="planner-title" className="text-2xl font-semibold">Cashflow Planner</h2></div>
      <div className="flex flex-wrap items-end gap-2"><Field label="First visible cycle"><Input type="month" min="0001-01" max="9999-05" value={visibleCycle} disabled={busy} onChange={e => { const value = e.target.value; if (/^\d{4}-\d{2}$/.test(value) && value >= '0001-01' && value <= '9999-05') setSelected(value) }} /></Field><Button variant="outline" onClick={() => setSelected('')}>Current cycle</Button><Button variant="outline" onClick={() => setEditor({ kind: 'opening' })}>Set opening cash</Button></div>
    </div>
    {data.source_currency !== 'THB' && (data.incomes.length > 0 || data.debt_accounts.length > 0 || data.installments.length > 0 || data.credit_cards.length > 0) && <p className="mb-3 rounded-xl bg-soft p-3">Linked sources use {data.source_currency ?? 'an unset currency'} and are excluded from this THB planner. No currency conversion is applied.</p>}
    {!data.opening && <p className="mb-4 rounded-xl bg-soft p-4">Enter opening cash and its initial cycle to calculate cumulative balances. Explicit zero is valid.</p>}
    <p className="mb-4 rounded-xl bg-soft p-3 text-sm">Compare the current cycle’s full Forecast with Actual recorded through today. These are separate views, never added together. Future cycles show Forecast; past cycles show Actual, with an option to compare saved forecasts.</p><TooltipProvider><PlannerTable data={data} activeCycle={activeCycle} selected={visibleCycle} busy={busy} edit={setEditor} /></TooltipProvider>
    <div className="mt-4 space-y-2 text-xs text-muted">
      <p>Actual income is money received in Cash / Bank / Wallet. Gross salary and payroll deductions show “Not recorded separately” until a receipt breakdown is recorded; deductions are not subtracted again from net receipts.</p>
      <p>General Expenses includes direct cash/bank/wallet spending. Card purchases appear in Expenses; recorded card payments reduce cash once under Credit Cards. Forecasts retain saved estimates and the existing bill-payment rules.</p>
      <p>Both views start from the same recorded opening cash. Future projections carry the current cycle’s full forecast once, not its actual plus forecast amounts. Balances depend on your opening-cash anchor and complete transaction records; actual cash does not reserve future spending.</p>
    </div>
    {editor && <PlannerDialog editor={editor} data={data} selected={visibleCycle} save={save} close={() => setEditor(null)} />}
  </section>
}

function PlannerTable({ data, selected, activeCycle, busy, edit }: { data: PlannerData; activeCycle: string; selected: string; busy: boolean; edit: (editor: Editor) => void }) {
  const [expandedLoans, setExpandedLoans] = useState<Set<string>>(new Set())
  const [comparePast, setComparePast] = useState(false)
  const cycles = plannerComparison(data, selected, activeCycle)
  const rows = useMemo<Row[]>(() => {
    const items = plannerItems(data)
    const categoryRows = (ids: PlannerCategoryId[]): Row[] => ids.flatMap(id => {
      const category = data.categories.find(category => category.id === id)
      if (!category) return []
      return [
        { id: `category:${id}`, kind: 'category', label: category.name, category: id },
        ...items.filter(item => item.category_id === id && !(id === 'income' && item.actual_key)).flatMap((item): Row[] => [
          { id: item.id, kind: 'item', label: item.name, item },
          ...(item.debt_account && expandedLoans.has(item.debt_account.id) ? (data.loan_contracts ?? []).filter(c => c.account_id === item.debt_account!.id).map(c => ({id:`contract:${c.id}`,kind:'item' as const,label:c.name,contract:c,parentId:item.id})) : []),
        ]),
        { id: `${id}-subtotal`, kind: 'subtotal', label: category.subtotal, category: id },
      ]
    })
    return [
      { id: 'section-income', kind: 'section', group: 'income', label: 'Income' },
      ...categoryRows(['income', 'deductions']),
      { id: 'net-after-deductions', kind: 'summary', group: 'income', label: 'Net Income / Cash Received', total: 'netIncome' },
      ...items.filter(item => item.category_id === 'income' && item.actual_key).map((item): Row => ({id:item.id,kind:'item',label:item.name,item})),
      { id: 'section-expenses', kind: 'section', group: 'expenses', label: 'Expenses' },
      ...categoryRows(['debt', 'installments', 'cards', 'expenses']),
      { id: 'total-expenses', kind: 'summary', group: 'expenses', label: 'Total Expenses', total: 'expenses' },
      { id: 'other-cash-in', kind: 'summary', label: 'Other Cash In', total: 'otherIn' },
      { id: 'other-cash-out', kind: 'summary', label: 'Other Cash Out', total: 'otherOut' },
      { id: 'total-outflows', kind: 'summary', label: 'Total Outflows Including Deductions', total: 'outflows' },
      { id: 'section-cash', kind: 'section', group: 'cash', label: 'Cash Balance' },
      ...summaries.map(([total, label]): Row => ({ id: `summary-${total}`, kind: 'summary', group: total === 'closing' ? 'cash' : undefined, label, total })),
    ]
  }, [data, expandedLoans])
  const rowStyle = (row: Row) => {
    const divider = row.kind === 'section' ? '[&>th]:border-t-4 [&>td]:border-t-4 [&>th]:py-3 [&>td]:py-3 ' : ''
    if (row.group === 'income') return divider + '[&>th]:bg-emerald-50 [&>td]:bg-emerald-50 dark:[&>th]:bg-emerald-950 dark:[&>td]:bg-emerald-950'
    if (row.group === 'expenses') return divider + '[&>th]:bg-orange-50 [&>td]:bg-orange-50 dark:[&>th]:bg-orange-950 dark:[&>td]:bg-orange-950'
    if (row.group === 'cash') return divider + '[&>th]:bg-soft [&>td]:bg-soft'
    return ''
  }
  const columns: ColumnDef<Row>[] = [
    { id: 'item', header: 'Item', meta: { rowHeader: true, headerClassName: 'sticky left-0 z-30 min-w-[240px] max-w-[280px] bg-card px-3 py-1.5 text-left border-b border-line', cellClassName: 'sticky left-0 z-10 min-w-[240px] max-w-[280px] bg-card px-3 py-1.5 border-b border-line text-left' }, cell: ({ row }) => {
      const entry = row.original
      if (entry.contract) return <span className="pl-5 text-muted">{entry.label} · schedule</span>
      if (entry.kind === 'section') return <h3 className={`text-sm font-bold ${entry.group === 'income' ? 'text-emerald-800 dark:text-emerald-200' : entry.group === 'expenses' ? 'text-orange-800 dark:text-orange-200' : 'text-brand'}`}>{entry.label}</h3>
      if (entry.kind === 'category') return <div className="flex items-center gap-1 text-brand"><span className="min-w-0 flex-1">{entry.label}</span><HelpTooltip label={entry.label} text={categoryHelp[entry.category!]} />{entry.category === 'deductions' ? <Button asChild size="xs" variant="outline"><Link to="/income" aria-label="Manage salary deductions in Income">Manage</Link></Button> : <Button size="xs" variant="outline" disabled={busy} aria-label={`Add item to ${entry.label}`} onClick={() => edit({ kind: 'item', category: entry.category! })}>Add</Button>}</div>
      if (!entry.item) return <div className="flex items-center justify-between gap-1 font-semibold"><span>{entry.label}</span><HelpTooltip label={entry.label} text={entry.total ? `Calculated: ${summaryHelp[entry.total]}` : 'Calculated sum of the amounts entered or scheduled in this category.'} /></div>
      const item = entry.item
      if (item.actual_key?.startsWith('actual:income:') && !item.actual_key.startsWith('actual:income:source:')) {
        const accountId = item.actual_key.slice('actual:income:'.length)
        return <Link to="/transactions" className="font-medium text-brand">Received · Unassigned · <AccountLabel id={accountId} name={data.ledger_transactions?.find(t => t.account_id === accountId)?.account_name ?? 'Account'} /></Link>
      }
      if (item.actual_key) return <div className="flex items-center gap-1"><Link to="/transactions" className="font-medium text-brand">{item.name}</Link><HelpTooltip label={item.name} text={item.description}/></div>
      if (item.debt_account && data.loan_facilities?.some(f => f.account_id === item.debt_account!.id)) return <div className="flex items-center gap-2"><Button size="icon-xs" variant="ghost" aria-label={`Expand ${item.name} contracts`} aria-expanded={expandedLoans.has(item.debt_account.id)} onClick={() => setExpandedLoans(current => {const next=new Set(current);if(next.has(item.debt_account!.id))next.delete(item.debt_account!.id);else next.add(item.debt_account!.id);return next})}>{expandedLoans.has(item.debt_account.id)?'−':'+'}</Button><Link to="/accounts/$accountId/loans" params={{accountId:item.debt_account.id}}><AccountLabel id={item.debt_account.id} name={item.name} /></Link><HelpTooltip label={item.name} text={item.description}/></div>
      if (item.credit_card) return <Link className="font-medium text-brand" to="/accounts/$accountId/billing" params={{accountId:item.credit_card.id}}><AccountLabel id={item.credit_card.id} name={item.name} /> · Billing</Link>
      if (item.income || item.debt_account || item.installment) {
        const route = item.income ? '/income' : item.debt_account ? '/accounts' : '/installments'
        const source = item.income ? 'Income' : item.debt_account ? 'Accounts' : 'Installments'
        return <div className="flex items-center justify-between gap-1"><Link to={route} className="min-w-0 truncate border-b border-dashed border-transparent pb-0.5 font-medium text-ink no-underline hover:border-current focus-visible:border-current dark:text-white">{item.debt_account ? <AccountLabel id={item.debt_account.id} name={item.name} /> : <>{item.card_name ? <><AccountLabel id={item.installment?.debt_account_id} name={item.card_name} /> · </> : ''}{item.name}</>}</Link><HelpTooltip label={item.name} text={`${item.name}. ${item.description}. Managed on ${source}; ${item.credit_card ? 'calculated from recorded transactions' : 'individual cycle amounts can be overridden'}.`} /></div>
      }
      const category = item.transaction_category_id ? ` Transaction category: ${data.expense_categories.find(c => c.id === item.transaction_category_id)?.name ?? 'Unavailable'}.` : ''
      return <div className="flex items-center gap-1"><button className="min-w-0 flex-1 truncate text-left font-medium underline decoration-dotted underline-offset-4 hover:text-brand" onClick={() => edit({ kind: 'item', category: item.category_id, item })} aria-label={`Edit ${item.name}`}>{item.card_name ? `${item.card_name} · ` : ''}{item.name}</button><HelpTooltip label={item.name} text={`${item.card_name ? `${item.card_name} · ` : ''}${item.name}. ${item.description || 'Select the item name to edit its details.'}${category}`} /><Button size="icon-xs" variant="ghost" className="text-muted" disabled={busy} aria-label={`Delete ${item.name}`} title={`Delete ${item.name}`} onClick={() => edit({ kind: 'delete', item })}><Trash2 aria-hidden="true" /></Button></div>
    } },
    ...cycles.map((cycle): ColumnDef<Row> => ({
      id: cycle.month,
      header: () => <><span className="block font-semibold">{monthLabel(cycle.month)}{cycle.month === activeCycle ? ' · Current cycle' : ''}</span><span className="mt-1 block text-[10px] font-normal text-muted">{cycleLabel(cycle.month, data.period_start_day)}</span></>,
      meta: { headerClassName: `border-b border-line px-2 py-3 text-center ${cycle.month === activeCycle ? 'bg-soft' : 'bg-card'}` },
      columns: comparisonViews(cycle.month, activeCycle, comparePast).map((view): ColumnDef<Row> => ({
        id: `${cycle.month}:${view}`,
        header: () => <><span className="block font-semibold">{view === 'forecast' ? 'Forecast' : 'Actual'}</span><span className="mt-1 block text-[10px] font-normal text-muted">{view === 'forecast' ? 'Full-cycle estimate' : cycle.month === activeCycle ? 'Recorded through today' : 'Recorded transactions'}</span></>,
        meta: { headerClassName: `min-w-[170px] border-b border-line px-2 py-2 text-right ${view === 'actual' ? 'bg-emerald-50 dark:bg-emerald-950' : 'bg-card'}`, cellClassName: `min-w-[170px] border-b border-line px-2 py-1.5 text-right tabular-nums ${view === 'actual' ? 'bg-emerald-50/40 dark:bg-emerald-950/30' : ''}` },
        cell: ({ row }) => {
          const entry = row.original, values = cycle[view]
          if (entry.kind === 'category' || entry.kind === 'section') return null
          if (entry.contract && view === 'actual') return <span title="Recorded repayments appear once on the parent loan account row.">—</span>
          if (entry.contract) return <span className="text-muted" title="Reference schedule only; included in the parent loan row.">{data.amounts.some(a => a.item_id === entry.parentId && a.month === cycle.month) ? 'Account override' : plannerMoney(loanContractAmount(data,entry.contract,cycle.month))}</span>
          if (entry.item) {
            const amount = itemAmount(data, entry.item, cycle.month, view)
            if (view === 'actual' || entry.item.actual_key) {
              const unrecorded = view === 'actual' && (entry.item.category_id === 'income' || entry.item.category_id === 'deductions') && !entry.item.actual_key
              return <span className="block px-2 text-xs tabular-nums" aria-label={`${entry.item.name} ${cycle.month}: ${amount.source}`} title={unrecorded ? 'Not recorded separately. Actual net receipts appear below; no gross salary or payroll deduction is inferred.' : `${amount.source}. Actuals come from Transactions; forecasts are preserved.`}>{amount.value === null ? '—' : plannerMoney(amount.value)}</span>
            }
            if (entry.item.credit_card || amount.source === 'Using recorded card total' || amount.source === 'Included in statement') return <Tooltip><TooltipTrigger asChild><button type="button" className="h-7 w-full rounded px-2 text-right text-xs tabular-nums" aria-label={`${entry.item.name} ${cycle.month}: ${amount.source}`}><span>{amount.source === 'Using recorded card total' ? '—' : plannerMoney(amount.value)}</span><span className="sr-only">{amount.source}</span></button></TooltipTrigger><TooltipContent>{amount.source === 'Using recorded card total' ? 'Recorded card payments replace this legacy installment forecast, without marking installments paid.' : amount.source === 'Included in statement' ? 'Explicitly included installment occurrences are covered by the bill; only uncovered occurrences remain here. Saved overrides are preserved.' : 'Forecast includes recorded cash payments plus remaining explicit statement plans. Actual is shown separately and is never added to this total.'}</TooltipContent></Tooltip>
            return <Button variant="outline" className="h-7 w-full justify-end px-2 py-1 text-xs font-normal" title={`${amount.source} · edit the forecast for this cycle`} disabled={busy} aria-label={`Edit ${entry.item.name} ${cycle.month} amount`} onClick={() => edit({kind:'amount',item:entry.item!,month:cycle.month})}><span>{amount.value === null ? '—' : plannerMoney(amount.value)}</span><span className="sr-only">{amount.source === 'Empty' ? 'Enter amount' : amount.source}</span></Button>
          }
          if (view === 'actual' && (entry.category === 'income' || entry.category === 'deductions')) return <span className="block text-xs text-muted" aria-label={`${entry.label} ${cycle.month}: Not recorded separately`}>—<span className="block text-[10px]">Not recorded separately</span></span>
          const value = !values.available ? null : entry.category ? values.buckets[entry.category] : values[entry.total!]
          return <span className={`font-semibold ${value !== null && value < 0n ? 'text-red-700 dark:text-red-400' : ''}`} title={entry.total === 'closing' ? view === 'forecast' ? 'Projected cash at cycle end' : 'Cash balance from recorded activity only' : undefined}>{plannerMoney(value)}</span>
        },
      })),
    })),
  ]
  const table = useReactTable({ data: rows, columns, getCoreRowModel: getCoreRowModel(), getRowId: row => row.id })
  const first = cycles[0]
  const views = comparisonViews(selected, activeCycle, comparePast)
  function summary(view: PlannerView) {
    const values = first[view]
    return <section key={view} aria-label={`${view === 'forecast' ? 'Forecast' : 'Actual'} summary`} className="rounded-2xl border border-line bg-card p-4">
      <h3 className="font-semibold">{monthLabel(selected)} · {view === 'forecast' ? 'Forecast — full cycle' : selected === activeCycle ? 'Actual — through today' : 'Actual — recorded activity'}</h3>
      <div className="mt-3 grid gap-4 sm:grid-cols-2">{([
        [view === 'forecast' ? 'Expected net income' : 'Net income received', values.netIncome],
        [view === 'forecast' ? 'Forecast expenses' : 'Recorded cash expenses & repayments', values.expenses],
        ['Cycle Surplus / Deficit', values.surplus],
        [view === 'forecast' ? 'Projected closing cash' : 'Cash balance from recorded activity', values.closing],
      ] as const).map(([label,value]) => <div key={label}><p className="text-xs text-muted">{label}</p><p className={`mt-1 text-lg font-semibold tabular-nums ${value !== null && value < 0n ? 'text-red-700 dark:text-red-400' : ''}`}>{plannerMoney(values.available ? value : null)}{values.available && value !== null && <span className="ml-1 text-xs">THB</span>}</p></div>)}</div>
      <p className="mt-3 text-xs text-muted">{!values.available ? 'Actuals require recorded data in THB.' : view === 'forecast' ? 'Estimate for the entire cycle; not added to Actual.' : 'Based on recorded transactions and your opening-cash anchor. Does not reserve future spending.'}</p>
    </section>
  }
  return <>
    {selected < activeCycle && <label className="mb-4 flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4" checked={comparePast} onChange={e => setComparePast(e.target.checked)} />Compare past forecasts</label>}
    <div className={`mb-5 grid gap-4 ${views.length === 2 ? 'lg:grid-cols-2' : ''}`}>{views.map(summary)}</div>
    <CardOutlookDetails data={data} months={cycles.map(c=>c.month)}/>
    <DataTable table={table} rowClassName={rowStyle} label="Seven-cycle cashflow planner in THB" className="border-separate border-spacing-0 text-xs" />
  </>
}

function PlannerDialog({ editor, data, selected, save, close }: { editor: Editor; data: PlannerData; selected: string; save: (change: PlannerChange) => Promise<void>; close: () => void }) {
  const item = editor.kind === 'item' || editor.kind === 'amount' || editor.kind === 'delete' ? editor.item : undefined
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null)
  const [dirty, setDirty] = useState(false), [discard, setDiscard] = useState(false)
  const [schedule, setSchedule] = useState(item?.schedule_amount !== null && item?.schedule_amount !== undefined)
  const [start, setStart] = useState(item?.schedule_start ?? selected)
  const [count, setCount] = useState(item?.schedule_start && item?.schedule_end ? monthIndex(item.schedule_end) - monthIndex(item.schedule_start) + 1 : 1)
  const installment = editor.kind === 'item' && editor.category === 'installments'
  let finalMonth = ''
  try { if (count >= 1 && count <= 600) finalMonth = addMonths(start, count - 1) } catch { /* Form validation shows the invalid range on submit. */ }
  function requestClose() { if (!busy) { if (dirty) setDiscard(true); else close() } }
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (busy) return
    const form = new FormData(event.currentTarget)
    setBusy(true); setError(null)
    try {
      let change: PlannerChange
      if (editor.kind === 'delete') change = { kind: 'delete', id: editor.item.id }
      else if (editor.kind === 'opening') change = { kind: 'opening', month: String(form.get('month')), amount: String(form.get('amount')).trim() ? parsePlannerAmount(String(form.get('amount')), true) : null }
      else if (editor.kind === 'amount') change = { kind: 'amount', item_id: editor.item.id, month: editor.month, amount: String(form.get('amount')).trim() ? parsePlannerAmount(String(form.get('amount'))) : null }
      else {
        if (schedule && installment && !finalMonth) throw new Error('Choose 1–600 installments and a valid start cycle.')
        change = { kind: 'item', id: item?.id ?? null, category_id: editor.category, name: String(form.get('name')).trim(), description: String(form.get('description')), card_name: String(form.get('card') ?? '').trim(), transaction_category_id: String(form.get('category') ?? '') || null,
          schedule_amount: schedule ? parsePlannerAmount(String(form.get('scheduleAmount'))) : null, schedule_start: schedule ? start : null, schedule_end: schedule ? installment ? finalMonth : String(form.get('end')) : null }
      }
      await save(change); close()
    } catch (e) { setError(errorMessage(e)) } finally { setBusy(false) }
  }
  const title = editor.kind === 'item' ? item ? 'Edit planner item' : 'Add planner item' : editor.kind === 'amount' ? `${item!.name} · ${monthLabel(editor.month)}` : editor.kind === 'opening' ? 'Initial opening cash' : 'Delete planner item?'
  return <Dialog open onOpenChange={open => { if (!open) requestClose() }}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!busy} onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }}><DialogHeader className="px-6 pt-6"><DialogTitle>{title}</DialogTitle><DialogDescription>{editor.kind === 'delete' ? `Delete “${item!.name}”, its schedule and all cycle entries? This affects historical planner totals.` : 'Amounts are in THB. Saving changes only this planner.'}</DialogDescription></DialogHeader>
    <form className="flex min-h-0 flex-col" onSubmit={submit} onChange={() => setDirty(true)}><fieldset disabled={busy} className="min-h-0 space-y-4 overflow-y-auto px-6 py-4">
      {editor.kind === 'item' && <>
        <Field label={installment ? 'Item / purchase name' : 'Item name'}><Input name="name" required maxLength={100} defaultValue={item?.name ?? ''} /></Field>
        {(installment || editor.category === 'cards') && <Field label="Card name"><Input name="card" maxLength={100} defaultValue={item?.card_name ?? ''} /></Field>}
        <Field label="Description (optional)"><Input name="description" maxLength={2000} defaultValue={item?.description ?? ''} /></Field>
        {editor.category === 'expenses' && <Field label="Transaction category"><NativeSelect name="category" defaultValue={item?.transaction_category_id ?? ''}><option value="">Uncategorized</option>{data.expense_categories.map(category => <option key={category.id} value={category.id} disabled={category.is_archived && category.id !== item?.transaction_category_id}>{category.name}{category.is_archived ? ' (archived)' : ''}</option>)}</NativeSelect></Field>}
        <Field label="Schedule"><NativeSelect value={schedule ? 'repeat' : 'manual'} onChange={e => setSchedule(e.target.value === 'repeat')}><option value="manual">Manual cycle entries</option><option value="repeat">{installment ? 'Installment schedule' : 'Repeat every cycle'}</option></NativeSelect></Field>
        {schedule && <><Field label={installment ? 'Monthly installment (THB)' : 'Recurring amount (THB)'}><Input name="scheduleAmount" required inputMode="decimal" defaultValue={editableSatang(item?.schedule_amount ?? null)} /></Field><Field label="First payment cycle"><Input type="month" required min="0001-01" max="9999-11" value={start} onChange={e => setStart(e.target.value)} /></Field>
          {installment ? <><Field label="Number of installments (including first)"><Input type="number" required min={1} max={600} value={count} onChange={e => setCount(Number(e.target.value))} /></Field><p className="text-sm">Final payment cycle: {finalMonth ? monthLabel(finalMonth) : 'Choose a valid range'}</p></> : <Field label="Final payment cycle (inclusive)"><Input name="end" type="month" required min={start} max="9999-11" defaultValue={item?.schedule_end ?? selected} /></Field>}
        </>}
        <p className="text-xs text-muted">Changing a schedule updates generated amounts in its range, including past cycles. Individually entered amounts always remain unchanged. Use an amount cell to change just one cycle.</p>
      </>}
      {editor.kind === 'amount' && <><p className="text-sm">{cycleLabel(editor.month, data.period_start_day)}</p><Field label="Cycle amount (THB)"><Input autoFocus name="amount" inputMode="decimal" defaultValue={editableSatang(itemAmount(data, editor.item, editor.month, 'forecast').value?.toString() ?? null)} /></Field><p className="text-xs text-muted">This entry changes only this cycle. Enter 0.00 to override a scheduled payment with zero. Clearing the field removes the override and uses the income estimate or schedule, if present.</p></>}
      {editor.kind === 'opening' && <><Field label="Initial tracking cycle"><Input name="month" type="month" min="0001-01" max="9999-11" required defaultValue={data.opening?.month ?? selected} /></Field><Field label="Available cash, bank, and wallet balance (THB)"><Input name="amount" inputMode="decimal" defaultValue={editableSatang(data.opening?.amount ?? null)} /></Field><p className="text-xs text-muted">Enter cash available at the start of this payday cycle. Zero is valid; leaving the amount blank makes cumulative balances unavailable. Changing this anchor recalculates all later closing balances.</p></>}
      {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{error}</p>}
    </fieldset><div className="border-t border-line px-6 py-4">{discard ? <><p className="mb-3" role="alert">Discard unsaved changes?</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>Keep editing</Button><Button type="button" variant="destructive" onClick={close}>Discard changes</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={busy} onClick={requestClose}>Cancel</Button><Button type="submit" disabled={busy} variant={editor.kind === 'delete' ? 'destructive' : 'default'}>{busy ? 'Saving…' : editor.kind === 'delete' ? 'Delete item' : 'Save'}</Button></DialogFooter>}</div></form>
  </DialogContent></Dialog>
}
