import { SelectiveDefaultWarning } from './SelectiveDefault'
import { exclusionFor } from '../lib/selective-defaults'
import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { CategoryIcon } from './CategoryIcon'
import { AccountLabel } from './InstitutionLogo'
import { CardOutlookDetails } from './CardOutlookDetails'
import { type LoanContract } from '../lib/loans'
import { useLocalDate } from '../lib/useLocalDate'
import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { toast } from 'sonner'
import { Link, useNavigate } from '@tanstack/react-router'
import { ChevronDown, ChevronRight, ReceiptText, Trash2 } from 'lucide-react'
import { ContextMenu, ContextMenuTrigger, ContextMenuContent, ContextMenuItem } from './ui/context-menu'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'
import { desktopAvailable } from '../lib/desktop'
import { itemExclusion, loanRepaymentSplit, comparisonViews, plannerComparison, loanContractAmount, addMonths, currentCycle, cycleLabel, editableSatang, getPlanner, itemAmount, monthIndex, monthLabel, parsePlannerAmount, plannerItems, plannerMoney, savePlanner, type PlannerView, type PlannerCategoryId, type PlannerChange, type PlannerData, type PlannerItem } from '../lib/cashflow'
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
const errorMessage = (e: unknown) => typeof e === 'string' ? e : e instanceof Error ? e.message : "Could not save your changes."

function HelpTooltip({ label, text }: { label: string; text: string }) {
  useLanguage()

  return <Tooltip><TooltipTrigger asChild><Button type="button" variant="ghost" size="icon-xs" className="shrink-0 rounded-full text-muted" aria-label={translate("Help: {value0}", { value0: label })}><span aria-hidden="true" className="flex size-4 items-center justify-center rounded-full border border-current text-[10px]">?</span></Button></TooltipTrigger><TooltipContent>{translate(text)}</TooltipContent></Tooltip>
}
const categoryHelp: Record<PlannerCategoryId, string> = {
  get income() { return translate("Forecast uses gross estimates from Income plus manual forecast rows. Gross Actual is not recorded separately: net cash receipts appear below under Net Income / Cash Received. No gross amount is inferred from a net receipt.") },
  get deductions() { return translate("Manage deductions on Salary sources in Income. Each deduction follows its salary’s recurrence and appears here separately from gross income. Cycle overrides replace estimates. Payroll loan payments belong here only. Retained manual deductions stay additive; avoid entering them again on a salary.") },
  get debt() { return translate("Loan schedules come from Accounts. Linked payroll deductions cover scheduled payments first; only the remaining amount is forecast here. Entered cycle amounts are additional payments outside payroll and replace the remaining estimate. Clearing an entry restores that estimate. Review unlinked manual rows for duplicates.") },
  get installments() { return translate("For cards with confirmed bills, explicitly included installment occurrences are shown in the bill instead. Saved cycle overrides are retained but suppressed for covered cycles. Other saved schedules are grouped by due date into payday cycles. They stop at the final payment. If a card has recorded payments in a cycle, its recorded total replaces linked installment forecasts for that cycle. This does not mark installments paid. Without payments, cycle overrides take precedence over schedules.") },
  get cards() { return translate("Recorded cash/bank/wallet payments plus the remaining explicit statement payment plan. Partial payments reduce the plan, not a second expense. Plans needing review or with past dates/unavailable accounts are excluded. Cards without statements retain the legacy recorded-total rule. Manual rows remain additive.") },
  get expenses() { return translate("Actual shows recorded cash/bank/wallet expenses by category through today. Credit card purchases belong in Expenses, not cash outflows. Forecast uses saved manual amounts and schedules; these never add to actuals.") },
}
const summaryHelp = {
  get opening() { return translate("Initial cash, bank, and wallet balance, then the previous cycle’s closing cash. Missing opening cash is unavailable; zero is valid.") },
  get netIncome() { return translate("Forecast: gross income minus payroll deductions. Actual: cash income received; payroll deductions are not subtracted again from net receipts.") },
  get otherIn() { return translate("Actual transfers into cash/bank/wallet from outside that group. These are cash movements, not income or spending.") },
  get otherOut() { return translate("Actual transfers out of cash/bank/wallet to investments or other non-cash accounts, excluding loan/card repayments already counted above.") },
  get expenses() { return translate("Debt payments + card installments + other card payments + general expenses.") },
  get outflows() { return translate("Income deductions + expenses paid from net income.") },
  get surplus() { return translate("Forecast: net income minus planned expenses. Actual: recorded cash income plus other cash inflows minus expenses, repayments and other cash outflows. Internal cash transfers cancel; no planned amounts are added.") },
  get closing() { return translate("Opening cash + cycle surplus or deficit. Includes intervening cycles outside this window and may include incomplete estimates.") },
}

