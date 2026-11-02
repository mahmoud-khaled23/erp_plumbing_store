import { lazy, Suspense, useState } from 'react';
import { api, s, type CalculationPreview } from '../../api/client';
import type { ReorderFilter } from '../../api/contracts';
import { allowed, businessDate, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { decimal, displayDate, parseDate } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, Check, ErrorState, Field, Input, Loading, Panel } from '../../components/ui';
import { MasterSelect } from '../../components/Pickers';
import { DataTable, Pager } from '../../components/Table';
const ProductPlanningDialog = lazy(() => import('./ProductPlanningDialog'));

export default function ReorderPage({
  onBack,
  onDocument,
}: {
  onBack: () => void;
  onDocument: (id: string) => void;
}) {
  const [warehouse, setWarehouse] = useState('1'),
    [through, setThrough] = useState(displayDate(businessDate())),
    [days, setDays] = useState('30'),
    [coverage, setCoverage] = useState('14'),
    [reason, setReason] = useState(''),
    [selected, setSelected] = useState<string[]>([]),
    [cursors, setCursors] = useState(['0']),
    [preview, setPreview] = useState<CalculationPreview>(),
    [planning, setPlanning] = useState<string>(),
    [created, setCreated] = useState<string[]>([]);
  const [filter, setFilter] = useState<ReorderFilter>({
    warehouse_id: '1',
    through: businessDate(),
    lookback_days: 30,
    coverage_days: 14,
  });
  const task = useTask(),
    query = useQuery({ type: 'reorder_suggestions', filter, after_id: cursors.at(-1)!, limit: 50 });
  const changedFilters =
    warehouse !== filter.warehouse_id ||
    through !== displayDate(filter.through) ||
    days !== String(filter.lookback_days) ||
    coverage !== String(filter.coverage_days);
  const columns = [
    'code',
    'name_ar',
    'supplier_name',
    'quantity_micros',
    'on_order_micros',
    'net_sales_micros',
    'trigger_micros',
    'target_micros',
    'suggested_micros',
    'suggested_unit_cost_minor',
  ];
  return (
    <Panel title="suggestedPurchases">
      <p className="mb-4 text-sm text-slate-600">{t('reorderHint')}</p>
      <Button className="mb-4" onClick={onBack}>
        {t('back')}
      </Button>
      <div className="form-grid">
        <MasterSelect
          table="warehouses"
          label="warehouse"
          value={warehouse}
          onChange={setWarehouse}
        />
        <Field label="to">
          <Input dir="ltr" value={through} onChange={(e) => setThrough(e.target.value)} />
        </Field>
        <Field label="salesLookbackDays">
          <Input dir="ltr" value={days} onChange={(e) => setDays(e.target.value)} />
        </Field>
        <Field label="purchaseCoverageDays">
          <Input dir="ltr" value={coverage} onChange={(e) => setCoverage(e.target.value)} />
        </Field>
      </div>
      <Button
        className="my-4"
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            setFilter({
              warehouse_id: warehouse,
              through: parseDate(through),
              lookback_days: Number(decimal(days, 0)),
              coverage_days: Number(decimal(coverage, 0)),
            });
            setSelected([]);
            setCursors(['0']);
            setPreview(undefined);
            setCreated([]);
          })
        }
      >
        {t('refreshSuggestions')}
      </Button>
      {query.error ? (
        <ErrorState error={query.error} />
      ) : query.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={query.data ?? []}
          columns={[
            {
              key: 'select',
              label: 'selectForOrder',
              render: (r) => (
                <Check
                  label="selectForOrder"
                  checked={selected.includes(s(r, 'product_id'))}
                  disabled={BigInt(s(r, 'suggested_micros', '0')) <= 0n || !r.supplier_name}
                  onChange={(e) => {
                    setPreview(undefined);
                    setSelected((ids) =>
                      e.target.checked
                        ? [...ids, s(r, 'product_id')]
                        : ids.filter((id) => id !== r.product_id),
                    );
                  }}
                />
              ),
            },
            ...columns,
          ]}
          actions={(r) => (
            <Button onClick={() => setPlanning(s(r, 'product_id'))}>{t('productPlanning')}</Button>
          )}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={query.data?.length === 50}
        onPrevious={() => setCursors((c) => c.slice(0, -1))}
        onNext={() => setCursors((c) => [...c, s(query.data?.at(-1), 'product_id')])}
      />
      {allowed('purchases.write') && (
        <>
          <Button
            className="my-4"
            disabled={changedFilters || !selected.length || selected.length > 200}
            busy={task.busy}
            onClick={() =>
              task.run(async () =>
                setPreview(
                  await api.query({
                    type: 'suggested_orders_preview',
                    filter,
                    product_ids: selected,
                  }),
                ),
              )
            }
          >
            {t('previewSuggestedOrders')}
          </Button>
          {preview && (
            <>
              <DataTable
                rows={preview.rows}
                columns={[
                  'name_ar',
                  'supplier_name',
                  'suggested_micros',
                  'suggested_unit_cost_minor',
                ]}
              />
              <Field label="reason">
                <Input
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  maxLength={2000}
                />
              </Field>
              <Button
                className="mt-4"
                tone="primary"
                busy={task.busy}
                disabled={changedFilters || !reason.trim()}
                onClick={() =>
                  task.run(async () => {
                    if ((await confirmAction(t('confirmSuggestedOrders'))) === null) return;
                    const result = await api.mutate({
                      type: 'create_suggested_orders',
                      filter,
                      product_ids: selected,
                      preview_hash: preview.preview_hash,
                      reason,
                    });
                    setCreated(result.document_ids ?? []);
                    setPreview(undefined);
                    setSelected([]);
                    useStore.getState().refresh();
                    toast(t('saved'));
                  })
                }
              >
                {t('createSuggestedOrders')}
              </Button>
            </>
          )}
          {created.length > 0 && (
            <div className="my-4 flex flex-wrap gap-3">
              {created.map((id) => (
                <Button key={id} onClick={() => onDocument(id)}>
                  {t('openCreatedOrder')} <bdi>{id}</bdi>
                </Button>
              ))}
            </div>
          )}
        </>
      )}
      {planning && (
        <Suspense fallback={<Loading />}>
          <ProductPlanningDialog
            product={planning}
            onClose={() => {
              setPlanning(undefined);
              setPreview(undefined);
            }}
          />
        </Suspense>
      )}
    </Panel>
  );
}
