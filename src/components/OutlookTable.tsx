import { useMemo, useState } from 'react'
import { getCoreRowModel, getExpandedRowModel, useReactTable, type ColumnDef, type ExpandedState } from '@tanstack/react-table'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { monthlyOutlook, type FlowKey } from '../lib/financial'
import { formatAmount } from '../lib/money'
import { Button } from './ui/button'
import { DataTable } from './ui/data-table'

type Period = ReturnType<typeof monthlyOutlook>[number]
type OutlookRow = { id: string; label: string } & (
  | { kind: 'opening' | 'net' | 'closing' | 'empty' }
  | { kind: 'flow'; flow: FlowKey; subRows: OutlookRow[] }
  | { kind: 'detail'; flow: FlowKey; detailKey: string }
)
const flows: { key: FlowKey; label: string }[] = [
  { key: 'income', label: 'Income' }, { key: 'expenses', label: 'Cash expenses' },
  { key: 'repayments', label: 'Debt repayments' }, { key: 'other', label: 'Other cash movements' },
]
const labelClass = 'sticky left-0 z-10 min-w-[180px] border-b border-line bg-card px-4 py-4 text-left align-top font-medium'

export function OutlookTable({ periods, currency, tomorrow, onPlan, loading }: {
  periods: Period[]; currency: string; tomorrow: string; onPlan: (date: string) => void; loading: boolean
}) {
  const [expanded, setExpanded] = useState<ExpandedState>({})
  const data = useMemo<OutlookRow[]>(() => [
    { id: 'opening', kind: 'opening', label: 'Opening cash balance' },
    ...flows.map(({ key, label }): OutlookRow => {
      const details = new Map<string, string>()
      for (const period of periods) for (const detail of period.buckets[key].details.values()) details.set(detail.key, detail.label)
      return { id: key, kind: 'flow', flow: key, label, subRows: details.size
        ? Array.from(details, ([detailKey, detailLabel]) => ({ id: `${key}:${detailKey}`, kind: 'detail', flow: key, detailKey, label: detailLabel }))
        : [{ id: `${key}:empty`, kind: 'empty', label: `No ${label.toLowerCase()} to break down in these periods.` }] }
    }),
    { id: 'net', kind: 'net', label: 'Net cash flow' },
    { id: 'closing', kind: 'closing', label: 'Closing cash balance' },
  ], [periods])
  const columns = useMemo<ColumnDef<OutlookRow>[]>(() => [
    { id: 'label', header: 'Cash flow', meta: { rowHeader: true, headerClassName: labelClass, cellClassName: labelClass }, cell: ({ row }) => {
      const item = row.original
      if (item.kind === 'flow') return <button className="flex items-center gap-1 text-left hover:text-brand" aria-expanded={row.getIsExpanded()} onClick={row.getToggleExpandedHandler()}>{row.getIsExpanded() ? <ChevronDown size={15} aria-hidden="true" /> : <ChevronRight size={15} aria-hidden="true" />}{item.label}</button>
      return <span className={item.kind === 'detail' || item.kind === 'empty' ? 'block pl-4 text-xs font-normal text-muted' : item.kind === 'closing' ? 'font-semibold' : ''}>{item.label}</span>
    } },
    ...periods.map((period, index): ColumnDef<OutlookRow> => {
      const cellClass = `min-w-[180px] border-b border-line px-4 py-4 text-right align-top tabular-nums ${index === 0 ? 'bg-soft/50' : ''}`
      return { id: period.fromKey, meta: { headerClassName: cellClass, cellClassName: cellClass }, header: () => <>
        <span className="block text-xs text-muted">{index === 0 ? 'Current' : `+${index}`}</span>
        <span className="block font-semibold">{period.from.toLocaleDateString(undefined, { month: 'short', year: 'numeric' })}</span>
        <span className="mt-1 block text-xs font-normal text-muted">{period.from.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })} – {new Date(period.to.getFullYear(), period.to.getMonth(), period.to.getDate() - 1).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}</span>
        {period.remainingDays && <Button size="sm" variant="ghost" className="mt-2" aria-label={`Plan for ${period.from.toLocaleDateString(undefined, { month: 'long', year: 'numeric' })}`} onClick={() => onPlan(period.fromKey > tomorrow ? period.fromKey : tomorrow)}>Plan</Button>}
      </>, cell: ({ row }) => <PeriodCell row={row.original} period={period} current={index === 0} previousPartial={index > 0 && periods[index - 1].partial} currency={currency} /> }
    }),
  ], [periods, currency, tomorrow, onPlan])
  const table = useReactTable({ data, columns, state: { expanded }, onExpandedChange: setExpanded, getRowId: row => row.id,
    getSubRows: row => row.kind === 'flow' ? row.subRows : undefined,
    getCoreRowModel: getCoreRowModel(), getExpandedRowModel: getExpandedRowModel(), autoResetExpanded: false })
  return <DataTable table={table} label="Cash outlook for the current payday cycle and six future cycles" regionLabel="Monthly outlook comparison" busy={loading} />
}

function PeriodCell({ row, period, current, previousPartial, currency }: { row: OutlookRow; period: Period; current: boolean; previousPartial: boolean; currency: string }) {
  const money = (value: bigint) => formatAmount(value.toString(), currency)
  switch (row.kind) {
    case 'empty': return null
    case 'opening': return <>{money(period.opening)}<span className="mt-1 block text-xs text-muted">{current ? 'Reconstructed' : previousPartial ? 'From partial forecast' : 'Previous closing forecast'}</span></>
    case 'net': return <>{current && <div>{money(period.actualNet)}<span className="ml-1 text-xs text-muted">Actual</span></div>}<div className={current ? 'mt-2' : ''}>{money(period.forecastNet)}<span className="mt-1 block text-xs text-muted">Remaining known flows</span></div></>
    case 'closing': return <div className={`font-semibold ${period.closing < 0n ? 'text-red-700 dark:text-red-400' : ''}`}>{money(period.closing)}<span className="mt-1 block text-xs font-normal text-muted">{period.partial ? 'Partial forecast · missing plans' : 'Forecast · known plans'}</span></div>
    case 'detail': {
      const detail = period.buckets[row.flow].details.get(row.detailKey)
      return <div className="text-xs">{current && <div>{money(detail?.actual ?? 0n)} Actual</div>}<div className="mt-1 text-muted">{money(detail?.forecast ?? 0n)} {row.flow === 'income' ? 'Expected' : 'Planned'}</div></div>
    }
    case 'flow': {
      const bucket = period.buckets[row.flow]
      const missing = row.flow === 'expenses' ? period.missingExpenses : row.flow === 'repayments' ? period.missingRepayments : false
      return <>{current && <div>{money(bucket.actual)}<span className="ml-1 text-xs text-muted">Actual</span></div>}
        <div className={current ? 'mt-2 text-xs text-muted' : ''}>{missing ? 'Not planned' : money(bucket.forecast)}<span className="mt-1 block text-xs text-muted">{missing ? 'Add a payment plan' : row.flow === 'income' ? 'Expected after today' : row.flow === 'other' ? 'No future transfers assumed' : !period.remainingDays ? 'No days remaining' : 'Planned'}</span></div>
      </>
    }
  }
}
