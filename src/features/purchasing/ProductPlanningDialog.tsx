import { useEffect, useState } from 'react';
import { api, s } from '../../api/client';
import { t } from '../../i18n';
import { allowed, toast, useStore } from '../../app/store';
import { decimal, editDecimal } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, ErrorState, Field, Input, Loading, Modal, Panel } from '../../components/ui';
import { PartyPicker } from '../../components/Pickers';
import { DataTable, Pager } from '../../components/Table';
import { DocumentDetail } from '../documents/DocumentDetail';

export default function ProductPlanningDialog({
  product,
  onClose,
}: {
  product: string;
  onClose: () => void;
}) {
  const query = useQuery({ type: 'product_planning', product_id: product }),
    task = useTask();
  const [supplier, setSupplier] = useState<string | null>(null),
    [supplierName, setSupplierName] = useState(''),
    [reorder, setReorder] = useState('0'),
    [max, setMax] = useState('0'),
    [lead, setLead] = useState('0'),
    [weight, setWeight] = useState('0'),
    [reason, setReason] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [document, setDocument] = useState<string>();
  const prices = useQuery(
      allowed('purchases.read')
        ? { type: 'supplier_prices', product_id: product, after_id: cursors.at(-1)!, limit: 50 }
        : null,
    ),
    detail = useQuery(document ? { type: 'document', id: document } : null);
  useEffect(() => {
    const r = query.data?.[0];
    if (!r) return;
    setSupplier(r.supplier_id);
    setSupplierName(s(r, 'supplier_name'));
    setReorder(editDecimal(s(r, 'reorder_point_micros'), 6));
    setMax(editDecimal(s(r, 'max_level_micros'), 6));
    setLead(s(r, 'lead_time_days'));
    setWeight(s(r, 'weight_grams'));
  }, [query.data]);
  return (
    <Modal title="productPlanning" onClose={onClose} wide busy={task.busy}>
      {document ? (
        detail.error ? (
          <ErrorState error={detail.error} />
        ) : detail.data ? (
          <DocumentDetail
            detail={detail.data}
            onBack={() => setDocument(undefined)}
            onChanged={setDocument}
          />
        ) : (
          <Loading />
        )
      ) : (
        <>
          {query.error ? (
            <ErrorState error={query.error} />
          ) : query.loading ? (
            <Loading />
          ) : (
            <>
              <h3 className="mb-4 font-bold">{s(query.data?.[0], 'name_ar')}</h3>
              <DataTable rows={query.data ?? []} columns={['last_purchase_cost_minor']} />
              <p className="mb-4 text-sm text-slate-600">{t('productPlanningHint')}</p>
              {allowed('purchases.plan') ? (
                <>
                  <div className="form-grid">
                    <PartyPicker
                      supplier
                      value={supplier}
                      name={supplierName}
                      onChange={(id, r) => {
                        setSupplier(id);
                        setSupplierName(s(r, 'name_ar'));
                      }}
                    />
                    <Field label="reorderPoint">
                      <Input
                        dir="ltr"
                        value={reorder}
                        onChange={(e) => setReorder(e.target.value)}
                      />
                    </Field>
                    <Field label="maximumStock">
                      <Input dir="ltr" value={max} onChange={(e) => setMax(e.target.value)} />
                    </Field>
                    <Field label="supplierLeadDays">
                      <Input dir="ltr" value={lead} onChange={(e) => setLead(e.target.value)} />
                    </Field>
                    <Field label="baseWeightGrams">
                      <Input dir="ltr" value={weight} onChange={(e) => setWeight(e.target.value)} />
                    </Field>
                    <Field label="reason">
                      <Input
                        value={reason}
                        onChange={(e) => setReason(e.target.value)}
                        maxLength={2000}
                      />
                    </Field>
                  </div>
                  <Button
                    className="my-4"
                    tone="primary"
                    busy={task.busy}
                    disabled={!query.data?.length || !reason.trim()}
                    onClick={() =>
                      task.run(async () => {
                        await api.mutate({
                          type: 'save_product_planning',
                          planning: {
                            product_id: product,
                            revision: s(query.data?.[0], 'revision', '0'),
                            supplier_id: supplier,
                            reorder_point: decimal(reorder, 6),
                            max_level: decimal(max, 6),
                            lead_time_days: Number(decimal(lead, 0)),
                            weight_grams: decimal(weight, 0),
                          },
                          reason,
                        });
                        useStore.getState().refresh();
                        toast(t('saved'));
                      })
                    }
                  >
                    {t('save')}
                  </Button>
                </>
              ) : (
                <DataTable
                  rows={query.data ?? []}
                  columns={[
                    'supplier_name',
                    'reorder_point_micros',
                    'max_level_micros',
                    'lead_time_days',
                    'weight_grams',
                  ]}
                />
              )}
            </>
          )}
          {allowed('purchases.read') && (
            <Panel title="supplierPriceHistory">
              <p className="mb-3 text-sm text-slate-500">{t('supplierPriceHistoryHint')}</p>
              {prices.error ? (
                <ErrorState error={prices.error} />
              ) : prices.loading ? (
                <Loading />
              ) : (
                <DataTable
                  rows={prices.data ?? []}
                  columns={[
                    'document_number',
                    'document_date',
                    'supplier_name',
                    'unit_name_snapshot',
                    'purchase_unit_cost_minor',
                    'base_unit_cost_minor',
                    'status',
                  ]}
                  actions={(r) => (
                    <Button onClick={() => setDocument(s(r, 'document_id'))}>
                      {t('viewDocument')}
                    </Button>
                  )}
                />
              )}
              <Pager
                page={cursors.length - 1}
                canNext={prices.data?.length === 50}
                onPrevious={() => setCursors((c) => c.slice(0, -1))}
                onNext={() => setCursors((c) => [...c, s(prices.data?.at(-1), 'purchase_line_id')])}
              />
            </Panel>
          )}
        </>
      )}
    </Modal>
  );
}
