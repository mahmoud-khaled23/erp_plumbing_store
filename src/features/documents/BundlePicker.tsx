import { useState } from 'react';
import { api, s, type ExpandedBundle, type Row } from '../../api/client';
import { t } from '../../i18n';
import { money } from '../../app/store';
import { decimal } from '../../lib/numbers';
import { useDebounced, useQuery, useTask } from '../../lib/hooks';
import { Button, ErrorState, Field, Input, Loading, Modal } from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';

export default function BundlePicker({
  wholesale,
  onAdd,
  onClose,
}: {
  wholesale: boolean;
  onAdd: (rows: Row[]) => Promise<void>;
  onClose: () => void;
}) {
  const [search, setSearch] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [chosen, setChosen] = useState<Row>(),
    [qty, setQty] = useState('1'),
    [preview, setPreview] = useState<ExpandedBundle>();
  const task = useTask(),
    query = useQuery({
      type: 'bundles',
      search: useDebounced(search),
      after_id: cursors.at(-1)!,
      limit: 50,
    });
  return (
    <Modal title="addBundle" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-600">{t('expandBundleHint')}</p>
      <Input
        aria-label={t('searchBundles')}
        placeholder={t('searchBundles')}
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          setCursors(['0']);
        }}
      />
      {query.error ? (
        <ErrorState error={query.error} />
      ) : query.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={(query.data ?? []).filter((r) => r.active === '1')}
          columns={['code', 'name_ar']}
          actions={(r) => (
            <Button
              onClick={() => {
                setChosen(r);
                setPreview(undefined);
              }}
            >
              {t('select')}
            </Button>
          )}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={query.data?.length === 50}
        onPrevious={() => setCursors((v) => v.slice(0, -1))}
        onNext={() => setCursors((v) => [...v, s(query.data?.at(-1), 'id')])}
      />
      {chosen && (
        <>
          <h3 className="my-3 font-bold">{s(chosen, 'name_ar')}</h3>
          <Field label="bundleQuantity">
            <Input
              dir="ltr"
              value={qty}
              onChange={(e) => {
                setQty(e.target.value);
                setPreview(undefined);
              }}
            />
          </Field>
          <Button
            className="my-3"
            busy={task.busy}
            onClick={() =>
              task.run(async () =>
                setPreview(
                  await api.query({
                    type: 'expand_bundle',
                    id: s(chosen, 'id'),
                    quantity: decimal(qty, 6),
                    wholesale,
                  }),
                ),
              )
            }
          >
            {t('previewBundle')}
          </Button>
        </>
      )}
      {preview && (
        <>
          <DataTable
            rows={preview.rows}
            columns={[
              'name_ar',
              'unit_name',
              'quantity_micros',
              'unit_price_minor',
              'subtotal_minor',
            ]}
          />
          <p className="my-3 font-bold">
            {t('subtotal')}: <bdi>{money(preview.total_minor)}</bdi>
          </p>
          <Button
            tone="primary"
            busy={task.busy}
            onClick={() =>
              task.run(async () => {
                await onAdd(preview.rows);
                onClose();
              })
            }
          >
            {t('addBundleComponents')}
          </Button>
        </>
      )}
    </Modal>
  );
}
