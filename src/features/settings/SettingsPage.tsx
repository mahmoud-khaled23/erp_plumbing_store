import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { allowed, businessDate, refreshBootstrap, toast, useStore } from '../../app/store';
import { decimal, displayDate, editDecimal, parseDate } from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import { Button, Modal, PageHeader, Panel, Select, Tabs } from '../../components/ui';
import { SmartForm, textValue, type FormField } from '../../components/Form';
import { DataTable } from '../../components/Table';
import { ImportDialog } from '../../components/ImportDialog';
import { ProductImage } from '../../components/ProductImage';
import { MasterManager, masterTitles, type MasterKind } from './MasterManager';
import { OpeningImports } from './OpeningImports';
import { settingsInput } from './settingsModel';
const settingsFields: FormField[] = [
  { name: 'store_name', label: 'storeName', required: true },
  { name: 'tax_number', label: 'taxNumber' },
  { name: 'address', label: 'address' },
  { name: 'phones', label: 'phones' },
  { name: 'currency_code', label: 'currency', required: true },
  { name: 'currency_symbol', label: 'currencySymbol', required: true },
  {
    name: 'currency_digits',
    label: 'currencyDigits',
    type: 'select',
    required: true,
    options: ['0', '1', '2', '3'].map((v) => ({ value: v, label: v })),
  },
  { name: 'arabic_digits', label: 'arabicDigits', type: 'check' },
  {
    name: 'timezone',
    label: 'timezone',
    type: 'select',
    required: true,
    options: [
      'Africa/Cairo',
      'Asia/Riyadh',
      'Asia/Dubai',
      'Asia/Amman',
      'Asia/Baghdad',
      'Africa/Khartoum',
      'Africa/Tripoli',
      'Africa/Tunis',
      'Africa/Algiers',
      'Africa/Casablanca',
      'UTC',
    ].map((v) => ({ value: v, label: t(`timezones.${v}`) })),
  },
  { name: 'vat_enabled', label: 'vatEnabled', type: 'check' },
  { name: 'vat', label: 'vatPercent', type: 'number', digits: 2, required: true },
  { name: 'negative_stock', label: 'negativeStock', type: 'check' },
  { name: 'block_credit', label: 'blockCredit', type: 'check' },
  { name: 'thermal', label: 'thermal', type: 'check' },
  { name: 'receipt_footer', label: 'receiptFooter', type: 'textarea', span: true },
  { name: 'print_copies', label: 'printCopies', type: 'number', required: true },
  { name: 'auto_backup', label: 'autoBackup', type: 'check' },
];

