import { useState } from 'react';
import { api, type Operation, type ProductDetail, type Row } from '../api/client';
import { t } from '../i18n';
import { businessDate, toast, useStore } from '../app/store';
import { displayDate, parseDate } from '../lib/numbers';
import { useTask } from '../lib/hooks';
import { Button, Check, Field, Input, Modal } from './ui';
import { MasterSelect } from './Pickers';
import { DataTable } from './Table';
export function ImportDialog({
  kind,
  onClose,
}: {
  kind: 'products' | 'stock' | 'balances';
  onClose: () => void;
}) {
  const [preview, setPreview] = useState<Operation>();
  const [replace, setReplace] = useState(false);
  const [day, setDay] = useState(displayDate(businessDate()));
  const [warehouse, setWarehouse] = useState('1');
  const task = useTask();
  const rows: Row[] = (preview?.preview ?? []).map((x) => {
    if (Array.isArray(x))
      return Object.fromEntries((preview?.headers ?? []).map((k, i) => [k, String(x[i] ?? '')]));
    if (typeof x === 'object' && x !== null)
      return Object.fromEntries(
        Object.entries(x)
          .filter(([, v]) => typeof v === 'string' || typeof v === 'boolean')
          .map(([k, v]) => [k, typeof v === 'boolean' ? (v ? '1' : '0') : String(v)]),
      );
    return {};
  });
  return (
    <Modal
      title={kind === 'products' ? 'importProducts' : 'openingImports'}
      onClose={onClose}
      wide
      busy={task.busy}
    >
      <p className="mb-4 text-sm text-slate-500">{t('importHint')}</p>
      <Button
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            const result = await api.file(
              kind === 'products'
                ? { type: 'import_products' }
                : { type: 'import_opening', balances: kind === 'balances' },
            );
            if (!result.cancelled) setPreview(result);
          })
        }
      >
        {t('preview')}
      </Button>
      {preview && (
        <>
          <p className="my-4 text-sm">
            {t('rows')}: <bdi>{preview.count}</bdi> · {t('previewLimit')}
          </p>
          <DataTable
            rows={rows}
            columns={
              kind === 'products'
                ? ['code', 'name', 'standard_cost', 'min_stock']
                : (preview.headers ?? [])
            }
          />
          {kind === 'products' ? (
            <Check
              label="replaceProducts"
              checked={replace}
              onChange={(e) => setReplace(e.target.checked)}
            />
          ) : (
            <div className="form-grid mt-4">
              <Field label="date">
                <Input dir="ltr" value={day} onChange={(e) => setDay(e.target.value)} />
              </Field>
              {kind === 'stock' && (
                <MasterSelect
                  table="warehouses"
                  label="warehouse"
                  value={warehouse}
                  onChange={setWarehouse}
                />
              )}
            </div>
          )}
          <div className="mt-5 flex justify-end">
            <Button
              tone="primary"
              busy={task.busy}
              onClick={() =>
                task.run(async () => {
                  if (!preview.token) return;
                  await (kind === 'products'
                    ? api.importProducts(preview.token, replace)
                    : api.importOpening(
                        preview.token,
                        kind === 'balances',
                        parseDate(day),
                        kind === 'stock' ? warehouse : null,
                      ));
                  toast(t('imported'));
                  useStore.getState().refresh();
                  onClose();
                })
              }
            >
              {t('applyImport')}
            </Button>
          </div>
        </>
      )}
    </Modal>
  );
}
