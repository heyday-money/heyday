import { flexRender, type RowData, type Table } from '@tanstack/react-table'
import { cn } from '../../lib/utils'

// Presentation metadata keeps semantic HTML and styling consistent across tables.
declare module '@tanstack/react-table' {
  interface ColumnMeta<TData extends RowData, TValue> {
    headerClassName?: string
    cellClassName?: string
    rowHeader?: boolean
  }
}

export function DataTable<TData>({ table, label, className, headerClassName, bodyClassName, rowClassName, regionLabel, busy = false }: {
  table: Table<TData>
  label: string
  className?: string
  headerClassName?: string
  bodyClassName?: string
  rowClassName?: string
  regionLabel?: string
  busy?: boolean
}) {
  return <div role="region" aria-label={regionLabel ?? `Scrollable ${label.toLowerCase()}`} aria-busy={busy} tabIndex={0} className="max-w-full overflow-x-auto rounded-2xl border border-line bg-card focus-visible:outline-2 focus-visible:outline-brand">
    <table aria-label={label} className={cn('w-full border-collapse text-left text-sm', className)}>
      <thead className={headerClassName}>{table.getHeaderGroups().map(group => <tr key={group.id}>{group.headers.map(header => <th key={header.id} scope="col" colSpan={header.colSpan} className={header.column.columnDef.meta?.headerClassName}>{header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}</th>)}</tr>)}</thead>
      <tbody className={bodyClassName}>{table.getRowModel().rows.map(row => <tr key={row.id} className={rowClassName}>{row.getVisibleCells().map(cell => {
        const meta = cell.column.columnDef.meta
        const Tag = meta?.rowHeader ? 'th' : 'td'
        return <Tag key={cell.id} scope={meta?.rowHeader ? 'row' : undefined} className={meta?.cellClassName}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</Tag>
      })}</tr>)}</tbody>
    </table>
  </div>
}
