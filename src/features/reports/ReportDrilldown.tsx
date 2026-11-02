import { useState } from 'react';
import type { ReportFilter } from '../../api/contracts';
import { s } from '../../api/client';
import { useQuery } from '../../lib/hooks';
import { t } from '../../i18n';
import { Modal, Button, ErrorState, Loading, Panel } from '../../components/ui';
import { DataTable, Pager, CellValue } from '../../components/Table';
import { openPrint } from '../printing/store';

export default function ReportDrilldown({
  filter,
  rowKey,
  onClose,
}: {
  filter: ReportFilter;
  rowKey: string;
  onClose: () => void;
}) {
  const [page, setPage] = useState(0),
    [document, setDocument] = useState<string | null>(null);
  const details = useQuery(
    document
      ? null
      : { type: 'report_drilldown', filter, key: rowKey, limit: 100, offset: page * 100 },
  );
  const doc = useQuery(document ? { type: 'document', id: document } : null);
  return (
    <Modal title="reportDrilldown" onClose={onClose} wide>
      {document ? (
        <>
          <div className="mb-4 flex gap-2">
            <Button onClick={() => setDocument(null)}>{t('back')}</Button>
            <Button
              disabled={!doc.data}
              onClick={() => openPrint({ type: 'document', id: document })}
            >
              {t('printDocument')}
            </Button>
          </div>
          {doc.error ? (
            <ErrorState error={doc.error} />
          ) : !doc.data ? (
            <Loading />
          ) : (
            <>
              <Panel title="sourceDocument">
                <DataTable
                  rows={doc.data.header}
                  columns={[
                    'number',
                    'kind',
                    'document_date',
                    'party_name_snapshot',
                    'total_minor',
                    'notes',
                    'reason',
                  ]}
                />
              </Panel>
              {!!doc.data.lines.length && (
                <Panel title="items">
                  <DataTable
                    rows={doc.data.lines}
                    columns={[
                      'product_name_snapshot',
                      'quantity_micros',
                      'unit_price_minor',
                      'line_discount_minor',
                      'invoice_discount_share_minor',
                      'tax_minor',
                      'total_minor',
                    ]}
                  />
                </Panel>
              )}
              {!!doc.data.payments.length && (
                <Panel title="payments">
                  <DataTable
                    rows={doc.data.payments}
                    columns={['method', 'amount_minor', 'reference']}
                  />
                </Panel>
              )}
              <Panel title="journalLines">
                <DataTable
                  rows={doc.data.journal}
                  columns={['account_code', 'account_name', 'debit_minor', 'credit_minor', 'memo']}
                />
              </Panel>
            </>
          )}
        </>
      ) : details.error ? (
        <ErrorState error={details.error} />
      ) : !details.data ? (
        <Loading />
      ) : (
        <>
          <DataTable
            rows={details.data.rows}
            columns={details.data.columns.map((key) =>
              key === 'difference_minor'
                ? {
                    key,
                    render: (r) => (
                      <strong className={s(r, key, '0') !== '0' ? 'text-red-700' : ''}>
                        <CellValue field={key} value={r[key]} />
                      </strong>
                    ),
                  }
                : key,
            )}
            onRow={(r) => setDocument(s(r, 'document_id'))}
            actions={(r) => (
              <Button onClick={() => setDocument(s(r, 'document_id'))}>{t('viewDocument')}</Button>
            )}
          />
          <Pager
            page={page}
            canNext={details.data.rows.length === 100}
            onPrevious={() => setPage((p) => p - 1)}
            onNext={() => setPage((p) => p + 1)}
          />
        </>
      )}
    </Modal>
  );
}
