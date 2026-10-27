import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import { decimal, editDecimal } from '../../lib/numbers';
import { useQuery } from '../../lib/hooks';
import { toast, useStore } from '../../app/store';
import { t } from '../../i18n';
import { ErrorState, Loading, Modal } from '../../components/ui';
import { SmartForm, textValue } from '../../components/Form';
export function StockRules({
  product,
  warehouse,
  onClose,
}: {
  product: Row;
  warehouse: string;
  onClose: () => void;
}) {
  const detail = useQuery({ type: 'product', id: s(product, 'product_id') });
  const rule = detail.data?.warehouse_settings.find((r) => r.warehouse_id === warehouse);
  return (
    <Modal title="warehouseProduct" onClose={onClose}>
      {detail.error ? (
        <ErrorState error={detail.error} />
      ) : !detail.data ? (
        <Loading />
      ) : (
        <>
          <p className="mb-4 font-bold">{s(product, 'name_ar')}</p>
          <SmartForm
            fields={[
              { name: 'min', label: 'minStock', type: 'number', digits: 6, required: true },
              { name: 'bin', label: 'bin' },
            ]}
            initial={{
              min: editDecimal(
                s(rule, 'min_stock_micros', s(detail.data.product[0], 'min_stock_micros', '0')),
                6,
              ),
              bin: s(rule, 'bin_location'),
            }}
            onSubmit={async (v) => {
              await api.mutate({
                type: 'save_master',
                master: {
                  type: 'warehouse_product',
                  warehouse_id: warehouse,
                  product_id: s(product, 'product_id'),
                  min_stock: decimal(textValue(v, 'min'), 6),
                  bin: textValue(v, 'bin'),
                },
              });
              toast(t('saved'));
              useStore.getState().refresh();
              onClose();
            }}
          />
        </>
      )}
    </Modal>
  );
}
