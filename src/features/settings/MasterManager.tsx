import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import type { Master, AccountKind } from '../../api/contracts';
import { t, enumLabel } from '../../i18n';
import { allowed, businessDate, refreshBootstrap, toast, useStore } from '../../app/store';
import { decimal } from '../../lib/numbers';
import { Button, Modal, Panel } from '../../components/ui';
import { SmartForm, textValue, type FormField, type FormValues } from '../../components/Form';
import { DataTable } from '../../components/Table';
export type MasterKind = Exclude<Master['type'], 'warehouse_product'>;
export const masterTitles: Record<MasterKind, string> = {
  category: 'categories',
  unit: 'units',
  warehouse: 'warehouses',
  account: 'accounts',
  cash_account: 'cashAccounts',
  expense_category: 'expenseCategories',
  sequence: 'sequences',
};
const tables = {
  category: 'categories',
  unit: 'units',
  warehouse: 'warehouses',
  account: 'accounts',
  cash_account: 'cash_accounts',
  expense_category: 'expense_categories',
  sequence: 'sequences',
} as const;
export const documentTypes = [
  'sale',
  'sale_return',
  'quotation',
  'purchase_order',
  'purchase',
  'purchase_return',
  'goods_receipt',
  'transfer',
  'adjustment',
  'stocktake',
  'opening_stock',
  'opening_balance',
  'receipt_voucher',
  'payment_voucher',
  'cash_transfer',
  'expense',
  'manual_journal',
  'landed_cost',
  'cost_adjustment',
  'settlement',
  'cheque_clear',
  'cheque_bounce',
  'cash_close',
  'reversal',
];
export function MasterManager({ kind }: { kind: MasterKind }) {
  const b = useStore((s) => s.bootstrap)!;
  const [editing, setEditing] = useState<Row | 'new' | null>(null);
  const row = editing === 'new' ? undefined : (editing ?? undefined);
  const permitted = allowed(
    kind === 'category' || kind === 'unit' ? 'products.write' : 'settings.write',
  );
  const options = (rows: Row[]) => rows.map((r) => ({ value: s(r, 'id'), label: s(r, 'name_ar') }));
  const field = (
    name: string,
    label: string,
    type: FormField['type'] = 'text',
    required = false,
  ): FormField => ({ name, label, type, required });
  let fields: FormField[] = [field('name', 'name', 'text', true)];
  let cols = ['name_ar'];
  if (['unit', 'warehouse', 'account'].includes(kind)) {
    fields.unshift(field('code', 'code', 'text', true));
    cols.unshift('code');
  }
  if (kind === 'category' || kind === 'account')
    fields.push({
      ...field('parent_id', 'parent', 'select'),
      options: options(
        (kind === 'category' ? b.categories : b.accounts).filter((r) => r.id !== row?.id),
      ),
    });
  if (kind === 'warehouse') fields.push(field('address', 'address'));
  if (kind === 'account')
    fields.push(
      {
        ...field('kind', 'kind', 'select', true),
        options: ['asset', 'liability', 'equity', 'income', 'expense'].map((v) => ({
          value: v,
          label: enumLabel(v),
        })),
      },
      field('postable', 'postable', 'check'),
    );
  if (kind === 'cash_account' || kind === 'expense_category') {
    fields.push({
      ...field('account_id', 'account', 'select', true),
      options: options(
        b.accounts.filter(
          (r) =>
            r.is_postable === '1' && r.kind === (kind === 'cash_account' ? 'asset' : 'expense'),
        ),
      ),
    });
    cols.push('account_id');
  }
  if (kind === 'cash_account')
    fields.push(
      field('bank', 'bank', 'check'),
      field('bank_name', 'bankName'),
      field('account_number', 'accountNumber'),
    );
  if (!['category', 'unit', 'sequence'].includes(kind)) {
    fields.push(field('active', 'active', 'check'));
    cols.push('active');
  }
  if (kind === 'sequence') {
    fields = [
      {
        ...field('kind', 'kind', 'select', true),
        options: documentTypes.map((v) => ({ value: v, label: enumLabel(v) })),
      },
      field('year', 'fiscalYear', 'number', true),
      field('prefix', 'prefix', 'text', true),
      field('padding', 'padding', 'number', true),
    ];
    cols = ['document_type', 'fiscal_year', 'prefix', 'next_value', 'padding'];
  }
  const initial: FormValues = {
    name: s(row, 'name_ar'),
    code: s(row, 'code'),
    parent_id: s(row, 'parent_id'),
    address: s(row, 'address'),
    kind: s(row, 'kind', kind === 'sequence' ? s(row, 'document_type', 'sale') : 'asset'),
    postable: row ? s(row, 'is_postable') === '1' : true,
    active: row ? s(row, 'active') === '1' : true,
    account_id: s(row, 'account_id'),
    bank: s(row, 'kind') === 'bank',
    bank_name: s(row, 'bank_name'),
    account_number: s(row, 'account_number'),
    year: s(row, 'fiscal_year', businessDate().slice(0, 4)),
    prefix: s(row, 'prefix'),
    padding: s(row, 'padding', '6'),
  };
  return (
    <Panel>
      <div className="mb-4 flex items-center justify-between">
        <h2 className="font-bold">{t(masterTitles[kind])}</h2>
        {permitted && (
          <Button tone="primary" onClick={() => setEditing('new')}>
            {t('newEntry')}
          </Button>
        )}
      </div>
      <DataTable
        rows={b[tables[kind]]}
        columns={cols}
        actions={(r) =>
          permitted ? <Button onClick={() => setEditing(r)}>{t('edit')}</Button> : null
        }
      />
      {editing && (
        <Modal title={masterTitles[kind]} onClose={() => setEditing(null)}>
          <SmartForm
            fields={fields}
            initial={initial}
            onSubmit={async (v) => {
              const id = row ? s(row, 'id') : null,
                name = textValue(v, 'name'),
                code = textValue(v, 'code');
              let master: Master;
              switch (kind) {
                case 'category':
                  master = { type: kind, id, name, parent_id: textValue(v, 'parent_id') || null };
                  break;
                case 'unit':
                  master = { type: kind, id, name, code };
                  break;
                case 'warehouse':
                  master = {
                    type: kind,
                    id,
                    name,
                    code,
                    address: textValue(v, 'address'),
                    active: !!v.active,
                  };
                  break;
                case 'account':
                  master = {
                    type: kind,
                    id,
                    name,
                    code,
                    parent_id: textValue(v, 'parent_id') || null,
                    kind: textValue(v, 'kind') as AccountKind,
                    postable: !!v.postable,
                    active: !!v.active,
                  };
                  break;
                case 'cash_account':
                  master = {
                    type: kind,
                    id,
                    name,
                    bank: !!v.bank,
                    account_id: textValue(v, 'account_id'),
                    bank_name: textValue(v, 'bank_name'),
                    account_number: textValue(v, 'account_number'),
                    active: !!v.active,
                  };
                  break;
                case 'expense_category':
                  master = {
                    type: kind,
                    id,
                    name,
                    account_id: textValue(v, 'account_id'),
                    active: !!v.active,
                  };
                  break;
                case 'sequence':
                  master = {
                    type: kind,
                    kind: textValue(v, 'kind'),
                    year: Number(decimal(textValue(v, 'year'), 0)),
                    prefix: textValue(v, 'prefix'),
                    padding: Number(decimal(textValue(v, 'padding'), 0)),
                  };
                  break;
              }
              await api.mutate({ type: 'save_master', master });
              await refreshBootstrap();
              useStore.getState().refresh();
              toast(t('saved'));
              setEditing(null);
            }}
          />
        </Modal>
      )}
    </Panel>
  );
}
