import { useState, type ReactNode } from 'react';
import { reportTypes, reportColumns } from './definitions';
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
import { openPrint } from '../printing/store';
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
        <Button
          disabled={result.loading || !!result.error}
          onClick={() =>
            openPrint({
              type: 'report',
              report: type,
              from: range.from,
              to: range.to,
              party_id: party,
            })
          }
        >
          {t('printReport')}
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
          <DataTable rows={rows} columns={reportColumns[type]} actions={actions} onRow={onRow} />
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