function Periods() {
  const b = useStore((s) => s.bootstrap)!;
  const [edit, setEdit] = useState<Row | 'new' | null>(null);
  const row = edit === 'new' ? undefined : (edit ?? undefined);
  const fields: FormField[] = [
    { name: 'name', label: 'name', required: true },
    { name: 'starts', label: 'starts_on', type: 'date', required: true },
    { name: 'ends', label: 'ends_on', type: 'date', required: true },
    { name: 'locked', label: 'lockPeriod', type: 'check' },
    { name: 'reason', label: 'reason', required: true },
  ];
  return (
    <Panel title="fiscalPeriods">
      <p className="mb-4 text-sm text-slate-500">{t('periodHint')}</p>
      {allowed('periods.unlock') && (
        <Button className="mb-4" onClick={() => setEdit('new')}>
          {t('newEntry')}
        </Button>
      )}
      <DataTable
        rows={b.fiscal_periods}
        columns={['name', 'starts_on', 'ends_on', 'locked_at']}
        actions={(r) => (
          <Button
            disabled={r.locked_at ? !allowed('periods.unlock') : !allowed('periods.lock')}
            onClick={() => setEdit(r)}
          >
            {t('edit')}
          </Button>
        )}
      />
      {edit && (
        <Modal title="fiscalPeriods" onClose={() => setEdit(null)}>
          <SmartForm
            fields={fields}
            initial={{
              name: s(row, 'name'),
              starts: displayDate(s(row, 'starts_on', businessDate().slice(0, 4) + '-01-01')),
              ends: displayDate(s(row, 'ends_on', businessDate().slice(0, 4) + '-12-31')),
              locked: !!row?.locked_at,
              reason: '',
            }}
            onSubmit={async (v) => {
              await api.mutate({
                type: 'fiscal_period',
                id: row ? s(row, 'id') : null,
                name: textValue(v, 'name'),
                starts_on: parseDate(textValue(v, 'starts')),
                ends_on: parseDate(textValue(v, 'ends')),
                locked: !!v.locked,
                reason: textValue(v, 'reason'),
              });
              await refreshBootstrap();
              toast(t('saved'));
              setEdit(null);
            }}
          />
        </Modal>
      )}
    </Panel>
  );
}
export default function SettingsPage() {
  const b = useStore((s) => s.bootstrap)!;
  const row = b.settings[0];
  const settings = settingsInput(row);
  const [tab, setTab] = useState(allowed('settings.write') ? 'storeProfile' : 'fiscalPeriods'),
    [master, setMaster] = useState<MasterKind>('warehouse');
  const task = useTask();
  const tabs = [
    ...(allowed('settings.write')
      ? [
          { value: 'storeProfile', label: 'storeProfile' },
          { value: 'masterData', label: 'masterData' },
        ]
      : []),
    ...(allowed('periods.lock') || allowed('periods.unlock')
      ? [{ value: 'fiscalPeriods', label: 'fiscalPeriods' }]
      : []),
    ...(allowed('backup.create') || allowed('backup.restore')
      ? [{ value: 'backups', label: 'backups' }]
      : []),
    { value: 'openingImports', label: 'openingImports' },
  ];
  return (
    <>
      <PageHeader title="settings" hint="settingsHint" />
      <Tabs value={tab} onChange={setTab} items={tabs} />
      {tab === 'storeProfile' && (
        <Panel title="storeProfile">
          <div className="mb-5 flex items-center gap-4">
            <ProductImage path={s(row, 'logo_path')} />
            <Button
              busy={task.busy}
              onClick={() =>
                task.run(async () => {
                  await api.file({ type: 'image', product_id: null });
                  await refreshBootstrap();
                })
              }
            >
              {t('logo')}
            </Button>
          </div>
          <SmartForm
            key={s(row, 'revision')}
            fields={settingsFields}
            initial={{
              ...Object.fromEntries(
                Object.entries(settings).map(([k, v]) => [
                  k,
                  typeof v === 'number' ? String(v) : v,
                ]),
              ),
              currency_digits: String(settings.currency_digits),
              vat: editDecimal(String(settings.vat_bps), 2),
              print_copies: String(settings.print_copies),
            }}
            onSubmit={async (v) => {
              await api.mutate({
                type: 'save_settings',
                settings: {
                  ...settings,
                  store_name: textValue(v, 'store_name'),
                  tax_number: textValue(v, 'tax_number'),
                  address: textValue(v, 'address'),
                  phones: textValue(v, 'phones'),
                  currency_code: textValue(v, 'currency_code'),
                  currency_symbol: textValue(v, 'currency_symbol'),
                  currency_digits: Number(textValue(v, 'currency_digits')),
                  arabic_digits: !!v.arabic_digits,
                  timezone: textValue(v, 'timezone'),
                  vat_enabled: !!v.vat_enabled,
                  vat_bps: Number(decimal(textValue(v, 'vat'), 2)),
                  negative_stock: !!v.negative_stock,
                  block_credit: !!v.block_credit,
                  thermal: !!v.thermal,
                  receipt_footer: textValue(v, 'receipt_footer'),
                  print_copies: Number(decimal(textValue(v, 'print_copies'), 0)),
                  auto_backup: !!v.auto_backup,
                },
              });
              await refreshBootstrap();
              useStore.getState().refresh();
              toast(t('saved'));
            }}
          />
        </Panel>
      )}
      {tab === 'masterData' && (
        <>
          <div className="mb-4 max-w-xs">
            <Select
              aria-label={t('masterData')}
              value={master}
              onChange={(e) => setMaster(e.target.value as MasterKind)}
            >
              {(Object.keys(masterTitles) as MasterKind[]).map((k) => (
                <option key={k} value={k}>
                  {t(masterTitles[k])}
                </option>
              ))}
            </Select>
          </div>
          <MasterManager key={master} kind={master} />
        </>
      )}
      {tab === 'fiscalPeriods' && <Periods />}
      {tab === 'openingImports' && (
        <Panel title="openingImports">
          <OpeningImports />
        </Panel>
      )}
      {tab === 'backups' && (
        <Panel title="backups">
          <p className="mb-5 text-sm text-slate-500">{t('backupHint')}</p>
          <div className="mb-5 flex flex-wrap gap-3">
            {allowed('backup.create') && (
              <Button
                tone="primary"
                busy={task.busy}
                onClick={() =>
                  task.run(async () => {
                    const r = await api.file({ type: 'backup' });
                    if (!r.cancelled) {
                      toast(t('done'));
                      await refreshBootstrap();
                    }
                  })
                }
              >
                {t('backup')}
              </Button>
            )}
            {allowed('settings.write') && (
              <Button
                onClick={() =>
                  task.run(async () => {
                    await api.file({ type: 'backup_directory' });
                    await refreshBootstrap();
                  })
                }
              >
                {t('backupFolder')}
              </Button>
            )}
            {allowed('backup.restore') && (
              <Button
                tone="danger"
                busy={task.busy}
                onClick={() =>
                  task.run(async () => {
                    const r = await api.file({ type: 'restore' });
                    if (r.restored) {
                      useStore.setState({ bootstrap: null, dirty: false });
                      await refreshBootstrap();
                    }
                  })
                }
              >
                {t('restore')}
              </Button>
            )}
          </div>
          <DataTable
            rows={b.backup_runs}
            columns={['kind', 'status', 'completed_at', 'size_bytes', 'error_message']}
          />
        </Panel>
      )}
    </>
  );
}
