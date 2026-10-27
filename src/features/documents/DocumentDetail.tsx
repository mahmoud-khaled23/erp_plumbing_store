import { useState } from 'react';
import { api, ApiError, s, type DocumentDetail as Detail, type Row } from '../../api/client';
import type { DocumentAction, DocumentKind } from '../../api/contracts';
import { t, enumLabel } from '../../i18n';
import { allowed, businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import {
  allocate,
  baseQuantity,
  decimal,
  displayDate,
  editDecimal,
  parseDate,
  rounded,
} from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import { Button, Field, Input, Modal, Panel, Status } from '../../components/ui';
import { DataTable } from '../../components/Table';
import { MasterSelect } from '../../components/Pickers';
import { Payments, paymentPayload, type PaymentEdit } from '../../components/Payments';
export const documentPermission = (kind: string) =>
  kind === 'sale' || kind === 'quotation'
    ? 'sales.write'
    : kind === 'purchase' || kind === 'purchase_order'
      ? 'purchases.write'
      : kind === 'goods_receipt'
        ? 'inventory.receive'
        : kind === 'transfer'
          ? 'inventory.transfer'
          : 'inventory.adjust';
export function returnValue(detail: Detail, quantities: Record<string, string>): bigint {
  return detail.lines.reduce((total, l) => {
    const requested = BigInt(decimal(quantities[s(l, 'id')] || '0', 6));
    if (!requested) return total;
    const available = detail.availability.find((a) => a.id === l.id);
    const remaining = BigInt(s(available, 'returnable_micros', s(l, 'quantity_micros')));
    if (requested > remaining) throw new Error(t('invalidNumber'));
    const qty = BigInt(
        baseQuantity(requested.toString(), s(l, 'factor_numerator'), s(l, 'factor_denominator')),
      ),
      base = BigInt(s(l, 'base_quantity_micros'));
    const used = detail.returned.find((r) => r.source_line_id === l.id);
    const left = (key: string) => BigInt(s(l, key, '0')) - BigInt(s(used, key, '0'));
    const last = requested === remaining;
    const remainingGross = left('subtotal_minor'),
      remainingDiscount = left('line_discount_minor'),
      remainingShare = left('invoice_discount_share_minor'),
      remainingTax = left('tax_minor');
    const proportion = rounded(BigInt(s(l, 'subtotal_minor')) * qty, base);
    const gross = last ? remainingGross : proportion < remainingGross ? proportion : remainingGross;
    const parts = allocate(gross, [
      remainingDiscount,
      remainingShare,
      remainingGross - remainingDiscount - remainingShare,
    ]);
    const taxPart = rounded(BigInt(s(l, 'tax_minor')) * qty, base);
    const tax = last ? remainingTax : taxPart < remainingTax ? taxPart : remainingTax;
    return total + gross - parts[0] - parts[1] + tax;
  }, 0n);
}
function ReturnDialog({
  detail,
  onClose,
  onDone,
}: {
  detail: Detail;
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const h = detail.header[0];
  const [quantities, setQuantities] = useState<Record<string, string>>({}),
    [date, setDate] = useState(displayDate(businessDate())),
    [warehouse, setWarehouse] = useState(s(h, 'warehouse_id', '1')),
    [reason, setReason] = useState(''),
    [payments, setPayments] = useState<PaymentEdit[]>([]);
  const task = useTask();
  let total = 0n,
    error = '';
  try {
    total = returnValue(detail, quantities);
  } catch (e) {
    error = e instanceof Error ? e.message : t('invalid');
  }
  return (
    <Modal title="return" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-500">{t('returnHint')}</p>
      <div className="form-grid mb-4">
        <Field label="returnDate">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        <MasterSelect
          table="warehouses"
          label="warehouse"
          value={warehouse}
          onChange={setWarehouse}
        />
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} />
        </Field>
      </div>
      <DataTable
        rows={detail.lines}
        columns={[
          'product_name_snapshot',
          'unit_name_snapshot',
          {
            key: 'returnable_micros',
            label: 'remainingSource',
            render: (l) => (
              <bdi>
                {editDecimal(
                  s(
                    detail.availability.find((a) => a.id === l.id),
                    'returnable_micros',
                    s(l, 'quantity_micros'),
                  ),
                  6,
                )}
              </bdi>
            ),
          },
          {
            key: 'return_quantity',
            label: 'returnQuantity',
            render: (l) => (
              <Input
                aria-label={`${t('returnQuantity')} ${s(l, 'product_name_snapshot')}`}
                dir="ltr"
                inputMode="decimal"
                value={quantities[s(l, 'id')] ?? '0'}
                onChange={(e) => setQuantities((q) => ({ ...q, [s(l, 'id')]: e.target.value }))}
              />
            ),
          },
        ]}
      />
      <div className="mt-5">
        <Payments value={payments} onChange={setPayments} total={total} />
      </div>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <div className="mt-5 flex justify-end">
        <Button
          tone="primary"
          busy={task.busy}
          disabled={!reason.trim() || !!error || total === 0n}
          onClick={() =>
            task.run(async () => {
              if ((await confirmAction(t('confirmPost'))) === null) return;
              const lines = detail.lines
                .map((l) => ({
                  source_line_id: s(l, 'id'),
                  quantity: decimal(quantities[s(l, 'id')] ?? '0', 6),
                }))
                .filter((l) => BigInt(l.quantity) > 0n);
              const r = await api.mutate({
                type: 'return',
                source_id: s(h, 'id'),
                date: parseDate(date),
                warehouse_id: warehouse,
                lines,
                payments: paymentPayload(payments),
                reason,
              });
              toast(t('postedSuccess'));
              useStore.getState().refresh();
              onDone(r.id!);
            })
          }
        >
          {t('post')}
        </Button>
      </div>
    </Modal>
  );
}
export function DocumentDetail({
  detail,
  onEdit,
  onChanged,
  onBack,
}: {
  detail: Detail;
  onEdit?: (d: Detail) => void;
  onChanged: (id: string) => void;
  onBack?: () => void;
}) {
  const h = detail.header[0];
  const status = s(h, 'status'),
    kind = s(h, 'kind'),
    id = s(h, 'id');
  const task = useTask();
  const [returning, setReturning] = useState(false),
    [reversing, setReversing] = useState(false),
    [day, setDay] = useState(displayDate(businessDate())),
    [reason, setReason] = useState('');
  const canEdit = allowed(documentPermission(kind));
  const action = async (action: DocumentAction) => {
    const reason = await confirmAction(
      t(action === 'post' ? 'confirmPost' : 'confirmAction'),
      action === 'cancel' || action === 'close',
    );
    if (reason === null) return;
    await task.run(async () => {
      const mutation = {
        type: 'document_action' as const,
        id,
        revision: s(h, 'revision'),
        action,
        reason,
        acknowledge_credit: false,
      };
      try {
        await api.mutate(mutation);
      } catch (e) {
        if (e instanceof ApiError && e.code === 'credit_warning') {
          const why = await confirmAction(e.message, true);
          if (why === null) return;
          await api.mutate({ ...mutation, reason: why, acknowledge_credit: true });
        } else throw e;
      }
      toast(t('done'));
      useStore.getState().refresh();
      onChanged(id);
    });
  };
  return (
    <div className="space-y-4">
      <Panel>
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="mb-1 text-sm text-slate-500">{enumLabel(kind)}</p>
            <h2 className="text-2xl font-bold">
              <bdi>{s(h, 'number', `#${id}`)}</bdi>
            </h2>
            <p className="mt-2 text-slate-500">
              {s(h, 'party_name_snapshot')} · <bdi>{displayDate(s(h, 'document_date'))}</bdi>
            </p>
          </div>
          <div className="text-end">
            <Status value={status} />
            <p className="mt-3 text-2xl font-bold text-accent">
              <bdi>{money(s(h, 'total_minor'))}</bdi>
            </p>
          </div>
        </div>
        <div className="mt-5 flex flex-wrap gap-2">
          {onBack && <Button onClick={onBack}>{t('back')}</Button>}
          {canEdit && status === 'draft' && onEdit && (
            <Button tone="primary" onClick={() => onEdit(detail)}>
              {t('edit')}
            </Button>
          )}
          {canEdit &&
            status === 'draft' &&
            !['quotation', 'purchase_order', 'stocktake'].includes(kind) && (
              <Button busy={task.busy} onClick={() => void action('post')}>
                {t('post')}
              </Button>
            )}
          {canEdit && status === 'held' && (
            <Button onClick={() => void action('resume')}>{t('resume')}</Button>
          )}
          {canEdit &&
            status === 'draft' &&
            ['quotation', 'purchase_order'].includes(kind) &&
            (kind !== 'purchase_order' || allowed('purchases.approve')) && (
              <Button onClick={() => void action('approve')}>{t('approve')}</Button>
            )}
          {kind === 'quotation' && status === 'approved' && allowed('sales.write') && (
            <Button
              onClick={() =>
                task.run(async () => {
                  const r = await api.mutate({
                    type: 'convert_quotation',
                    id,
                    date: businessDate(),
                  });
                  useStore.getState().refresh();
                  await useStore.getState().navigate('pos', r.id);
                })
              }
            >
              {t('convert')}
            </Button>
          )}
          {canEdit && ['draft', 'held', 'approved'].includes(status) && (
            <Button tone="danger" onClick={() => void action('cancel')}>
              {t('cancel')}
            </Button>
          )}
          {canEdit && kind === 'purchase_order' && status === 'approved' && (
            <Button onClick={() => void action('close')}>{t('close')}</Button>
          )}
          {status === 'posted' &&
            ['sale', 'purchase'].includes(kind) &&
            allowed(kind === 'sale' ? 'sales.return' : 'purchases.write') && (
              <Button onClick={() => setReturning(true)}>{t('return')}</Button>
            )}
          {status === 'posted' &&
            allowed('documents.reverse') &&
            !['reversal', 'cheque_clear', 'cheque_bounce', 'cash_close'].includes(kind) && (
              <Button tone="danger" onClick={() => setReversing(true)}>
                {t('reverse')}
              </Button>
            )}
        </div>
      </Panel>
      {detail.lines.length > 0 && (
        <Panel>
          <DataTable
            rows={detail.lines}
            columns={[
              'product_code_snapshot',
              'product_name_snapshot',
              'unit_name_snapshot',
              'quantity_micros',
              'unit_price_minor',
              'line_discount_minor',
              'invoice_discount_share_minor',
              'tax_minor',
              'total_minor',
            ]}
          />
        </Panel>
      )}
      {detail.payments.length > 0 && (
        <Panel title="payments">
          <DataTable
            rows={detail.payments}
            columns={['method', 'cash_account_id', 'amount_minor', 'reference']}
          />
        </Panel>
      )}
      {detail.receiving.length > 0 && (
        <Panel title="receiving">
          <DataTable
            rows={detail.receiving}
            columns={[
              'document_line_id',
              'purchase_order_line_id',
              'damaged_quantity_micros',
              'short_quantity_micros',
              'inspection_notes',
            ]}
          />
        </Panel>
      )}
      {detail.expenses.length > 0 && (
        <Panel title="expense">
          <DataTable rows={detail.expenses} columns={['description', 'net_minor', 'tax_minor']} />
        </Panel>
      )}
      {detail.journal.length > 0 && (
        <Panel title="journalLines">
          <DataTable
            rows={detail.journal}
            columns={[
              'account_code',
              'account_name',
              'party_id',
              'debit_minor',
              'credit_minor',
              'due_date',
              'memo',
            ]}
          />
        </Panel>
      )}
      {detail.settlements.length > 0 && (
        <Panel title="settlements">
          <DataTable
            rows={detail.settlements}
            columns={['id', 'debit_line_id', 'credit_line_id', 'amount_minor']}
            actions={(r) =>
              allowed('payments.write') && BigInt(s(r, 'amount_minor', '0')) > 0n ? (
                <Button
                  onClick={() =>
                    task.run(async () => {
                      const reason = await confirmAction(t('unsettle'), true);
                      if (reason === null) return;
                      await api.mutate({
                        type: 'unsettle',
                        date: businessDate(),
                        settlement_id: s(r, 'id'),
                        reason,
                      });
                      toast(t('done'));
                      useStore.getState().refresh();
                      onChanged(id);
                    })
                  }
                >
                  {t('unsettle')}
                </Button>
              ) : null
            }
          />
        </Panel>
      )}
      {(h.notes || h.reason) && (
        <Panel title="notes">
          <p className="whitespace-pre-wrap">{s(h, 'notes')}</p>
          <p className="mt-2 text-slate-500">{s(h, 'reason')}</p>
        </Panel>
      )}
      <Panel title="documentEvents">
        <DataTable
          rows={detail.events}
          columns={['created_at', 'user_id', 'from_status', 'to_status', 'reason']}
        />
      </Panel>
      {returning && (
        <ReturnDialog
          detail={detail}
          onClose={() => setReturning(false)}
          onDone={(id) => {
            setReturning(false);
            onChanged(id);
          }}
        />
      )}
      {reversing && (
        <Modal title="reverse" onClose={() => setReversing(false)} busy={task.busy}>
          <div className="form-grid">
            <Field label="date">
              <Input dir="ltr" value={day} onChange={(e) => setDay(e.target.value)} />
            </Field>
            <Field label="reason">
              <Input value={reason} onChange={(e) => setReason(e.target.value)} />
            </Field>
          </div>
          <Button
            className="mt-5"
            tone="danger"
            busy={task.busy}
            disabled={!reason.trim()}
            onClick={() =>
              task.run(async () => {
                const r = await api.mutate({ type: 'reverse', id, date: parseDate(day), reason });
                setReversing(false);
                toast(t('done'));
                useStore.getState().refresh();
                onChanged(r.id!);
              })
            }
          >
            {t('reverse')}
          </Button>
        </Modal>
      )}
    </div>
  );
}
