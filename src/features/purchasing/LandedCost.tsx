import { useState } from 'react';
import { api, s, type DocumentDetail } from '../../api/client';
import { t } from '../../i18n';
import { businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { allocate, decimal, displayDate, parseDate } from '../../lib/numbers';
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
    [basis, setBasis] = useState('value');
  const task = useTask();
  let shares: bigint[] = [],
    error = '';
  try {
    shares = allocate(
      BigInt(decimal(amount, moneyDigits())),
      (source?.lines ?? []).map((l) =>
        BigInt(s(l, basis === 'value' ? 'subtotal_minor' : 'base_quantity_micros')),
      ),
    );
  } catch (e) {
    error = e instanceof Error ? e.message : t('invalid');
  }
  return (
    <Modal title="landedCost" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-500">{t('landedHint')}</p>
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
          <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
        </Field>
        <Field label="allocateBy">
          <Select value={basis} onChange={(e) => setBasis(e.target.value)}>
            <option value="value">{t('byValue')}</option>
            <option value="quantity">{t('byQuantity')}</option>
          </Select>
        </Field>
      </div>
      {source && (
        <DataTable
          rows={source.lines}
          columns={[
            'product_name_snapshot',
            'quantity_micros',
            'subtotal_minor',
            {
              key: 'allocated',
              label: 'allocated',
              render: (l) => <bdi>{money(shares[source.lines.indexOf(l)])}</bdi>,
            },
          ]}
        />
      )}{' '}
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <Button
        className="mt-5"
        tone="primary"
        disabled={!source || !party || !description.trim() || !!error}
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            if ((await confirmAction(t('confirmPost'))) === null) return;
            const r = await api.mutate({
              type: 'landed_cost',
              date: parseDate(date),
              supplier_id: party!,
              description,
              shares: source!.lines
                .map((l, i) => ({ receipt_line_id: s(l, 'id'), amount: shares[i].toString() }))
                .filter((v) => BigInt(v.amount) > 0n),
            });
            toast(t('postedSuccess'));
            useStore.getState().refresh();
            onDone(r.id!);
          })
        }
      >
        {t('post')}
      </Button>
      {picking && (
        <SourcePicker kind="goods_receipt" onClose={() => setPicking(false)} onSelect={setSource} />
      )}
    </Modal>
  );
}