export function CashflowPlanner() {
  useLanguage()

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
    try { const result = await savePlanner(change); if (current === request.current) setData(result); else await load(); toast.success(translate("Planner saved.")); }
    finally { setBusy(false) }
  }
  if (!desktopAvailable) return <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to use your local cashflow planner.")}</p>
  if (!data) return error ? <div role="alert">{translate(error)}<Button onClick={load}>{translate("Retry planner")}</Button></div> : <p role="status">{translate("Loading planner…")}</p>
  return <section className="min-w-0" aria-labelledby="planner-title">
    {error && <div role="alert" className="mb-3 rounded-xl bg-soft p-3">{translate("Could not refresh the planner. Displayed figures may be out of date.")}{" "}<Button variant="outline" onClick={load}>{translate("Retry planner")}</Button></div>}
    <div className="mb-4 flex flex-wrap items-end justify-between gap-4"><div><h2 id="planner-title" className="text-2xl font-semibold">{translate("Cashflow Planner")}</h2></div>
      <div className="flex flex-wrap items-end gap-2"><Field label={translate("First visible cycle")}><Input type="month" min="0001-01" max="9999-05" value={visibleCycle} disabled={busy} onChange={e => { const value = e.target.value; if (/^\d{4}-\d{2}$/.test(value) && value >= '0001-01' && value <= '9999-05') setSelected(value) }} /></Field><Button variant="outline" onClick={() => setSelected('')}>{translate("Current cycle")}</Button><Button variant="outline" onClick={() => setEditor({ kind: 'opening' })}>{translate("Set opening cash")}</Button></div>
    </div>
    {data.source_currency !== 'THB' && (data.incomes.length > 0 || data.debt_accounts.length > 0 || data.installments.length > 0 || data.credit_cards.length > 0) && <p className="mb-3 rounded-xl bg-soft p-3">{translate("Linked sources use")}{" "}{data.source_currency ?? translate("an unset currency")} {" "}{translate("and are excluded from this THB planner. No currency conversion is applied.")}</p>}
    <TooltipProvider><PlannerTable data={data} activeCycle={activeCycle} selected={visibleCycle} busy={busy} edit={setEditor} /></TooltipProvider>
    <details className="mt-4 rounded-xl border border-line p-4 text-xs text-muted"><summary className="cursor-pointer text-sm font-medium text-ink">{translate("How forecasts and actuals work")}</summary><div className="mt-3 space-y-2">
      <p>{translate("Actual income uses net cash, bank and wallet receipts. Gross salary and deductions are available in Salary payment history; payroll is not subtracted again from net receipts.")}</p>
      <p>{translate("General Expenses includes direct cash/bank/wallet spending. Card purchases appear in Expenses; recorded card payments reduce cash once under Credit Cards. Forecasts retain saved estimates and the existing bill-payment rules.")}</p>
      <p>{translate("Both views start from the same recorded opening cash. Future projections carry the current cycle’s full forecast once, not its actual plus forecast amounts. Balances depend on your opening-cash anchor and complete transaction records; actual cash does not reserve future spending.")}</p>
    </div>
    </details>
    {editor && <PlannerDialog editor={editor} data={data} selected={visibleCycle} save={save} close={() => setEditor(null)} />}
  </section>
}

