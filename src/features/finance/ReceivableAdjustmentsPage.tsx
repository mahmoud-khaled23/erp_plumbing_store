import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import type { ReceivableAdjustmentKind } from '../../api/contracts';
import { allowed, businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { decimal, displayDate, editDecimal, parseDate } from '../../lib/numbers';
import { useDebounced, useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  ErrorState,
  Field,
  Input,
  Loading,
  Modal,
  Panel,
  Select,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';

export default function ReceivableAdjustmentsPage({
  onDocument,
}: {
  onDocument: (id: string) => void;
}) {
  const kinds = (['bad_debt', 'customer_credit', 'settlement_discount'] as const).filter((k) =>
    allowed(
      {
        bad_debt: 'finance.writeoff',
        customer_credit: 'finance.credit_note',
        settlement_discount: 'finance.settlement_discount',
      }[k],
    ),
  );
  const [search, setSearch] = useState(''),
    [cursors, setCursors] = useState(['0']);
  const [item, setItem] = useState<Row>(),
    [kind, setKind] = useState<ReceivableAdjustmentKind>(kinds[0]);
  const [date, setDate] = useState(displayDate(businessDate())),
    [amount, setAmount] = useState('0'),
    [reason, setReason] = useState('');
  const task = useTask(),
    query = useQuery({
      type: 'customer_open_items',
      search: useDebounced(search),
      after_id: cursors.at(-1)!,
      limit: 50,
    });
  return (
    <Panel title="customerAdjustments">
      <p className="mb-4 text-sm text-slate-600">{t('customerAdjustmentsHint')}</p>
      <Input
        className="mb-4"
        aria-label={t('search')}
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          setCursors(['0']);
        }}
      />
      {query.error ? (
        <ErrorState error={query.error} />
      ) : query.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={query.data ?? []}
          columns={['name_ar', 'document_number', 'due_date', 'outstanding_signed_minor']}
          actions={(r) => (
            <>
              <Button onClick={() => onDocument(s(r, 'document_id'))}>{t('viewDocument')}</Button>
              <Button
                onClick={() => {
                  setItem(r);
                  setAmount(editDecimal(s(r, 'outstanding_signed_minor'), moneyDigits()));
                  setReason('');
                }}
              >
                {t('customerAdjustment')}
              </Button>
            </>
          )}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={query.data?.length === 50}
        onPrevious={() => setCursors((c) => c.slice(0, -1))}
        onNext={() => setCursors((c) => [...c, s(query.data?.at(-1), 'journal_line_id')])}
      />
      {item && (
        <Modal title="customerAdjustment" onClose={() => setItem(undefined)} busy={task.busy}>
          <p className="mb-4">
            {s(item, 'name_ar')} · {s(item, 'document_number')} ·{' '}
            <bdi>{money(s(item, 'outstanding_signed_minor'))}</bdi>
          </p>
          <div className="form-grid">
            <Field label="kind">
              <Select
                value={kind}
                onChange={(e) => setKind(e.target.value as ReceivableAdjustmentKind)}
              >
                {kinds.map((k) => (
                  <option key={k} value={k}>
                    {t(`enums.${k}`)}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="date">
              <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
            </Field>
            <Field label="amount">
              <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
            </Field>
            <Field label="reason">
              <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
            </Field>
          </div>
          <p className="my-4 text-sm">{t(`adjustmentPosting.${kind}`)}</p>
          <Button
            tone="primary"
            busy={task.busy}
            disabled={!reason.trim()}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmPost'))) === null) return;
                const result = await api.mutate({
                  type: 'receivable_adjustment',
                  kind,
                  date: parseDate(date),
                  debit_line_id: s(item, 'journal_line_id'),
                  amount: decimal(amount, moneyDigits()),
                  reason,
                });
                setItem(undefined);
                useStore.getState().refresh();
                toast(t('done'));
                onDocument(result.id!);
              })
            }
          >
            {t('post')}
          </Button>
        </Modal>
      )}
    </Panel>
  );
}
