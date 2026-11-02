import { useRef } from 'react';
import { api, s, type ProductDetail, type Row } from '../api/client';
import { t } from '../i18n';
import { decimal, editDecimal } from '../lib/numbers';
import { useTask } from '../lib/hooks';
import { ProductSearch } from './Pickers';
import { Button, Input, Select } from './ui';

export interface ComponentEdit {
  key: string;
  product: ProductDetail;
  unit: string;
  quantity: string;
}
export async function loadComponents(rows: Row[]): Promise<ComponentEdit[]> {
  const products = new Map<string, ProductDetail>();
  for (const id of new Set(rows.map((r) => s(r, 'product_id'))))
    products.set(id, await api.query({ type: 'product', id }));
  return rows.map((r) => ({
    key: crypto.randomUUID(),
    product: products.get(s(r, 'product_id'))!,
    unit: s(r, 'unit_id'),
    quantity: editDecimal(s(r, 'quantity_micros'), 6),
  }));
}
export const componentPayload = (items: ComponentEdit[]) =>
  items.map((i) => ({ unit_id: i.unit, quantity: decimal(i.quantity, 6) }));
export function ComponentLines({
  items,
  onChange,
  disabled = false,
}: {
  items: ComponentEdit[];
  onChange: (v: ComponentEdit[]) => void;
  disabled?: boolean;
}) {
  const task = useTask();
  const current = useRef(items);
  current.current = items;
  return (
    <div className="space-y-4">
      {!disabled && (
        <ProductSearch
          onSelect={async (r) => {
            await task.run(async () => {
              const p = await api.query({ type: 'product', id: s(r, 'id') });
              const u =
                p.units.find((u) => u.id === r.matched_unit_id) ??
                p.units.find((u) => u.unit_id === p.product[0].base_unit_id && u.active === '1');
              if (!u) throw new Error(t('invalid'));
              current.current = [
                ...current.current,
                { key: crypto.randomUUID(), product: p, unit: s(u, 'id'), quantity: '1' },
              ];
              onChange(current.current);
            });
          }}
        />
      )}
      <div className="table-scroll">
        <table className="data-table">
          <thead>
            <tr>
              {['productName', 'unit', 'quantity', 'actions'].map((k) => (
                <th key={k}>{t(k)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {items.map((item, index) => (
              <tr key={item.key}>
                <td>{s(item.product.product[0], 'name_ar')}</td>
                <td>
                  <Select
                    disabled={disabled}
                    aria-label={t('unit')}
                    value={item.unit}
                    onChange={(e) =>
                      onChange(
                        items.map((v, i) => (i === index ? { ...v, unit: e.target.value } : v)),
                      )
                    }
                  >
                    {item.product.units
                      .filter((u) => u.active === '1' || u.id === item.unit)
                      .map((u) => (
                        <option key={s(u, 'id')} value={s(u, 'id')}>
                          {s(u, 'name_ar')}
                        </option>
                      ))}
                  </Select>
                </td>
                <td>
                  <Input
                    disabled={disabled}
                    dir="ltr"
                    aria-label={`${t('componentQuantity')} ${index + 1}`}
                    value={item.quantity}
                    onChange={(e) =>
                      onChange(
                        items.map((v, i) => (i === index ? { ...v, quantity: e.target.value } : v)),
                      )
                    }
                  />
                </td>
                <td>
                  <Button
                    disabled={disabled}
                    aria-label={`${t('remove')} ${index + 1}`}
                    onClick={() => onChange(items.filter((_, i) => i !== index))}
                  >
                    {t('remove')}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
