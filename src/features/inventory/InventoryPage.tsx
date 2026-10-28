import { useState } from 'react';
import { StockRules } from './StockRules';
import { api, s, type Row, type DocumentDetail as Detail } from '../../api/client';
import { t } from '../../i18n';
import { allowed, businessDate, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { decimal, displayDate, editDecimal, formatDecimal, parseDate } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  Check,
  ErrorState,
  Field,
  Input,
  Loading,
  Modal,
  PageHeader,
  Panel,
  Tabs,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { MasterSelect, ProductSearch } from '../../components/Pickers';
import { DocumentEditor } from '../documents/DocumentEditor';
import { DocumentDetail } from '../documents/DocumentDetail';
import { openPrint } from '../printing/store';

function StockCard({
  row,
  warehouse,
  onClose,
}: {
  row: Row;
  warehouse: string;
  onClose: () => void;
}) {
  const [cursors, setCursors] = useState(['0']);
  const result = useQuery({
    type: 'stock_card',
    product_id: s(row, 'product_id'),
    warehouse_id: warehouse,
    after_id: cursors.at(-1)!,
    limit: 50,
  });
  return (
    <Modal title="stockCard" onClose={onClose} wide>
      <h3 className="mb-4 font-bold">{s(row, 'name_ar')}</h3>
      <Button
        className="mb-4"
        onClick={() =>
          openPrint({
            type: 'stock_card',
            product_id: s(row, 'product_id'),
            warehouse_id: warehouse,
          })
        }
      >
        {t('printReport')}
      </Button>
      {result.error ? (
        <ErrorState error={result.error} />
      ) : result.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={result.data ?? []}
          columns={[
            'document_date',
            'number',
            'movement_type',
            'quantity_micros',
            'value_minor',
            'running_quantity_micros',
            'running_value_minor',
            'created_by',
            'reason',
          ]}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={result.data?.length === 50}
        onPrevious={() => setCursors((c) => c.slice(0, -1))}
        onNext={() => setCursors((c) => [...c, s(result.data?.at(-1), 'id')])}
      />
    </Modal>
  );
}
function Adjustment({
  warehouse,
  onClose,
  onDone,
}: {
  warehouse: string;
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const [product, setProduct] = useState<Row>(),
    [qty, setQty] = useState('0'),
    [cost, setCost] = useState('0'),
    [reason, setReason] = useState(''),
    [date, setDate] = useState(displayDate(businessDate()));
  const task = useTask();
  return (
    <Modal title="adjustment" onClose={onClose} busy={task.busy}>
      <p className="mb-4 text-sm text-slate-500">{t('adjustmentHint')}</p>
      <ProductSearch
        onSelect={(r) => {
          setProduct(r);
          setCost(editDecimal(s(r, 'standard_cost_minor'), moneyDigits()));
        }}
      />
      {product && <p className="my-3 font-bold">{s(product, 'name_ar')}</p>}
      <div className="form-grid mt-4">
        <Field label="date">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        <Field label="quantity">
          <Input dir="ltr" value={qty} onChange={(e) => setQty(e.target.value)} />
        </Field>
        <Field label="standardCost">
          <Input dir="ltr" value={cost} onChange={(e) => setCost(e.target.value)} />
        </Field>
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} />
        </Field>
      </div>
      <Button
        className="mt-5"
        tone="primary"
        busy={task.busy}
        disabled={!product || !reason.trim()}
        onClick={() =>
          task.run(async () => {
            if ((await confirmAction(t('confirmPost'))) === null) return;
            const r = await api.mutate({
              type: 'adjustment',
              date: parseDate(date),
              warehouse_id: warehouse,
              product_id: s(product, 'id'),
              quantity: decimal(qty, 6, true),
              unit_cost: decimal(cost, moneyDigits()),
              reason,
            });
            toast(t('postedSuccess'));
            useStore.getState().refresh();
            onDone(r.id!);
          })
        }
      >
        {t('post')}
      </Button>
    </Modal>
  );
}
function StartCount({
  warehouse,
  onClose,
  onDone,
}: {
  warehouse: string;
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const [products, setProducts] = useState<Row[]>([]),
    [all, setAll] = useState(false),
    [reason, setReason] = useState(''),
    [date, setDate] = useState(displayDate(businessDate()));
  const task = useTask();
  return (
    <Modal title="newStocktake" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-500">{t('snapshotHint')}</p>
      <Check label="allItems" checked={all} onChange={(e) => setAll(e.target.checked)} />
      {!all && (
        <>
          <ProductSearch
            onSelect={(r) => {
              setProducts((p) => (p.some((x) => x.id === r.id) ? p : [...p, r]));
            }}
          />
          <DataTable
            rows={products}
            columns={['code', 'name_ar']}
            actions={(r) => (
              <Button onClick={() => setProducts((p) => p.filter((x) => x.id !== r.id))}>
                {t('remove')}
              </Button>
            )}
          />
        </>
      )}
      <div className="form-grid mt-4">
        <Field label="date">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} />
        </Field>
      </div>
      <Button
        className="mt-5"
        tone="primary"
        busy={task.busy}
        disabled={!reason.trim() || (!all && !products.length)}
        onClick={() =>
          task.run(async () => {
            let selected = products;
            if (all) {
              selected = [];
              let cursor = '0';
              for (;;) {
                const rows = await api.query({
                  type: 'products',
                  search: '',
                  after_id: cursor,
                  limit: 200,
                });
                selected.push(...rows.filter((r) => r.active === '1'));
                if (selected.length > 10000) throw new Error(t('tooLarge'));
                if (rows.length < 200) break;
                cursor = s(rows.at(-1), 'id');
              }
            }
            const r = await api.mutate({
              type: 'start_stocktake',
              date: parseDate(date),
              warehouse_id: warehouse,
              product_ids: selected.map((r) => s(r, 'id')),
              reason,
            });
            useStore.getState().refresh();
            onDone(r.id!);
          })
        }
      >
        {t('newStocktake')}
      </Button>
    </Modal>
  );
}
function CountSheet({ detail, onDone }: { detail: Detail; onDone: () => void }) {
  const [counts, setCounts] = useState<Record<string, string>>(() =>
      Object.fromEntries(
        detail.counts.map((r) => [
          s(r, 'product_id'),
          r.counted_quantity_micros == null ? '' : editDecimal(r.counted_quantity_micros, 6),
        ]),
      ),
    ),
    [reasons, setReasons] = useState<Record<string, string>>(() =>
      Object.fromEntries(detail.counts.map((r) => [s(r, 'product_id'), s(r, 'reason')])),
    ),
    [page, setPage] = useState(0);
  const [revision, setRevision] = useState(s(detail.header[0], 'revision'));
  const task = useTask();
  const h = detail.header[0];
  const save = async () => {
    const lines = detail.counts
      .filter((r) => counts[s(r, 'product_id')].trim() !== '')
      .map((r) => ({
        product_id: s(r, 'product_id'),
        quantity: decimal(counts[s(r, 'product_id')], 6),
        reason: reasons[s(r, 'product_id')],
      }));
    const r = await api.mutate({ type: 'count_stocktake', id: s(h, 'id'), revision, lines });
    setRevision(r.revision!);
    return r.revision!;
  };
  return (
    <Panel title="countSheet">
      <p className="mb-4 text-sm text-slate-500">{t('snapshotHint')}</p>
      <DataTable
        rows={detail.counts.slice(page * 100, page * 100 + 100)}
        columns={[
          'code',
          'name_ar',
          'expected_quantity_micros',
          {
            key: 'counted',
            label: 'counted',
            render: (r) => (
              <Input
                dir="ltr"
                aria-label={`${t('counted')} ${s(r, 'name_ar')}`}
                value={counts[s(r, 'product_id')]}
                onChange={(e) => setCounts((c) => ({ ...c, [s(r, 'product_id')]: e.target.value }))}
              />
            ),
          },
          {
            key: 'difference',
            label: 'difference',
            render: (r) => {
              try {
                return (
                  <bdi>
                    {formatDecimal(
                      BigInt(decimal(counts[s(r, 'product_id')], 6)) -
                        BigInt(s(r, 'expected_quantity_micros')),
                      6,
                      false,
                      true,
                    )}
                  </bdi>
                );
              } catch {
                return '—';
              }
            },
          },
          {
            key: 'reason',
            render: (r) => (
              <Input
                aria-label={t('reason')}
                value={reasons[s(r, 'product_id')]}
                onChange={(e) =>
                  setReasons((c) => ({ ...c, [s(r, 'product_id')]: e.target.value }))
                }
              />
            ),
          },
        ]}
      />
      <Pager
        page={page}
        canNext={(page + 1) * 100 < detail.counts.length}
        onPrevious={() => setPage((p) => p - 1)}
        onNext={() => setPage((p) => p + 1)}
      />
      <div className="mt-4 flex gap-2">
        <Button
          busy={task.busy}
          onClick={() =>
            task.run(async () => {
              await save();
              toast(t('saved'));
            })
          }
        >
          {t('save')}
        </Button>
        <Button
          tone="primary"
          busy={task.busy}
          onClick={() =>
            task.run(async () => {
              if ((await confirmAction(t('confirmPost'))) === null) return;
              const next = await save();
              await api.mutate({
                type: 'document_action',
                id: s(h, 'id'),
                revision: next,
                action: 'post',
                reason: s(h, 'reason'),
                acknowledge_credit: false,
              });
              toast(t('postedSuccess'));
              useStore.getState().refresh();
              onDone();
            })
          }
        >
          {t('post')}
        </Button>
      </div>
    </Panel>
  );
}
export default function InventoryPage() {
  const [warehouse, setWarehouse] = useState('1'),
    [tab, setTab] = useState<'inventory' | 'stocktake' | 'transfer' | 'opening_stock'>('inventory'),
    [cursors, setCursors] = useState(['0']),
    [dialog, setDialog] = useState<'adjust' | 'count' | null>(null),
    [card, setCard] = useState<Row | null>(null),
    [rules, setRules] = useState<Row | null>(null),
    [selected, setSelected] = useState<string | null>(null),
    [editing, setEditing] = useState(false);
  const inventory = useQuery(
    tab === 'inventory' && !selected
      ? { type: 'inventory', warehouse_id: warehouse, after_id: cursors.at(-1)!, limit: 50 }
      : null,
  );
  const list = useQuery(
    tab !== 'inventory' && !selected && !editing
      ? { type: 'documents', kind: tab, status: null, after_id: cursors.at(-1)!, limit: 50 }
      : null,
  );
  const detail = useQuery(selected ? { type: 'document', id: selected } : null);
  const open = (id: string) => {
    setDialog(null);
    setEditing(false);
    setSelected(id);
  };
  return (
    <>
      <PageHeader title="inventory" hint="inventoryHint">
        {allowed('inventory.adjust') && (
          <>
            <Button onClick={() => setDialog('adjust')}>{t('adjustment')}</Button>
            <Button tone="primary" onClick={() => setDialog('count')}>
              {t('newStocktake')}
            </Button>
          </>
        )}
        {((tab === 'transfer' && allowed('inventory.transfer')) ||
          (tab === 'opening_stock' && allowed('inventory.adjust'))) && (
          <Button
            tone="primary"
            onClick={() => {
              setEditing(true);
              setSelected(null);
            }}
          >
            {t('new')}
          </Button>
        )}
      </PageHeader>
      <Tabs
        value={tab}
        onChange={(v) => {
          setTab(v);
          setSelected(null);
          setEditing(false);
          setCursors(['0']);
        }}
        items={['inventory', 'stocktake', 'transfer', 'opening_stock'].map((value) => ({
          value: value as typeof tab,
          label: value === 'opening_stock' ? 'openingStock' : value,
        }))}
      />
      {editing && (tab === 'transfer' || tab === 'opening_stock') ? (
        <DocumentEditor
          kind={tab}
          initial={detail.data}
          onComplete={open}
          onCancel={() => {
            setEditing(false);
            useStore.getState().setDirty(false);
          }}
        />
      ) : selected ? (
        detail.error ? (
          <ErrorState error={detail.error} />
        ) : !detail.data ? (
          <Loading />
        ) : (
          <>
            <DocumentDetail
              detail={detail.data}
              onChanged={open}
              onBack={() => setSelected(null)}
              onEdit={
                s(detail.data.header[0], 'kind') === 'stocktake'
                  ? undefined
                  : () => setEditing(true)
              }
            />
            {s(detail.data.header[0], 'kind') === 'stocktake' &&
            s(detail.data.header[0], 'status') === 'draft' &&
            allowed('inventory.adjust') ? (
              <CountSheet
                key={selected}
                detail={detail.data}
                onDone={() => useStore.getState().refresh()}
              />
            ) : (
              detail.data.counts.length > 0 && (
                <Panel title="countSheet">
                  <DataTable
                    rows={detail.data.counts}
                    columns={[
                      'code',
                      'name_ar',
                      'expected_quantity_micros',
                      'counted_quantity_micros',
                      'reason',
                    ]}
                  />
                </Panel>
              )
            )}
          </>
        )
      ) : (
        <Panel>
          {tab === 'inventory' && (
            <div className="mb-4 max-w-64">
              <MasterSelect
                table="warehouses"
                label="warehouse"
                value={warehouse}
                onChange={(id) => {
                  setWarehouse(id);
                  setCursors(['0']);
                }}
              />
            </div>
          )}
          {inventory.error || list.error ? (
            <ErrorState error={inventory.error || list.error} />
          ) : inventory.loading || list.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={tab === 'inventory' ? (inventory.data ?? []) : (list.data ?? [])}
              columns={
                tab === 'inventory'
                  ? ['code', 'name_ar', 'quantity_micros', 'value_minor']
                  : ['number', 'document_date', 'status', 'reason']
              }
              onRow={tab !== 'inventory' ? (r) => open(s(r, 'id')) : undefined}
              actions={
                tab === 'inventory'
                  ? (r) => (
                      <>
                        <Button onClick={() => setCard(r)}>{t('stockCard')}</Button>
                        {allowed('inventory.adjust') && (
                          <Button onClick={() => setRules(r)}>{t('warehouseProduct')}</Button>
                        )}
                      </>
                    )
                  : undefined
              }
            />
          )}
          <Pager
            page={cursors.length - 1}
            canNext={(tab === 'inventory' ? inventory.data : list.data)?.length === 50}
            onPrevious={() => setCursors((v) => v.slice(0, -1))}
            onNext={() =>
              setCursors((v) => [
                ...v,
                s(
                  (tab === 'inventory' ? inventory.data : list.data)?.at(-1),
                  tab === 'inventory' ? 'product_id' : 'id',
                ),
              ])
            }
          />
        </Panel>
      )}
      {dialog === 'adjust' && (
        <Adjustment warehouse={warehouse} onClose={() => setDialog(null)} onDone={open} />
      )}{' '}
      {dialog === 'count' && (
        <StartCount warehouse={warehouse} onClose={() => setDialog(null)} onDone={open} />
      )}{' '}
      {rules && <StockRules product={rules} warehouse={warehouse} onClose={() => setRules(null)} />}{' '}
      {card && <StockCard row={card} warehouse={warehouse} onClose={() => setCard(null)} />}
    </>
  );
}
