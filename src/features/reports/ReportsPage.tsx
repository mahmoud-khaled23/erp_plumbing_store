import { useState, type ReactNode } from 'react';
import type { Report } from '../../api/contracts';
import { s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { businessDate } from '../../app/store';
import { displayDate, parseDate } from '../../lib/numbers';
import { useQuery } from '../../lib/hooks';
import {
  Button,
  ErrorState,
  Field,
  Input,
  Loading,
  PageHeader,
  Panel,
  Select,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { PartyPicker } from '../../components/Pickers';
export const reportTypes: Report[] = [
  'trial_balance',
  'income_statement',
  'cash_flow',
  'product_profit',
  'category_profit',
  'customer_profit',
  'party_statement',
  'receivables',
  'payables',
  'aging',
  'vat',
  'low_stock',
  'valuation',
  'cheques',
  'purchase_orders',
  'open_items',
  'cash_shifts',
];
const columns: Record<Report, string[]> = {
  trial_balance: [
    'account_code',
    'account_name',
    'opening_minor',
    'debit_minor',
    'credit_minor',
    'closing_minor',
  ],
  income_statement: ['account_code', 'account_name', 'account_kind', 'profit_signed_minor'],
  cash_flow: [
    'document_date',
    'document_kind',
    'cash_account_id',
    'cash_in_minor',
    'cash_out_minor',
  ],
  product_profit: ['name_ar', 'net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
  category_profit: ['name_ar', 'net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
  customer_profit: ['name_ar', 'net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
  party_statement: [
    'document_date',
    'document_number',
    'document_kind',
    'account_name',
    'debit_minor',
    'credit_minor',
    'running_balance_minor',
    'due_date',
  ],
  receivables: ['name_ar', 'balance_minor'],
  payables: ['name_ar', 'balance_minor'],
  aging: [
    'code',
    'name_ar',
    'control_kind',
    'not_due_minor',
    'days_0_30_minor',
    'days_31_60_minor',
    'days_61_90_minor',
    'over_90_minor',
    'unapplied_credit_minor',
    'net_due_minor',
  ],
  vat: ['input_vat_minor', 'output_vat_minor', 'net_vat_minor'],
  low_stock: ['code', 'name_ar', 'warehouse_name', 'quantity_micros', 'min_stock_micros'],
  valuation: ['name_ar', 'warehouse_id', 'quantity_micros', 'value_minor'],
  cheques: [
    'cheque_number',
    'bank_name',
    'party_id',
    'due_date',
    'direction',
    'amount_minor',
    'status',
  ],
  purchase_orders: [
    'number',
    'party_id',
    'document_date',
    'ordered_micros',
    'accepted_micros',
    'progress',
  ],
  open_items: [
    'document_number',
    'document_kind',
    'document_date',
    'due_date',
    'control_kind',
    'original_signed_minor',
    'outstanding_signed_minor',
  ],
  cash_shifts: [
    'id',
    'cash_account_id',
    'opened_at',
    'closed_at',
    'opening_balance_minor',
    'expected_balance_minor',
    'counted_balance_minor',
  ],
};
export function ReportView({
  initial = 'trial_balance',
  partyId,
  partyName,
  actions,
  onRow,
}: {
  initial?: Report;
  partyId?: string;
  partyName?: string;
  actions?: (r: Row) => ReactNode;
  onRow?: (r: Row) => void;
}) {
  const [type, setType] = useState<Report>(initial);
  const day = businessDate();
  const [from, setFrom] = useState(displayDate(day.slice(0, 7) + '-01')),
    [to, setTo] = useState(displayDate(day)),
    [party, setParty] = useState<string | null>(partyId ?? null),
    [name, setName] = useState(partyName ?? ''),
    [supplier, setSupplier] = useState(false),
    [page, setPage] = useState(0);
  const [range, setRange] = useState({ from: day.slice(0, 7) + '-01', to: day }),
    [error, setError] = useState('');
  const result = useQuery({
    type: 'report',
    report: type,
    from: range.from,
    to: range.to,
    party_id: party,
    limit: 100,
    offset: page * 100,
  });
  const rows = Array.isArray(result.data) ? result.data : (result.data?.rows ?? []);
  return (
    <>
      <div className="mb-5 flex flex-wrap items-end gap-3">
        <div className="min-w-52 flex-1">
          <Field label="report">
            <Select
              value={type}
              disabled={!!actions}
              onChange={(e) => {
                setType(e.target.value as Report);
                setPage(0);
              }}
            >
              {reportTypes.map((r) => (
                <option key={r} value={r}>
                  {t(`reportNames.${r}`)}
                </option>
              ))}
            </Select>
          </Field>
        </div>
        <Field label="from">
          <Input
            className="w-36"
            dir="ltr"
            value={from}
            onChange={(e) => setFrom(e.target.value)}
          />
        </Field>
        <Field label="to">
          <Input className="w-36" dir="ltr" value={to} onChange={(e) => setTo(e.target.value)} />
        </Field>
        <Button
          tone="primary"
          onClick={() => {
            try {
              const f = parseDate(from),
                tdate = parseDate(to);
              if (f > tdate) throw new Error(t('dateOrder'));
              setRange({ from: f, to: tdate });
              setPage(0);
              setError('');
            } catch (e) {
              setError(e instanceof Error ? e.message : t('invalidDate'));
            }
          }}
        >
          {t('runReport')}
        </Button>
      </div>
      {['party_statement', 'aging', 'open_items'].includes(type) && !partyId && (
        <div className="mb-4 flex max-w-lg items-end gap-3">
          <PartyPicker
            optional
            value={party}
            name={name}
            supplier={supplier}
            onChange={(id, r) => {
              setParty(id);
              setName(s(r, 'name_ar'));
              setPage(0);
            }}
          />
          <label className="flex gap-2 py-3 text-sm">
            <input
              type="checkbox"
              checked={supplier}
              onChange={(e) => {
                setSupplier(e.target.checked);
                setParty(null);
              }}
            />
            {t('supplierAccount')}
          </label>
        </div>
      )}
      {error && (
        <p role="alert" className="field-error mb-3">
          {error}
        </p>
      )}
      {result.error ? (
        <ErrorState error={result.error} />
      ) : result.loading ? (
        <Loading />
      ) : (
        <>
          {result.data && !Array.isArray(result.data) && result.data.opening.length > 0 && (
            <div className="mb-4">
              <h3 className="mb-2 font-bold">{t('opening')}</h3>
              <DataTable
                rows={result.data.opening}
                columns={['account_id', 'balance_minor']}
                compact
              />
            </div>
          )}
          <DataTable rows={rows} columns={columns[type]} actions={actions} onRow={onRow} />
          <Pager
            page={page}
            canNext={rows.length === 100}
            onPrevious={() => setPage((p) => p - 1)}
            onNext={() => setPage((p) => p + 1)}
          />
        </>
      )}
      <p className="mt-3 text-xs text-slate-500">{t('reportPageHint')}</p>
    </>
  );
}
export default function ReportsPage() {
  return (
    <>
      <PageHeader title="reports" hint="reportsHint" />
      <Panel>
        <ReportView />
      </Panel>
    </>
  );
}
