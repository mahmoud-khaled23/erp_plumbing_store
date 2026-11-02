import { useState } from 'react';
import { api, s } from '../../api/client';
import { allowed, businessDate, refreshBootstrap, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { displayDate, parseDate } from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import {
  ComponentLines,
  componentPayload,
  type ComponentEdit,
} from '../../components/ComponentLines';
import { MasterSelect } from '../../components/Pickers';
import { Button, Check, Field, Input, Modal, Panel, Select } from '../../components/ui';

export default function QuarantineDialog({
  onClose,
  onDocument,
}: {
  onClose: () => void;
  onDocument: (id: string) => void;
}) {
  const task = useTask(),
    b = useStore((v) => v.bootstrap)!;
  const [direction, setDirection] = useState('quarantine'),
    [from, setFrom] = useState(''),
    [to, setTo] = useState(''),
    [day, setDay] = useState(displayDate(businessDate())),
    [reason, setReason] = useState(''),
    [items, setItems] = useState<ComponentEdit[]>([]),
    [config, setConfig] = useState(''),
    [flag, setFlag] = useState(false),
    [configReason, setConfigReason] = useState('');
  const selected = b.warehouses.find((w) => w.id === config);
  return (
    <Modal title="quarantineWorkflow" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-600">{t('quarantineHint')}</p>
      {allowed('inventory.quarantine_manage') && (
        <Panel title="quarantineWarehouseSettings">
          <MasterSelect
            table="warehouses"
            label="warehouse"
            value={config}
            onChange={(id) => {
              setConfig(id);
              setFlag(b.warehouses.find((w) => w.id === id)?.is_quarantine === '1');
            }}
          />
          <Check
            label="quarantineWarehouseTag"
            checked={flag}
            onChange={(e) => setFlag(e.target.checked)}
          />
          <Field label="reason">
            <Input
              value={configReason}
              onChange={(e) => setConfigReason(e.target.value)}
              maxLength={2000}
            />
          </Field>
          <Button
            className="my-3"
            busy={task.busy}
            disabled={!selected || !configReason.trim()}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmQuarantineWarehouse'))) === null) return;
                await api.mutate({
                  type: 'set_quarantine_warehouse',
                  warehouse_id: config,
                  revision: s(selected, 'quarantine_revision', '0'),
                  quarantine: flag,
                  reason: configReason,
                });
                await refreshBootstrap();
                useStore.getState().refresh();
                setFrom('');
                setTo('');
                toast(t('saved'));
              })
            }
          >
            {t('saveQuarantineWarehouse')}
          </Button>
        </Panel>
      )}
      <Panel title="quarantineMovement">
        <div className="form-grid">
          <Field label="quarantineDirection">
            <Select
              value={direction}
              onChange={(e) => {
                setDirection(e.target.value);
                setFrom('');
                setTo('');
              }}
            >
              <option value="quarantine">{t('quarantineGoods')}</option>
              <option value="release">{t('releaseGoods')}</option>
            </Select>
          </Field>
          <Field label="date">
            <Input dir="ltr" value={day} onChange={(e) => setDay(e.target.value)} />
          </Field>
          <MasterSelect
            table="warehouses"
            label="sourceWarehouse"
            value={from}
            onChange={setFrom}
            filter={(r) => (r.is_quarantine === '1') === (direction === 'release')}
          />
          <MasterSelect
            table="warehouses"
            label="destination"
            value={to}
            onChange={setTo}
            filter={(r) => (r.is_quarantine === '1') === (direction === 'quarantine')}
          />
        </div>
        <div className="my-4">
          <ComponentLines items={items} onChange={setItems} disabled={task.busy} />
        </div>
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
        </Field>
        <Button
          className="mt-4"
          tone="primary"
          busy={task.busy}
          disabled={!from || !to || !items.length || !reason.trim()}
          onClick={() =>
            task.run(async () => {
              if ((await confirmAction(t('confirmQuarantineTransfer'))) === null) return;
              const result = await api.mutate({
                type: 'quarantine_transfer',
                date: parseDate(day),
                from_id: from,
                to_id: to,
                lines: componentPayload(items),
                reason,
              });
              useStore.getState().refresh();
              toast(t('postedSuccess'));
              onDocument(result.id!);
            })
          }
        >
          {t('postQuarantineTransfer')}
        </Button>
      </Panel>
    </Modal>
  );
}
