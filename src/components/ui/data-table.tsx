import { t as translate, useLanguage } from "../../lib/i18n"
import { flexRender, type RowData, type Table } from '@tanstack/react-table'
import { cn } from '../../lib/utils'
import type { MouseEvent, ReactElement, ReactNode } from 'react'

// Presentation metadata keeps semantic HTML and styling consistent across tables.
declare module '@tanstack/react-table' {
  interface ColumnMeta<TData extends RowData, TValue> {
    headerClassName?: string
    cellClassName?: string
    rowHeader?: boolean
  }
}

export function DataTable<TData>({ table, label, className, headerClassName, bodyClassName, rowClassName, onRowClick, wrapRow, regionLabel, containerClassName, busy = false }: {
  table: Table<TData>
  label: string
  className?: string
  headerClassName?: string
  bodyClassName?: string
  rowClassName?: string | ((row: TData) => string)
  onRowClick?: (row: TData, event: MouseEvent<HTMLTableRowElement>) => void
  wrapRow?: (row: TData, element: ReactElement) => ReactNode
  regionLabel?: string
  containerClassName?: string
  busy?: boolean
}) {
  useLanguage()

  return <div role="region" aria-label={regionLabel ?? translate("Scrollable {value0}", { value0: label.toLowerCase() })} aria-busy={busy} tabIndex={0} className={cn("max-w-full overflow-x-auto rounded-2xl border border-line bg-card focus-visible:outline-2 focus-visible:outline-brand", containerClassName)}>
    <table aria-label={label} className={cn('w-full border-collapse text-left text-[13px] tabular-nums', className)}>
      <thead className={headerClassName}>{table.getHeaderGroups().map(group => <tr key={group.id}>{group.headers.map(header => <th key={header.id} aria-sort={header.column.getCanSort() ? header.column.getIsSorted() === 'asc' ? 'ascending' : header.column.getIsSorted() === 'desc' ? 'descending' : 'none' : undefined} scope={header.colSpan > 1 ? 'colgroup' : 'col'} colSpan={header.colSpan} className={header.column.columnDef.meta?.headerClassName}>{header.isPlaceholder ? null : typeof header.column.columnDef.header === 'string' ? translate(header.column.columnDef.header) : flexRender(header.column.columnDef.header, header.getContext())}</th>)}</tr>)}</thead>
      <tbody className={bodyClassName}>{table.getRowModel().rows.map(row => {
        const element = <tr key={row.id} onClick={onRowClick ? event => onRowClick(row.original, event) : undefined} className={typeof rowClassName === 'function' ? rowClassName(row.original) : rowClassName}>{row.getVisibleCells().map(cell => {
        const meta = cell.column.columnDef.meta
        const Tag = meta?.rowHeader ? 'th' : 'td'
        return <Tag key={cell.id} scope={meta?.rowHeader ? 'row' : undefined} className={meta?.cellClassName}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</Tag>
      })}</tr>
        return wrapRow ? wrapRow(row.original, element) : element
      })}</tbody>
    </table>
  </div>
}
