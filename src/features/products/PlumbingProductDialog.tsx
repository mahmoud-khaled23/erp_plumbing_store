import { useEffect, useState } from 'react';
import { api, s, type CalculationPreview } from '../../api/client';
import type { VariantInput } from '../../api/contracts';
import { allowed, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { decimal } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, ErrorState, Field, Input, Loading, Modal, Panel } from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';

export default function PlumbingProductDialog({
  product,
  onClose,
}: {
  product: string;
  onClose: () => void;
}) {
  const detail = useQuery({ type: 'product', id: product }),
    task = useTask();
  const [days, setDays] = useState(''),
    [why, setWhy] = useState(''),
    [sizes, setSizes] = useState(''),
    [materials, setMaterials] = useState(''),
    [prefix, setPrefix] = useState(''),
    [variants, setVariants] = useState<VariantInput[]>([]),
    [preview, setPreview] = useState<CalculationPreview>(),
    [cursors, setCursors] = useState(['0']);
  const family = useQuery({
    type: 'product_variants',
    template_id: product,
    after_id: cursors.at(-1)!,
    limit: 50,
  });
  useEffect(() => {
    if (detail.data) {
      setDays(s(detail.data.product[0], 'warranty_days'));
      setPrefix(s(detail.data.product[0], 'code'));
    }
  }, [detail.data]);
  const edit = (index: number, patch: Partial<VariantInput>) => {
    setVariants((v) => v.map((r, i) => (i === index ? { ...r, ...patch } : r)));
    setPreview(undefined);
  };
  return (
    <Modal title="plumbingProduct" onClose={onClose} wide busy={task.busy}>
      {detail.error ? (
        <ErrorState error={detail.error} />
      ) : !detail.data ? (
        <Loading />
      ) : (
        <>
          <h3 className="mb-4 font-bold">{s(detail.data.product[0], 'name_ar')}</h3>
          <Panel title="productWarranty">
            <p className="mb-4 text-sm text-slate-600">{t('warrantyHint')}</p>
            <Field label="warrantyDays">
              <Input
                dir="ltr"
                value={days}
                disabled={!allowed('products.warranty')}
                onChange={(e) => setDays(e.target.value)}
              />
            </Field>
            {allowed('products.warranty') && (
              <>
                <Field label="reason">
                  <Input value={why} onChange={(e) => setWhy(e.target.value)} maxLength={2000} />
                </Field>
                <Button
                  className="mt-3"
                  busy={task.busy}
                  disabled={!why.trim()}
                  onClick={() =>
                    task.run(async () => {
                      await api.mutate({
                        type: 'set_warranty',
                        product_id: product,
                        revision: s(detail.data?.product[0], 'revision'),
                        days: days.trim() ? Number(decimal(days, 0)) : null,
                        reason: why,
                      });
                      useStore.getState().refresh();
                      toast(t('saved'));
                    })
                  }
                >
                  {t('saveWarranty')}
                </Button>
              </>
            )}
          </Panel>
          {allowed('products.variants') && (
            <Panel title="createVariants">
              <p className="mb-4 text-sm text-slate-600">{t('variantsHint')}</p>
              <div className="form-grid">
                <Field label="variantSizes">
                  <Input value={sizes} onChange={(e) => setSizes(e.target.value)} />
                </Field>
                <Field label="variantMaterials">
                  <Input value={materials} onChange={(e) => setMaterials(e.target.value)} />
                </Field>
                <Field label="variantCodePrefix">
                  <Input dir="ltr" value={prefix} onChange={(e) => setPrefix(e.target.value)} />
                </Field>
              </div>
              <Button
                className="my-4"
                onClick={() =>
                  task.run(async () => {
                    const split = (v: string) => [
                      ...new Set(
                        v
                          .split(/[,،\n]/)
                          .map((s) => s.trim())
                          .filter(Boolean),
                      ),
                    ];
                    const ss = split(sizes),
                      mm = split(materials);
                    if (!ss.length || !mm.length || ss.length * mm.length > 100)
                      throw new Error(t('variantLimit'));
                    setVariants(
                      ss
                        .flatMap((size) =>
                          mm.map((material) => ({ code: '', size, material, barcode: null })),
                        )
                        .map((v, i) => ({ ...v, code: `${prefix}-${i + 1}` })),
                    );
                    setPreview(undefined);
                  })
                }
              >
                {t('generateVariantGrid')}
              </Button>
              {!!variants.length && (
                <>
                  <div className="table-scroll">
                    <table className="data-table">
                      <thead>
                        <tr>
                          {['code', 'size', 'material', 'barcode'].map((k) => (
                            <th key={k}>{t(k)}</th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {variants.map((v, i) => (
                          <tr key={`${v.size}/${v.material}`}>
                            <td>
                              <Input
                                aria-label={`${t('code')} ${i + 1}`}
                                value={v.code}
                                onChange={(e) => edit(i, { code: e.target.value })}
                              />
                            </td>
                            <td>{v.size}</td>
                            <td>{v.material}</td>
                            <td>
                              <Input
                                dir="ltr"
                                aria-label={`${t('barcode')} ${i + 1}`}
                                value={v.barcode ?? ''}
                                onChange={(e) => edit(i, { barcode: e.target.value || null })}
                              />
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <Button
                    className="my-3"
                    busy={task.busy}
                    onClick={() =>
                      task.run(async () =>
                        setPreview(
                          await api.query({
                            type: 'variant_preview',
                            template_id: product,
                            variants,
                          }),
                        ),
                      )
                    }
                  >
                    {t('previewVariants')}
                  </Button>
                </>
              )}
              {preview && (
                <>
                  <DataTable
                    rows={preview.rows}
                    columns={['code', 'name_ar', 'size', 'material', 'barcode']}
                  />
                  <Field label="reason">
                    <Input value={why} onChange={(e) => setWhy(e.target.value)} maxLength={2000} />
                  </Field>
                  <Button
                    className="mt-3"
                    tone="primary"
                    busy={task.busy}
                    disabled={!why.trim()}
                    onClick={() =>
                      task.run(async () => {
                        if ((await confirmAction(t('confirmVariants'))) === null) return;
                        await api.mutate({
                          type: 'create_variants',
                          template_id: product,
                          variants,
                          preview_hash: preview.preview_hash,
                          reason: why,
                        });
                        setPreview(undefined);
                        setVariants([]);
                        setCursors(['0']);
                        useStore.getState().refresh();
                        toast(t('saved'));
                      })
                    }
                  >
                    {t('applyVariants')}
                  </Button>
                </>
              )}
            </Panel>
          )}
          <Panel title="variantFamily">
            {family.error ? (
              <ErrorState error={family.error} />
            ) : (
              <DataTable
                rows={family.data ?? []}
                columns={['code', 'name_ar', 'size', 'material']}
              />
            )}
            <Pager
              page={cursors.length - 1}
              canNext={family.data?.length === 50}
              onPrevious={() => setCursors((v) => v.slice(0, -1))}
              onNext={() => setCursors((v) => [...v, s(family.data?.at(-1), 'product_id')])}
            />
          </Panel>
        </>
      )}
    </Modal>
  );
}
