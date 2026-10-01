import { getLocale } from "../lib/i18n"
import { t as translate, useLanguage, getLanguage } from "../lib/i18n"
import { AccountLabel } from './InstitutionLogo'
import { useMemo, useState } from 'react'
import { getCoreRowModel, useReactTable, type ColumnDef } from '@tanstack/react-table'
import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-react'
import type { FinancialData } from '../lib/desktop'
import { dateKey } from '../lib/financial'
import { installmentYearSummary } from '../lib/installments'
import { formatAmount } from '../lib/money'
import { DataTable } from './ui/data-table'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { FormField } from './FormField'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from './ui/dialog'

type SummaryRow = ReturnType<typeof installmentYearSummary>['total']
function monthLabel(month: string) {
  const [year, number] = month.split('-').map(Number)
  const date = new Date(0)
  date.setUTCFullYear(year, number - 1, 1)
  return date.toLocaleDateString(getLocale(), { month: 'short', year: 'numeric', timeZone: 'UTC' })
}
export function InstallmentCalendar({ data, today, loading }: { data: FinancialData; today: Date; loading: boolean }) {
  useLanguage()

  const currentMonth = dateKey(today).slice(0, 7)
  const defaultMonth = currentMonth > '9999-01' ? '9999-01' : currentMonth
  const [month, setMonth] = useState(defaultMonth)
  const summary = useMemo(() => installmentYearSummary(data.installments ?? [], month), [data.installments, month, getLanguage()])
  const rows = useMemo(() => [...summary.rows, summary.total], [summary, getLanguage()])
  const currency = data.settings.currency!
  const columns = useMemo<ColumnDef<SummaryRow>[]>(() => {
    const labelClass = 'sticky left-0 z-10 min-w-40 max-w-56 border-r border-line bg-card px-3 py-2.5 text-left align-top'
    const numberClass = 'min-w-28 whitespace-nowrap px-3 py-2.5 text-right tabular-nums'
    const amountCell = (value: bigint, total: boolean) => <span className={total ? 'font-bold' : ''}>{!total && value === 0n ? '—' : formatAmount(value.toString(), currency, false)}</span>
    return [
      { id: 'plan', header: translate("Item"), meta: { rowHeader: true, headerClassName: labelClass, cellClassName: labelClass }, cell: ({ row }) => <><span className={`block break-words ${row.original.total ? 'font-bold' : 'font-medium'}`}>{row.original.name}</span>{!row.original.total && <span className="mt-1 block break-words text-[11px] font-normal text-muted"><AccountLabel id={data.installments?.find(i => i.id === row.original.id)?.debt_account_id} name={row.original.accountName} /></span>}</> },
      ...summary.months.map((key, index): ColumnDef<SummaryRow> => ({ id: key, header: monthLabel(key), meta: { headerClassName: `${numberClass} ${key === currentMonth ? 'bg-soft text-brand' : ''}`, cellClassName: `${numberClass} ${key === currentMonth ? 'bg-soft/50' : ''}` }, cell: ({ row }) => amountCell(row.original.amounts[index], row.original.total) })),
      { id: 'remaining', header: translate("Remaining total"), meta: { headerClassName: `${numberClass} sticky right-0 z-10 border-l border-line bg-soft`, cellClassName: `${numberClass} sticky right-0 z-10 border-l border-line bg-soft font-semibold` }, cell: ({ row }) => amountCell(row.original.remaining, row.original.total) },
    ]
  }, [summary.months, currency, currentMonth, data.installments, getLanguage()])
  const table = useReactTable({ data: rows, columns, getRowId: row => row.id, getCoreRowModel: getCoreRowModel() })
  function move(offset: number) {
    const [year, number] = month.split('-').map(Number)
    const index = year * 12 + number - 1 + offset
    setMonth(`${String(Math.floor(index / 12)).padStart(4, '0')}-${String(index % 12 + 1).padStart(2, '0')}`)
  }
  return <Dialog onOpenChange={open => { if (open) setMonth(defaultMonth) }}>
    <DialogTrigger asChild><Button variant="outline" disabled={loading} aria-label={translate("Monthly installment summary")} title={translate("Monthly installment summary")}><CalendarDays size={16} aria-hidden="true" /><span className="hidden sm:inline">{translate("Monthly summary")}</span></Button></DialogTrigger>
    <DialogContent className="flex max-h-[85dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[calc(100vw-4rem)]">
      <DialogHeader className="shrink-0 border-b border-line px-6 py-5 pr-12"><DialogTitle>{translate("Monthly installment summary")}</DialogTitle><DialogDescription>{translate("Scheduled payments by item across 12 calendar months.")}</DialogDescription></DialogHeader>
      <div className="min-h-0 overflow-y-auto px-6 py-5" aria-busy={loading}>
        <div className="mb-5 flex flex-wrap items-end gap-2">
          <Button variant="outline" size="icon" aria-label={translate("Previous month")} disabled={month === '0001-01'} onClick={() => move(-1)}><ChevronLeft size={16} /></Button>
          <FormField label={translate("Start month")}><Input type="month" className="mt-2 w-44" min="0001-01" max="9999-01" value={month} onChange={event => { const value = event.target.value; if (/^(?!0000)\d{4}-(0[1-9]|1[0-2])$/.test(value) && value <= '9999-01') setMonth(value) }} /></FormField>
          <Button variant="outline" size="icon" aria-label={translate("Next month")} disabled={month === '9999-01'} onClick={() => move(1)}><ChevronRight size={16} /></Button>
          <Button variant="ghost" onClick={() => setMonth(defaultMonth)}>{translate("This month")}</Button>
        </div>
        <h3 className="mb-4 text-lg font-semibold" aria-live="polite">{monthLabel(summary.months[0])} – {monthLabel(summary.months[11])}</h3>
        {!summary.rows.length && <p className="mb-4 text-sm">{translate("No remaining installments from this month.")}</p>}
        <DataTable table={table} label={translate("Monthly installment schedule")} className="text-xs" headerClassName="border-b border-line bg-soft text-[11px] text-muted" bodyClassName="divide-y divide-line [&>tr:last-child]:border-t-2 [&>tr:last-child]:border-line [&>tr:last-child]:bg-soft/50" />
        <p className="mt-4 text-xs">{translate("Remaining total includes all scheduled payments from the selected start month through each plan’s end, including months beyond this view. It is not a confirmed outstanding balance. Payments are not matched to installments. Schedules with unavailable accounts remain visible here.")}</p>
      </div>
    </DialogContent>
  </Dialog>
}