function PlannerTable({ data, selected, activeCycle, busy, edit }: { data: PlannerData; activeCycle: string; selected: string; busy: boolean; edit: (editor: Editor) => void }) {
  useLanguage()

  const navigate = useNavigate()
  const [expandedLoans, setExpandedLoans] = useState<Set<string>>(new Set())
  const [comparePast, setComparePast] = useState(false)
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const toggleSection = (id: string) => setCollapsed(previous => {
    const next = new Set(previous)
    if (next.has(id)) next.delete(id); else next.add(id)
    return next
  })
  const collapseControl = (id: string, label: string) => <Button type="button" variant="ghost" size="icon-xs" aria-label={translate("{value0} {value1}", { value0: collapsed.has(id) ? translate("Expand") : translate("Collapse"), value1: label })} aria-expanded={!collapsed.has(id)} onClick={() => toggleSection(id)}>{collapsed.has(id) ? <ChevronRight className="size-4" /> : <ChevronDown className="size-4" />}</Button>
  const cycles = plannerComparison(data, selected, activeCycle)
  const rows = useMemo<Row[]>(() => {
    const archivedIds = new Set([...data.debt_accounts, ...data.credit_cards].filter(account => account.is_archived).map(account => account.id))
    const items = plannerItems(data).filter(item => !archivedIds.has(item.debt_account?.id ?? item.credit_card?.id ?? item.installment?.debt_account_id ?? ''))
    const categoryRows = (ids: PlannerCategoryId[]): Row[] => ids.flatMap(id => {
      const category = data.categories.find(category => category.id === id)
      if (!category) return []
      return [
        { id: `category:${id}`, kind: 'category', label: translate(category.name), category: id },
        ...(collapsed.has(`category:${id}`) ? [] : items.filter(item => item.category_id === id && !(id === 'income' && item.actual_key)).flatMap((item): Row[] => [
          { id: item.id, kind: 'item', label: item.name, item },
          ...(item.debt_account && expandedLoans.has(item.debt_account.id) ? (data.loan_contracts ?? []).filter(c => c.account_id === item.debt_account!.id).map(c => ({id:`contract:${c.id}`,kind:'item' as const,label:c.name,contract:c,parentId:item.id})) : []),
        ])),
        { id: `${id}-subtotal`, kind: 'subtotal', label: translate(category.subtotal), category: id },
      ]
    })
    return [
      { id: 'section-income', kind: 'section', group: 'income', label: translate("Income") },
      ...(collapsed.has('section-income') ? [] : categoryRows(['income', 'deductions'])),
      { id: 'net-after-deductions', kind: 'summary', group: 'income', label: translate("Net Income / Cash Received"), total: 'netIncome' },
      ...(collapsed.has('section-income') ? [] : items.filter(item => item.category_id === 'income' && item.actual_key).map((item): Row => ({id:item.id,kind:'item',label:item.name,item}))),
      { id: 'section-expenses', kind: 'section', group: 'expenses', label: translate("Expenses") },
      ...(collapsed.has('section-expenses') ? [] : categoryRows(['debt', 'installments', 'cards', 'expenses'])),
      { id: 'total-expenses', kind: 'summary', group: 'expenses', label: translate("Total Expenses"), total: 'expenses' },
      { id: 'other-cash-in', kind: 'summary', label: translate("Other Cash In"), total: 'otherIn' },
      { id: 'other-cash-out', kind: 'summary', label: translate("Other Cash Out"), total: 'otherOut' },
      { id: 'total-outflows', kind: 'summary', label: translate("Total Outflows Including Deductions"), total: 'outflows' },
      { id: 'section-cash', kind: 'section', group: 'cash', label: translate("Cash Balance") },
      ...summaries.map(([total, label]): Row => ({ id: `summary-${total}`, kind: 'summary', group: total === 'closing' ? 'cash' : undefined, label: translate(label), total })),
    ]
  }, [data, expandedLoans, collapsed, getLanguage()])
  const rowStyle = (row: Row) => {
    if (row.kind === 'subtotal' || (row.kind === 'summary' && !row.group)) return '[&>th]:bg-soft [&>td]:bg-soft [&>th]:border-t-2 [&>td]:border-t-2 [&>th]:border-line [&>td]:border-line [&>th]:py-2.5 [&>td]:py-2.5'
    const divider = row.kind === 'section' ? '[&>th]:border-t-4 [&>td]:border-t-4 [&>th]:py-3 [&>td]:py-3 ' : ''
    if (row.group === 'income') return divider + '[&>th]:bg-emerald-50 [&>td]:bg-emerald-50 dark:[&>th]:bg-emerald-950 dark:[&>td]:bg-emerald-950'
    if (row.group === 'expenses') return divider + '[&>th]:bg-orange-50 [&>td]:bg-orange-50 dark:[&>th]:bg-orange-950 dark:[&>td]:bg-orange-950'
    if (row.group === 'cash') return divider + '[&>th]:bg-soft [&>td]:bg-soft'
    return ''
  }
  const columns: ColumnDef<Row>[] = [
    { id: 'item', header: translate("Item"), meta: { rowHeader: true, headerClassName: 'sticky left-0 z-30 min-w-[240px] max-w-[280px] bg-card px-3 py-1.5 text-left border-b border-line', cellClassName: 'sticky left-0 z-10 min-w-[240px] max-w-[280px] bg-card px-3 py-1.5 border-b border-line text-left' }, cell: ({ row }) => {
      const entry = row.original
      if (entry.contract) return <span className="pl-5 text-muted">{entry.label} {" "}{translate("· schedule")}</span>
      if (entry.kind === 'section') return <h3 className={`flex items-center gap-1 text-sm font-bold ${entry.group === 'income' ? 'text-emerald-800 dark:text-emerald-200' : entry.group === 'expenses' ? 'text-orange-800 dark:text-orange-200' : 'text-brand'}`}>{entry.group !== 'cash' && collapseControl(entry.id, entry.label)}<span>{entry.label}</span></h3>
      if (entry.kind === 'category') return <div className="flex items-center gap-1 text-brand">{collapseControl(entry.id, entry.label)}<span className="min-w-0 flex-1">{entry.label}</span><HelpTooltip label={entry.label} text={categoryHelp[entry.category!]} />{entry.category === 'deductions' ? <Button asChild size="xs" variant="outline"><Link to="/income" aria-label={translate("Manage salary deductions in Income")}>{translate("Manage")}</Link></Button> : <Button size="xs" variant="outline" disabled={busy} aria-label={translate("Add item to {value0}", { value0: entry.label })} onClick={() => edit({ kind: 'item', category: entry.category! })}>{translate("Add")}</Button>}</div>
      if (!entry.item) return <div className="flex items-center justify-between gap-1 font-semibold"><span>{entry.label}</span><HelpTooltip label={entry.label} text={entry.total ? `Calculated: ${summaryHelp[entry.total]}` : translate("Calculated sum of the amounts entered or scheduled in this category.")} /></div>
      const item = entry.item
      const excluded = cycles.map(c => itemExclusion(data,item,c.month)).find(Boolean)
      const warning = <SelectiveDefaultWarning period={excluded} />
      const expenseIcon = item.category_id === 'expenses' ? <CategoryIcon name={data.expense_categories.find(category => category.id === item.transaction_category_id)?.icon} /> : null
      if (item.actual_key?.startsWith('actual:income:') && !item.actual_key.startsWith('actual:income:source:')) {
        const accountId = item.actual_key.slice('actual:income:'.length)
        return <Link to="/accounts/$accountId/details" params={{ accountId }} className="font-medium text-brand">{translate("Received · Unassigned ·")}{" "}<AccountLabel id={accountId} name={data.ledger_transactions?.find(t => t.account_id === accountId)?.account_name ?? translate("Account")} /></Link>
      }
      if (item.actual_key) return <div className="flex items-center gap-1">{expenseIcon}<Link to="/transactions" className={`font-medium text-brand ${item.category_id === 'expenses' ? 'min-w-0 flex-1' : ''}`}>{translate(item.name)}</Link><HelpTooltip label={translate(item.name)} text={item.description}/></div>
      if (item.debt_account) return <div className="flex items-center gap-2">{data.loan_facilities?.some(f => f.account_id === item.debt_account!.id) && <Button size="icon-xs" variant="ghost" aria-label={translate("Expand {value0} contracts", { value0: item.name })} aria-expanded={expandedLoans.has(item.debt_account.id)} onClick={() => setExpandedLoans(current => {const next=new Set(current);if(next.has(item.debt_account!.id))next.delete(item.debt_account!.id);else next.add(item.debt_account!.id);return next})}>{expandedLoans.has(item.debt_account.id)?'−':'+'}</Button>}<Link className="font-medium text-brand" to="/accounts/$accountId/details" params={{accountId:item.debt_account.id}}><AccountLabel id={item.debt_account.id} name={item.name} /></Link>{warning}<HelpTooltip label={item.name} text={item.description}/></div>
      if (item.credit_card) return <div className="flex items-center gap-1"><Link className="font-medium text-brand" to="/accounts/$accountId/details" params={{accountId:item.credit_card.id}}><AccountLabel id={item.credit_card.id} name={item.name} /></Link>{warning}</div>
      if (item.income || item.installment) {
        const route = item.income ? '/income' : '/installments'
        const source = item.income ? translate("Income") : translate("Installments")
        return <div className="flex items-center justify-between gap-1"><Link to={route} className="min-w-0 truncate border-b border-dashed border-transparent pb-0.5 font-medium text-ink no-underline hover:border-current focus-visible:border-current dark:text-white"><>{item.card_name ? <><AccountLabel id={item.installment?.debt_account_id} name={item.card_name} /> · </> : ''}{item.name}</></Link>{warning}<HelpTooltip label={item.name} text={translate("{value0}. {value1}. Managed on {value2}; {value3}.", { value0: item.name, value1: item.description, value2: source, value3: item.credit_card ? translate("calculated from recorded transactions") : translate("individual cycle amounts can be overridden") })} /></div>
      }
      const category = item.transaction_category_id ? ` Transaction category: ${data.expense_categories.find(c => c.id === item.transaction_category_id)?.name ?? translate("Unavailable")}.` : ''
      const itemHelp = <HelpTooltip label={item.name} text={`${item.card_name ? `${item.card_name} · ` : ''}${item.name}. ${item.description || translate("Select the item name to edit its details.")}${category}`} />
      return <div className="flex items-center gap-1">{expenseIcon}<button className="min-w-0 flex-1 truncate text-left font-medium underline decoration-dotted underline-offset-4 hover:text-brand" onClick={() => edit({ kind: 'item', category: item.category_id, item })} aria-label={translate("Edit {value0}", { value0: item.name })}>{item.card_name ? translate("{value0} · ", { value0: item.card_name }) : ''}{item.name}</button>{item.category_id !== 'expenses' && itemHelp}<Button size="icon-xs" variant="ghost" className="text-muted" disabled={busy} aria-label={translate("Delete {value0}", { value0: item.name })} title={translate("Delete {value0}", { value0: item.name })} onClick={() => edit({ kind: 'delete', item })}><Trash2 aria-hidden="true" /></Button>{item.category_id === 'expenses' && itemHelp}</div>
    } },
    ...cycles.map((cycle): ColumnDef<Row> => ({
      id: cycle.month,
      header: () => <><span className="block font-semibold">{monthLabel(cycle.month)}{cycle.month === activeCycle ? translate(" · Current cycle") : ''}</span><span className="mt-1 block text-[10px] font-normal text-muted">{cycleLabel(cycle.month, data.period_start_day)}</span></>,
      meta: { headerClassName: `border-b border-line px-2 py-3 text-center ${cycle.month === activeCycle ? 'bg-soft' : 'bg-card'}` },
      columns: comparisonViews(cycle.month, activeCycle, comparePast).map((view): ColumnDef<Row> => ({
        id: `${cycle.month}:${view}`,
        header: () => <><span className="block font-semibold">{view === 'forecast' ? translate("Forecast") : translate("Actual")}</span><span className="mt-1 block text-[10px] font-normal text-muted">{view === 'forecast' ? translate("Full-cycle estimate") : cycle.month === activeCycle ? translate("Recorded through today") : translate("Recorded transactions")}</span></>,
        meta: { headerClassName: `min-w-[170px] border-b border-line px-2 py-2 text-right ${view === 'actual' ? 'bg-emerald-50 dark:bg-emerald-950' : 'bg-card'}`, cellClassName: `min-w-[170px] border-b border-line px-2 py-1.5 text-right tabular-nums ${view === 'actual' ? 'bg-emerald-50/40 dark:bg-emerald-950/30' : ''}` },
        cell: ({ row }) => {
          const entry = row.original, values = cycle[view]
          if (entry.kind === 'category' || entry.kind === 'section') return null
          if (entry.contract && view === 'forecast' && exclusionFor(data.selective_defaults, entry.contract.account_id, cycle.month)) return <span title={translate("Excluded by Selective Default")}>—</span>
          if (entry.contract && view === 'actual') return <span title={translate("Recorded repayments appear once on the parent loan account row.")}>—</span>
          if (entry.contract) return <span className="text-muted" title={translate("Reference schedule only; included in the parent loan row.")}>{data.amounts.some(a => a.item_id === entry.parentId && a.month === cycle.month) ? translate("Account override") : plannerMoney(loanContractAmount(data,entry.contract,cycle.month))}</span>
          if (entry.item) {
            const amount = itemAmount(data, entry.item, cycle.month, view)
            if (view === 'forecast' && itemExclusion(data,entry.item,cycle.month)) return <Tooltip><TooltipTrigger asChild><button type="button" className="block w-full rounded px-2 text-right text-xs text-muted" aria-label={translate("{value0} {value1}: Excluded", { value0: entry.item.name, value1: cycle.month })}>—</button></TooltipTrigger><TooltipContent>{translate("Excluded by Selective Default")}. {translate("Recorded payments remain in Actual only during excluded cycles. Balances and saved plans are unchanged.")}</TooltipContent></Tooltip>
            if (view === 'actual' || entry.item.actual_key) {
              if (view === 'actual' && entry.item.debt_account) {
                const split = loanRepaymentSplit(data, cycle.month, entry.item.debt_account.id)
                if (split) return <Tooltip><TooltipTrigger asChild><button type="button" className="block w-full rounded px-2 text-right text-xs tabular-nums" aria-label={translate("Total repayment") + ': ' + plannerMoney(amount.value ?? 0n)}>{plannerMoney(amount.value ?? 0n)}</button></TooltipTrigger><TooltipContent><p className="font-semibold">{translate("Total repayment")}</p><p>{translate("Principal")}: {plannerMoney(split.principal)}</p><p>{translate("Interest")}: {plannerMoney(split.interest)}</p><p>{translate("Fees")}: {plannerMoney(split.fee)}</p>{split.other !== 0n && <p>{translate("Other repayments")}: {plannerMoney(split.other)}</p>}<p>{translate("Linked interest and fees are included here, not counted again in General Expenses.")}</p></TooltipContent></Tooltip>
              }
              const unrecorded = view === 'actual' && (entry.item.category_id === 'income' || entry.item.category_id === 'deductions') && !entry.item.actual_key
              return <span className="block px-2 text-xs tabular-nums" aria-label={translate("{value0} {value1}: {value2}", { value0: entry.item.name, value1: cycle.month, value2: amount.source })} title={unrecorded ? translate("Not shown separately here. Actual net receipts appear below; open Salary payment history for recorded payslip breakdowns.") : translate("{value0}. Actuals come from Transactions; forecasts are preserved.", { value0: amount.source })}>{amount.value === null ? '—' : plannerMoney(amount.value)}</span>
            }
            if (amount.source === translate("Loan paid off")) return <span className="block px-2 text-xs text-muted" title={translate("Paid off; saved forecast entries are preserved but excluded.")}>{translate("Paid off")}</span>
            if (entry.item.credit_card || amount.source === translate("Using recorded card total") || amount.source === translate("Included in statement")) return <Tooltip><TooltipTrigger asChild><button type="button" className="h-7 w-full rounded px-2 text-right text-xs tabular-nums" aria-label={translate("{value0} {value1}: {value2}", { value0: entry.item.name, value1: cycle.month, value2: amount.source })}><span>{amount.source === translate("Statement needs review") && amount.value === 0n ? translate("Review bill") : amount.source === translate("Using recorded card total") ? '—' : plannerMoney(amount.value)}</span><span className="sr-only">{amount.source}</span></button></TooltipTrigger><TooltipContent>{amount.source === translate("Statement needs review") ? translate("A saved statement needs review after a change. Its payment plan is excluded until you review and confirm the bill. Any recorded payments and other eligible plans remain included.") : amount.source === translate("Using recorded card total") ? translate("Recorded card payments replace this legacy installment forecast, without marking installments paid.") : amount.source === translate("Included in statement") ? translate("Explicitly included installment occurrences are covered by the bill; only uncovered occurrences remain here. Saved overrides are preserved.") : translate("Forecast includes recorded cash payments plus remaining explicit statement plans. Actual is shown separately and is never added to this total.")}</TooltipContent></Tooltip>
            return <Button variant="outline" className="h-7 w-full justify-end px-2 py-1 text-xs font-normal" title={translate("{value0} · edit the forecast for this cycle", { value0: amount.source })} disabled={busy} aria-label={translate("Edit {value0} {value1} amount", { value0: entry.item.name, value1: cycle.month })} onClick={() => edit({kind:'amount',item:entry.item!,month:cycle.month})}><span>{amount.value === null ? '—' : plannerMoney(amount.value)}</span><span className="sr-only">{amount.source === translate("Empty") ? translate("Enter amount") : amount.source}</span></Button>
          }
          if (view === 'actual' && (entry.category === 'income' || entry.category === 'deductions')) return <span className="block text-xs text-muted" aria-label={translate("{value0} {value1}: Not shown separately", { value0: entry.label, value1: cycle.month })}>—<span className="block text-[10px]">{translate("Not shown separately")}</span></span>
          const value = !values.available ? null : entry.category ? values.buckets[entry.category] : values[entry.total!]
          return <span className={`font-semibold ${value !== null && value < 0n ? 'text-red-700 dark:text-red-400' : ''}`} title={entry.total === 'closing' ? view === 'forecast' ? translate("Projected cash at cycle end") : translate("Cash balance from recorded activity only") : undefined}>{plannerMoney(value)}</span>
        },
      })),
    })),
  ]
  const table = useReactTable({ data: rows, columns, getCoreRowModel: getCoreRowModel(), getRowId: row => row.id })
  const first = cycles[0]
  const views = comparisonViews(selected, activeCycle, comparePast)
  function summary(view: PlannerView) {
    const values = first[view]
    return <section key={view} aria-label={translate("{value0} summary", { value0: view === 'forecast' ? translate("Forecast") : translate("Actual") })} className="rounded-2xl border border-line bg-card p-4">
      <h3 className="font-semibold">{monthLabel(selected)} · {view === 'forecast' ? translate("Forecast — full cycle") : selected === activeCycle ? translate("Actual — through today") : translate("Actual — recorded activity")}</h3>
      <div className="mt-3 grid gap-4 sm:grid-cols-2">{([
        [view === 'forecast' ? translate("Expected net income") : translate("Net income received"), values.netIncome],
        [view === 'forecast' ? translate("Forecast expenses") : translate("Recorded cash expenses & repayments"), values.expenses],
        [translate('Cycle Surplus / Deficit'), values.surplus],
        [view === 'forecast' ? translate("Projected closing cash") : translate("Cash balance from recorded activity"), values.closing],
      ] as const).map(([label,value]) => <div key={label}><p className="text-xs text-muted">{label}</p><p className={`mt-1 text-xl font-semibold tabular-nums ${value !== null && value < 0n ? 'text-red-700 dark:text-red-400' : 'text-ink'}`}>{plannerMoney(values.available ? value : null)}{values.available && value !== null && <span className="ml-1 text-xs">THB</span>}</p></div>)}</div>
      <p className="mt-3 text-xs text-muted">{!values.available ? translate("Actuals require recorded data in THB.") : view === 'forecast' ? translate("Estimate for the entire cycle; not added to Actual.") : translate("Based on recorded transactions and your opening-cash anchor. Does not reserve future spending.")}</p>
    </section>
  }
  return <>
    {selected < activeCycle && <label className="mb-4 flex items-center gap-2 text-sm"><Input type="checkbox" className="size-4" checked={comparePast} onChange={e => setComparePast(e.target.checked)} />{translate("Compare past forecasts")}</label>}
    <div className={`mb-5 grid gap-4 ${views.length === 2 ? 'lg:grid-cols-2' : ''}`}>{views.map(summary)}</div>
    <CardOutlookDetails data={data} months={cycles.map(c=>c.month)}/>
    <DataTable table={table} rowClassName={rowStyle} wrapRow={(row, element) => row.item?.credit_card ? <ContextMenu key={row.id}><ContextMenuTrigger asChild tabIndex={0}>{element}</ContextMenuTrigger><ContextMenuContent aria-label={`${row.item.credit_card.name} actions`}><ContextMenuItem onSelect={() => void navigate({to:'/accounts/$accountId/billing',params:{accountId:row.item!.credit_card!.id}})}><ReceiptText aria-hidden="true" />{translate("Billing")}</ContextMenuItem></ContextMenuContent></ContextMenu> : element} label={translate("Seven-cycle cashflow planner in THB")} className="border-separate border-spacing-0 text-xs" />
  </>
}

