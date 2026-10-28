import { useEffect, useRef, useState } from 'react';
import { useForm, useFieldArray } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import JsBarcode from 'jsbarcode';
import { api, assetUrl, productInput, s, type ProductDetail, type Row } from '../../api/client';
import type { Product } from '../../api/contracts';
import { ProductImage } from '../../components/ProductImage';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { allowed, moneyDigits, toast, useStore } from '../../app/store';
import { decimal, editDecimal } from '../../lib/numbers';
import { useDebounced, useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  Check,
  ErrorState,
  Field,
  Icon,
  Input,
  Loading,
  Modal,
  PageHeader,
  Panel,
  Select,
  Tabs,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { ImportDialog } from '../../components/ImportDialog';
import { MasterManager } from '../settings/MasterManager';
import { openPrint } from '../printing/store';

export function Barcode({ value }: { value: string }) {
  const ref = useRef<SVGSVGElement>(null);
  const [invalid, setInvalid] = useState(false);
  useEffect(() => {
    if (!value) {
      setInvalid(false);
      return;
    }
    try {
      JsBarcode(ref.current, value, {
        format: 'CODE128',
        height: 44,
        width: 1.4,
        margin: 8,
        displayValue: true,
        fontSize: 13,
      });
      setInvalid(false);
    } catch {
      setInvalid(true);
    }
  }, [value]);
  return invalid ? (
    <p role="alert" className="field-error">
      {t('barcodeInvalid')}
    </p>
  ) : value ? (
    <svg ref={ref} role="img" aria-label={t('barcodePreview')} className="max-w-full" />
  ) : null;
}

const unitSchema = z.object({
  id: z.string().nullable(),
  unit_id: z.string().min(1, t('required')),
  numerator: z.string().min(1, t('required')),
  denominator: z.string().min(1, t('required')),
  barcode: z.string(),
  retail_price: z.string(),
  wholesale_price: z.string(),
  active: z.boolean(),
});
const formSchema = z.object({
  code: z.string().trim().min(1, t('required')),
  name: z.string().trim().min(1, t('required')),
  category_id: z.string().min(1, t('required')),
  brand: z.string(),
  material: z.string(),
  size: z.string(),
  base_unit_id: z.string().min(1, t('required')),
  standard_cost: z.string(),
  min_stock: z.string(),
  active: z.boolean(),
  units: z.array(unitSchema).min(1, t('noLines')),
});
type ProductForm = z.infer<typeof formSchema>;
function ProductEditor({ detail, onClose }: { detail?: ProductDetail; onClose: () => void }) {
  const b = useStore((s) => s.bootstrap)!;
  const digits = moneyDigits();
  const p = detail ? productInput(detail) : undefined;
  const form = useForm<ProductForm>({
    resolver: zodResolver(formSchema),
    defaultValues: p
      ? {
          ...p,
          standard_cost: editDecimal(p.standard_cost, digits),
          min_stock: editDecimal(p.min_stock, 6),
          units: p.units.map((u) => ({
            ...u,
            barcode: u.barcode ?? '',
            retail_price: editDecimal(u.retail_price, digits),
            wholesale_price: editDecimal(u.wholesale_price, digits),
          })),
        }
      : {
          code: '',
          name: '',
          category_id: s(b.categories[0], 'id'),
          brand: '',
          material: '',
          size: '',
          base_unit_id: s(b.units[0], 'id'),
          standard_cost: '0',
          min_stock: '0',
          active: true,
          units: [
            {
              id: null,
              unit_id: s(b.units[0], 'id'),
              numerator: '1',
              denominator: '1',
              barcode: '',
              retail_price: '0',
              wholesale_price: '0',
              active: true,
            },
          ],
        },
  });
  const array = useFieldArray({ control: form.control, name: 'units', keyName: 'key' });
  const task = useTask();
  const currentBarcode = form.watch('units.0.barcode');
  const close = async () => {
    if (form.formState.isDirty) {
      if ((await confirmAction(t('unsaved'))) === null) return;
    }
    onClose();
  };
  return (
    <Modal
      title={p ? 'editProduct' : 'newProduct'}
      onClose={() => void close()}
      wide
      busy={task.busy}
    >
      <form
        noValidate
        onSubmit={form.handleSubmit((v) =>
          task.run(async () => {
            const product: Product = {
              ...v,
              id: p?.id ?? null,
              revision: p?.revision ?? '0',
              standard_cost: decimal(v.standard_cost, digits),
              min_stock: decimal(v.min_stock, 6),
              units: v.units.map((u) => ({
                ...u,
                numerator: decimal(u.numerator, 0),
                denominator: decimal(u.denominator, 0),
                barcode: u.barcode.trim() || null,
                retail_price: decimal(u.retail_price, digits),
                wholesale_price: decimal(u.wholesale_price, digits),
              })),
            };
            await api.mutate({ type: 'save_product', product });
            toast(t('saved'));
            useStore.getState().refresh();
            onClose();
          }),
        )}
      >
        <div className="form-grid">
          {(['code', 'name', 'brand', 'material', 'size'] as const).map((k) => (
            <Field key={k} label={k} error={form.formState.errors[k]?.message}>
              <Input autoFocus={k === 'code'} {...form.register(k)} />
            </Field>
          ))}
          <Field label="category" error={form.formState.errors.category_id?.message}>
            <Select {...form.register('category_id')}>
              {b.categories.map((c) => (
                <option key={s(c, 'id')} value={s(c, 'id')}>
                  {s(c, 'name_ar')}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="baseUnit">
            <Select {...form.register('base_unit_id')}>
              {b.units.map((c) => (
                <option key={s(c, 'id')} value={s(c, 'id')}>
                  {s(c, 'name_ar')}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="standardCost">
            <Input dir="ltr" inputMode="decimal" {...form.register('standard_cost')} />
          </Field>
          <Field label="minStock">
            <Input dir="ltr" inputMode="decimal" {...form.register('min_stock')} />
          </Field>
          <Check label="active" {...form.register('active')} />
        </div>
        <h3 className="mb-2 mt-6 font-bold">{t('productPricing')}</h3>
        <p className="mb-4 text-sm text-slate-500">{t('unitHint')}</p>
        <div className="space-y-3">
          {array.fields.map((u, i) => (
            <div className="rounded-xl border border-slate-200 p-4" key={u.key}>
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
                <Field label="unit">
                  <Select {...form.register(`units.${i}.unit_id`)}>
                    {b.units.map((c) => (
                      <option key={s(c, 'id')} value={s(c, 'id')}>
                        {s(c, 'name_ar')}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="numerator">
                  <Input dir="ltr" inputMode="numeric" {...form.register(`units.${i}.numerator`)} />
                </Field>
                <Field label="denominator">
                  <Input
                    dir="ltr"
                    inputMode="numeric"
                    {...form.register(`units.${i}.denominator`)}
                  />
                </Field>
                <Field label="barcode">
                  <Input dir="ltr" {...form.register(`units.${i}.barcode`)} />
                </Field>
                <Field label="retailPrice">
                  <Input
                    dir="ltr"
                    inputMode="decimal"
                    {...form.register(`units.${i}.retail_price`)}
                  />
                </Field>
                <Field label="wholesalePrice">
                  <Input
                    dir="ltr"
                    inputMode="decimal"
                    {...form.register(`units.${i}.wholesale_price`)}
                  />
                </Field>
                <Check label="active" {...form.register(`units.${i}.active`)} />
                <Button
                  disabled={array.fields.length === 1}
                  tone="danger"
                  onClick={() => array.remove(i)}
                >
                  {t('remove')}
                </Button>
              </div>
            </div>
          ))}
        </div>
        <div className="mt-3 flex items-center justify-between">
          <Button
            onClick={() =>
              array.append({
                id: null,
                unit_id: s(b.units[0], 'id'),
                numerator: '1',
                denominator: '1',
                barcode: '',
                retail_price: '0',
                wholesale_price: '0',
                active: true,
              })
            }
          >
            {t('addUnit')}
          </Button>
          <Barcode value={currentBarcode} />
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button onClick={() => void close()}>{t('cancel')}</Button>
          <Button tone="primary" type="submit" busy={task.busy}>
            {t('save')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
export default function ProductsPage() {
  const [tab, setTab] = useState<'products' | 'categories' | 'units'>('products');
  const [search, setSearch] = useState('');
  const q = useDebounced(search);
  const [cursors, setCursors] = useState(['0']);
  const result = useQuery({ type: 'products', search: q, after_id: cursors.at(-1)!, limit: 50 });
  const [editor, setEditor] = useState<ProductDetail | 'new' | null>(null);
  const [inspect, setInspect] = useState<ProductDetail | null>(null);
  const [importing, setImporting] = useState(false);
  const task = useTask();
  return (
    <>
      <PageHeader title="products" hint="productHint">
        {allowed('products.import') && (
          <Button onClick={() => setImporting(true)}>{t('import')}</Button>
        )}
        <Button onClick={() => task.run(() => api.file({ type: 'export_products', excel: false }))}>
          {t('exportCsv')}
        </Button>
        <Button onClick={() => task.run(() => api.file({ type: 'export_products', excel: true }))}>
          {t('exportExcel')}
        </Button>
        {allowed('products.write') && (
          <Button tone="primary" onClick={() => setEditor('new')}>
            <Icon name="plus" />
            {t('newProduct')}
          </Button>
        )}
      </PageHeader>
      <Tabs
        value={tab}
        onChange={setTab}
        items={['products', 'categories', 'units'].map((value) => ({
          value: value as typeof tab,
          label: value,
        }))}
      />
      {tab === 'products' ? (
        <Panel>
          <div className="search-field mb-4">
            <Icon name="search" />
            <input
              data-page-search
              value={search}
              placeholder={t('searchProducts')}
              aria-label={t('searchProducts')}
              onChange={(e) => {
                setSearch(e.target.value);
                setCursors(['0']);
              }}
            />
          </div>
          {result.error ? (
            <ErrorState error={result.error} />
          ) : result.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={result.data ?? []}
              columns={[
                'code',
                'name_ar',
                'category_id',
                'brand',
                'size',
                'standard_cost_minor',
                'min_stock_micros',
                'active',
              ]}
              onRow={(r) =>
                void task.run(async () =>
                  setInspect(await api.query({ type: 'product', id: s(r, 'id') })),
                )
              }
              actions={(r) => (
                <>
                  {allowed('products.write') && (
                    <Button
                      onClick={() =>
                        task.run(async () =>
                          setEditor(await api.query({ type: 'product', id: s(r, 'id') })),
                        )
                      }
                    >
                      {t('edit')}
                    </Button>
                  )}
                </>
              )}
            />
          )}
          <Pager
            page={cursors.length - 1}
            canNext={(result.data?.length ?? 0) === 50}
            onPrevious={() => setCursors((v) => v.slice(0, -1))}
            onNext={() => setCursors((v) => [...v, s(result.data?.at(-1), 'id')])}
          />
        </Panel>
      ) : (
        <MasterManager kind={tab === 'categories' ? 'category' : 'unit'} />
      )}{' '}
      {editor && (
        <ProductEditor
          detail={editor === 'new' ? undefined : editor}
          onClose={() => setEditor(null)}
        />
      )}{' '}
      {inspect && (
        <Modal title="details" onClose={() => setInspect(null)} wide>
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h3 className="text-xl font-bold">{s(inspect.product[0], 'name_ar')}</h3>
              <bdi className="text-slate-500">{s(inspect.product[0], 'code')}</bdi>
            </div>
            <ProductImage path={s(inspect.product[0], 'image_path')} />
          </div>
          <DataTable
            rows={inspect.units}
            actions={(u) =>
              u.active === '1' && u.barcode ? (
                <Button onClick={() => openPrint({ type: 'labels', unit_ids: [s(u, 'id')] })}>
                  {t('printLabels')}
                </Button>
              ) : null
            }
            columns={[
              'name_ar',
              'barcode',
              'retail_price_minor',
              'wholesale_price_minor',
              'active',
            ]}
          />
          {inspect.units.map((u) => (
            <Barcode key={s(u, 'id')} value={s(u, 'barcode')} />
          ))}
          <Button
            disabled={!inspect.units.some((u) => u.active === '1' && u.barcode)}
            onClick={() =>
              openPrint({
                type: 'labels',
                unit_ids: inspect.units
                  .filter((u) => u.active === '1' && u.barcode)
                  .map((u) => s(u, 'id')),
              })
            }
          >
            {t('printAllUnits')}
          </Button>
          {allowed('products.write') && (
            <Button
              onClick={() =>
                task.run(async () => {
                  await api.file({ type: 'image', product_id: s(inspect.product[0], 'id') });
                  setInspect(await api.query({ type: 'product', id: s(inspect.product[0], 'id') }));
                  useStore.getState().refresh();
                })
              }
            >
              {t('selectImage')}
            </Button>
          )}
        </Modal>
      )}
      {importing && <ImportDialog kind="products" onClose={() => setImporting(false)} />}
    </>
  );
}
