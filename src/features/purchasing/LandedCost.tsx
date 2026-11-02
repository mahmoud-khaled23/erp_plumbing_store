import { useState } from 'react';
import { api, s, type CalculationPreview, type DocumentDetail } from '../../api/client';
import type { LandedBasis } from '../../api/contracts';
import { t } from '../../i18n';
import { businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { decimal, displayDate, parseDate } from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import { Button, Field, Input, Modal, Select } from '../../components/ui';
import { DataTable } from '../../components/Table';
import { PartyPicker } from '../../components/Pickers';
import { SourcePicker } from '../documents/SourcePicker';

export function LandedCost({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const [source, setSource] = useState<DocumentDetail>(),
    [picking, setPicking] = useState(false),
    [party, setParty] = useState<string | null>(null),
    [partyName, setPartyName] = useState(''),
    [date, setDate] = useState(displayDate(businessDate())),
    [description, setDescription] = useState(''),
    [amount, setAmount] = useState('0'),
    [basis, setBasis] = useState<LandedBasis | 'manual'>('value'),
    [manual, setManual] = useState<Record<string, string>>({}),
    [preview, setPreview] = useState<CalculationPreview>();
  const task = useTask(),
    lines = (source?.lines ?? []).filter((l) => BigInt(s(l, 'base_quantity_micros', '0')) > 0n);
  return (
    <Modal title="landedCost" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-500">{t('landedAllocationHint')}</p>
      <Button onClick={() => setPicking(true)}>{t('linkedReceipt')}</Button>
      {source && <p className="my-3 font-bold">{s(source.header[0], 'number')}</p>}
      <div className="form-grid my-4">
        <PartyPicker
          supplier
          optional={false}
          value={party}
          name={partyName}
          onChange={(id, r) => {
            setParty(id);
            setPartyName(s(r, 'name_ar'));
          }}
        />
        <Field label="date">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        <Field label="description">
          <Input value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <Field label="amount">
          <Input
            dir="ltr"
            value={amount}
            onChange={(e) => {
              setAmount(e.target.value);
              setPreview(undefined);
            }}
          />
        </Field>
        <Field label="allocateBy">
          <Select
            value={basis}
            onChange={(e) => {
              setBasis(e.target.value as LandedBasis | 'manual');
              setPreview(undefined);
            }}
          >
            {(['value', 'quantity', 'weight', 'manual'] as const).map((v) => (
              <option value={v} key={v}>
                {t(`landedMethods.${v}`)}
              </option>
            ))}
          </Select>
        </Field>
      </div>
      <DataTable
        rows={lines}
        columns={[
          'product_name_snapshot',
          'base_quantity_micros',
          'subtotal_minor',
          ...(basis === 'manual'
            ? [
                {
                  key: 'manual',
                  label: 'allocated',
                  render: (r: Record<string, string | null>) => (
                    <Input
                      dir="ltr"
                      aria-label={`${t('allocated')} ${s(r, 'product_name_snapshot')}`}
                      value={manual[s(r, 'id')] ?? '0'}
                      onChange={(e) => {
                        setManual((m) => ({ ...m, [s(r, 'id')]: e.target.value }));
                        setPreview(undefined);
                      }}
                    />
                  ),
                },
              ]
            : []),
        ]}
      />
      <Button
        className="my-4"
        disabled={!lines.length}
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            const total = decimal(amount, moneyDigits());
            if (basis === 'manual') {
              const rows = lines.map((l) => ({
                receipt_line_id: s(l, 'id'),
                product_name_snapshot: s(l, 'product_name_snapshot'),
                amount_minor: decimal(manual[s(l, 'id')] ?? '0', moneyDigits()),
              }));
              if (
                BigInt(total) <= 0n ||
                rows.some((r) => BigInt(r.amount_minor) < 0n) ||
                rows.reduce((a, r) => a + BigInt(r.amount_minor), 0n) !== BigInt(total)
              )
                throw new Error(t('manualAllocationMismatch'));
              setPreview({ rows, preview_hash: '' });
            } else
              setPreview(
                await api.query({
                  type: 'landed_preview',
                  basis,
                  amount: total,
                  receipt_line_ids: lines.map((l) => s(l, 'id')),
                }),
              );
          })
        }
      >
        {t('previewAllocation')}
      </Button>
      {preview && (
        <>
          <DataTable rows={preview.rows} columns={['product_name_snapshot', 'amount_minor']} />
          <p className="my-3 font-bold">
            {t('total')}{' '}
            <bdi>{money(preview.rows.reduce((a, r) => a + BigInt(s(r, 'amount_minor')), 0n))}</bdi>
          </p>
          <Button
            tone="primary"
            disabled={!party || !description.trim()}
            busy={task.busy}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmPost'))) === null) return;
                const common = { date: parseDate(date), supplier_id: party!, description };
                const result = await api.mutate(
                  basis === 'manual'
                    ? {
                        type: 'landed_cost',
                        ...common,
                        shares: preview.rows
                          .filter((r) => BigInt(s(r, 'amount_minor')) > 0n)
                          .map((r) => ({
                            receipt_line_id: s(r, 'receipt_line_id'),
                            amount: s(r, 'amount_minor'),
                          })),
                      }
                    : {
                        type: 'allocate_landed_cost',
                        ...common,
                        basis,
                        amount: decimal(amount, moneyDigits()),
                        receipt_line_ids: lines.map((l) => s(l, 'id')),
                        preview_hash: preview.preview_hash,
                      },
                );
                toast(t('postedSuccess'));
                useStore.getState().refresh();
                onDone(result.id!);
              })
            }
          >
            {t('post')}
          </Button>
        </>
      )}
      {picking && (
        <SourcePicker
          kind="goods_receipt"
          onClose={() => setPicking(false)}
          onSelect={(d) => {
            setSource(d);
            setManual({});
            setPreview(undefined);
          }}
        />
      )}
    </Modal>
  );
}