function PlannerDialog({ editor, data, selected, save, close }: { editor: Editor; data: PlannerData; selected: string; save: (change: PlannerChange) => Promise<void>; close: () => void }) {
  useLanguage()

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
        if (schedule && installment && !finalMonth) throw new Error("Choose 1–600 installments and a valid start cycle.")
        change = { kind: 'item', id: item?.id ?? null, category_id: editor.category, name: String(form.get('name')).trim(), description: String(form.get('description')), card_name: String(form.get('card') ?? '').trim(), transaction_category_id: String(form.get('category') ?? '') || null,
          schedule_amount: schedule ? parsePlannerAmount(String(form.get('scheduleAmount'))) : null, schedule_start: schedule ? start : null, schedule_end: schedule ? installment ? finalMonth : String(form.get('end')) : null }
      }
      await save(change); close()
    } catch (e) { setError(errorMessage(e)) } finally { setBusy(false) }
  }
  const title = editor.kind === 'item' ? item ? translate("Edit planner item") : translate("Add planner item") : editor.kind === 'amount' ? `${item!.name} · ${monthLabel(editor.month)}` : editor.kind === 'opening' ? translate("Initial opening cash") : translate("Delete planner item?")
  return <Dialog open onOpenChange={open => { if (!open) requestClose() }}><DialogContent className="flex max-h-[85dvh] flex-col overflow-hidden p-0 sm:max-w-[600px]" showCloseButton={!busy} onInteractOutside={e => e.preventDefault()} onEscapeKeyDown={e => { if (busy) e.preventDefault() }}><DialogHeader className="px-6 pt-6"><DialogTitle>{title}</DialogTitle><DialogDescription>{editor.kind === 'delete' ? translate("Delete “{value0}”, its schedule and all cycle entries? This affects historical planner totals.", { value0: item!.name }) : translate("Amounts are in THB. Saving changes only this planner.")}</DialogDescription></DialogHeader>
    <form className="flex min-h-0 flex-col" onSubmit={submit} onChange={() => setDirty(true)}><fieldset disabled={busy} className="min-h-0 space-y-4 overflow-y-auto px-6 py-4">
      {editor.kind === 'item' && <>
        <Field label={installment ? translate('Item / purchase name') : translate("Item name")}><Input name="name" required maxLength={100} defaultValue={item?.name ?? ''} /></Field>
        {(installment || editor.category === 'cards') && <Field label={translate("Card name")}><Input name="card" maxLength={100} defaultValue={item?.card_name ?? ''} /></Field>}
        <Field label={translate("Description (optional)")}><Input name="description" maxLength={2000} defaultValue={item?.description ?? ''} /></Field>
        {editor.category === 'expenses' && <Field label={translate("Transaction category")}><NativeSelect name="category" defaultValue={item?.transaction_category_id ?? ''}><option value="">{translate("Uncategorized")}</option>{data.expense_categories.map(category => <option key={category.id} value={category.id} disabled={category.is_archived && category.id !== item?.transaction_category_id}>{category.name}{category.is_archived ? translate(" (archived)") : ''}</option>)}</NativeSelect></Field>}
        <Field label={translate("Schedule")}><NativeSelect value={schedule ? 'repeat' : 'manual'} onChange={e => setSchedule(e.target.value === 'repeat')}><option value="manual">{translate("Manual cycle entries")}</option><option value="repeat">{installment ? translate("Installment schedule") : translate("Repeat every cycle")}</option></NativeSelect></Field>
        {schedule && <><Field label={installment ? translate("Monthly installment (THB)") : translate("Recurring amount (THB)")}><Input name="scheduleAmount" required inputMode="decimal" defaultValue={editableSatang(item?.schedule_amount ?? null)} /></Field><Field label={translate("First payment cycle")}><Input type="month" required min="0001-01" max="9999-11" value={start} onChange={e => setStart(e.target.value)} /></Field>
          {installment ? <><Field label={translate("Number of installments (including first)")}><Input type="number" required min={1} max={600} value={count} onChange={e => setCount(Number(e.target.value))} /></Field><p className="text-sm">{translate("Final payment cycle:")}{" "}{finalMonth ? monthLabel(finalMonth) : translate("Choose a valid range")}</p></> : <Field label={translate("Final payment cycle (inclusive)")}><Input name="end" type="month" required min={start} max="9999-11" defaultValue={item?.schedule_end ?? selected} /></Field>}
        </>}
        <p className="text-xs text-muted">{translate("Changing a schedule updates generated amounts in its range, including past cycles. Individually entered amounts always remain unchanged. Use an amount cell to change just one cycle.")}</p>
      </>}
      {editor.kind === 'amount' && <><p className="text-sm">{cycleLabel(editor.month, data.period_start_day)}</p><Field label={translate("Cycle amount (THB)")}><Input autoFocus name="amount" inputMode="decimal" defaultValue={editableSatang(itemAmount(data, editor.item, editor.month, 'forecast').value?.toString() ?? null)} /></Field><p className="text-xs text-muted">{translate("This entry changes only this cycle. Enter 0.00 to override a scheduled payment with zero. Clearing the field removes the override and uses the income estimate or schedule, if present.")}</p></>}
      {editor.kind === 'opening' && <><Field label={translate("Initial tracking cycle")}><Input name="month" type="month" min="0001-01" max="9999-11" required defaultValue={data.opening?.month ?? selected} /></Field><Field label={translate("Available cash, bank, and wallet balance (THB)")}><Input name="amount" inputMode="decimal" defaultValue={editableSatang(data.opening?.amount ?? null)} /></Field><p className="text-xs text-muted">{translate("Enter cash available at the start of this payday cycle. Zero is valid; leaving the amount blank makes cumulative balances unavailable. Changing this anchor recalculates all later closing balances.")}</p></>}
      {error && <p role="alert" className="text-sm text-red-700 dark:text-red-400">{translate(error)}</p>}
    </fieldset><div className="border-t border-line px-6 py-4">{discard ? <><p className="mb-3" role="alert">{translate("Discard unsaved changes?")}</p><DialogFooter><Button type="button" variant="outline" onClick={() => setDiscard(false)}>{translate("Keep editing")}</Button><Button type="button" variant="destructive" onClick={close}>{translate("Discard changes")}</Button></DialogFooter></> : <DialogFooter><Button type="button" variant="outline" disabled={busy} onClick={requestClose}>{translate("Cancel")}</Button><Button type="submit" disabled={busy} variant={editor.kind === 'delete' ? 'destructive' : 'default'}>{busy ? translate("Saving…") : editor.kind === 'delete' ? translate("Delete item") : translate("Save")}</Button></DialogFooter>}</div></form>
  </DialogContent></Dialog>
}
