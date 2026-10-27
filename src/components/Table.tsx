import { useMemo, type ReactNode } from 'react';
import { useReactTable, getCoreRowModel, flexRender, type ColumnDef } from '@tanstack/react-table';
import { type Row, s } from '../api/client';
import { columnLabel, enumLabel, t } from '../i18n';
import { arabicDigits, money, useStore } from '../app/store';
import { displayDate, formatDecimal, localDigits } from '../lib/numbers';
import { Button, Empty, Status } from './ui';
const enums = new Set([
  'action',
  'entity_type',
  'kind',
  'status',
  'progress',
  'document_kind',
  'account_kind',
  'control_kind',
  'direction',
  'movement_type',
  'method',
  'from_status',
  'to_status',
]);
export function CellValue({ field, value }: { field: string; value: string | null | undefined }) {
  const b = useStore((s) => s.bootstrap);
  const ar = arabicDigits();
  if (value == null || value === '') return <span className="text-slate-400">—</span>;
  if (field === 'status' || field === 'progress') return <Status value={value} />;
  if (enums.has(field)) return <>{enumLabel(value)}</>;
  const lookup: Record<string, string> = {
    warehouse_id: 'warehouses',
    cash_account_id: 'cash_accounts',
    bank_account_id: 'cash_accounts',
    account_id: 'accounts',
    category_id: 'categories',
    unit_id: 'units',
  };
  if (lookup[field] && b) {
    const list = b[lookup[field] as keyof typeof b];
    if (Array.isArray(list)) {
      const r = (list as Row[]).find((x) => s(x, 'id') === value);
      if (r) return <>{s(r, 'name_ar', s(r, 'name'))}</>;
    }
  }
  if (
    ['active', 'is_customer', 'is_supplier', 'is_postable', 'must_change_password'].includes(field)
  )
    return <>{t(value === '1' ? 'yes' : 'no')}</>;
  if (/^-?\d+$/.test(value)) {
    if (field.endsWith('_minor') || field === 'standard_cost')
      return <bdi className="number">{money(value)}</bdi>;
    if (field.endsWith('_micros') || field === 'min_stock')
      return <bdi className="number">{formatDecimal(value, 6, ar, true)}</bdi>;
    if (field.endsWith('_bps')) return <bdi>{formatDecimal(value, 2, ar, true)}%</bdi>;
    if (field === 'unit_cost_micropiastres') return <bdi>{formatDecimal(value, 8, ar, true)}</bdi>;
    return <bdi>{localDigits(value, ar)}</bdi>;
  }
  if (
    (field.endsWith('_date') ||
      field.endsWith('_at') ||
      ['starts_on', 'ends_on', 'locked_until'].includes(field)) &&
    /^\d{4}-\d\d-\d\d/.test(value)
  )
    return (
      <bdi>
        {displayDate(value, ar)}
        {value.includes('T') ? ' ' + localDigits(value.slice(11, 16), ar) : ''}
      </bdi>
    );
  return <span className="whitespace-pre-wrap break-words">{value}</span>;
}
export interface TableColumn {
  key: string;
  label?: string;
  render?: (row: Row) => ReactNode;
}
export function DataTable({
  rows,
  columns,
  onRow,
  actions,
  compact = false,
}: {
  rows: Row[];
  columns: (string | TableColumn)[];
  onRow?: (row: Row) => void;
  actions?: (row: Row) => ReactNode;
  compact?: boolean;
}) {
  const cols = useMemo<ColumnDef<Row>[]>(
    () => [
      ...columns.map((item) => {
        const c = typeof item === 'string' ? { key: item } : item;
        return {
          id: c.key,
          accessorKey: c.key,
          header: c.label ? t(c.label) : columnLabel(c.key),
          cell: ({ row }: { row: { original: Row } }) =>
            c.render ? (
              c.render(row.original)
            ) : (
              <CellValue field={c.key} value={row.original[c.key]} />
            ),
        };
      }),
      ...(actions
        ? [
            {
              id: 'actions',
              header: t('actions'),
              cell: ({ row }: { row: { original: Row } }) => (
                <div className="flex items-center gap-2">{actions(row.original)}</div>
              ),
            },
          ]
        : []),
    ],
    [columns, actions],
  );
  const table = useReactTable({ data: rows, columns: cols, getCoreRowModel: getCoreRowModel() });
  if (!rows.length) return <Empty />;
  return (
    <div className="table-scroll">
      <table className={`data-table ${compact ? 'compact' : ''}`}>
        <thead>
          {table.getHeaderGroups().map((g) => (
            <tr key={g.id}>
              {g.headers.map((h) => (
                <th key={h.id} scope="col">
                  {flexRender(h.column.columnDef.header, h.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr
              key={row.id}
              tabIndex={onRow ? 0 : undefined}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && e.target === e.currentTarget) onRow?.(row.original);
              }}
              onClick={(e) => {
                if ((e.target as HTMLElement).closest('button,input,select,a')) return;
                onRow?.(row.original);
              }}
              className={onRow ? 'cursor-pointer' : ''}
            >
              {row.getVisibleCells().map((cell) => (
                <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
export function Pager({
  page,
  canNext,
  onNext,
  onPrevious,
}: {
  page: number;
  canNext: boolean;
  onNext: () => void;
  onPrevious: () => void;
}) {
  return (
    <div className="flex items-center justify-between border-t border-slate-100 px-4 py-3">
      <span className="text-sm text-slate-500">
        {t('page')} {localDigits(String(page + 1), arabicDigits())}
      </span>
      <div className="flex gap-2">
        <Button onClick={onPrevious} disabled={page === 0}>
          {t('previous')}
        </Button>
        <Button onClick={onNext} disabled={!canNext}>
          {t('next')}
        </Button>
      </div>
    </div>
  );
}
