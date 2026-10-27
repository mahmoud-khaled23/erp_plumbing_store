import { useEffect, useRef, useState } from 'react';
import { api, ApiError, s, type DocumentDetail, type Row } from '../../api/client';
import type { DocumentKind, Draft, DraftLine } from '../../api/contracts';
import { t } from '../../i18n';
import {
  allowed,
  arabicDigits,
  businessDate,
  currentSettings,
  money,
  moneyDigits,
  toast,
  useStore,
} from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import {
  allocate,
  baseQuantity,
  decimal,
  displayDate,
  editDecimal,
  formatDecimal,
  parseDate,
  rounded,
} from '../../lib/numbers';
import { useDirty, useHotkeys, useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  Check,
  Empty,
  ErrorState,
  Field,
  Input,
  Loading,
  Panel,
  Select,
} from '../../components/ui';
import { MasterSelect, PartyPicker, ProductSearch } from '../../components/Pickers';
import { Payments, paymentPayload, type PaymentEdit } from '../../components/Payments';
import { SourcePicker } from './SourcePicker';
import { invoiceTotals } from './calculation';

interface LineEdit {
  key: string;
  product: Row;
  units: Row[];
  unit: string;
  quantity: string;
  price: string;
  discount: string;
  tax: string;
  orderLine: string | null;
  damaged: string;
  short: string;
  matches: { receipt_line_id: string; base_quantity: string }[];
  notes: string;
}
const stockKinds = new Set<DocumentKind>(['goods_receipt', 'transfer', 'opening_stock']);
export function DocumentEditor({
  kind,
  initial,
  onComplete,
  onCancel,
}: {
  kind: DocumentKind;
  initial?: DocumentDetail;
  onComplete: (id: string) => void;
  onCancel?: () => void;
}) {
  const settings = currentSettings();
  const digits = moneyDigits(),
    stock = stockKinds.has(kind),
    sales = kind === 'sale' || kind === 'quotation',
    payable = kind === 'sale' || kind === 'purchase';
  const h = initial?.header[0];
  const [id, setId] = useState<string | null>(h ? s(h, 'id') : null),
    [revision, setRevision] = useState(s(h, 'revision', '0'));
  const [date, setDate] = useState(displayDate(s(h, 'document_date', businessDate()))),
    [due, setDue] = useState(h?.due_date ? displayDate(h.due_date) : '');
  const [party, setParty] = useState<string | null>(h?.party_id ?? null),
    [partyName, setPartyName] = useState(s(h, 'party_name_snapshot'));
  const [warehouse, setWarehouse] = useState(s(h, 'warehouse_id', '1')),
    [destination, setDestination] = useState(s(h, 'destination_warehouse_id'));
  const [wholesale, setWholesale] = useState(h?.price_list === 'wholesale'),
    [discount, setDiscount] = useState(editDecimal(s(h, 'invoice_discount_minor', '0'), digits));
  const [notes, setNotes] = useState(s(h, 'notes')),
    [reason, setReason] = useState(s(h, 'reason')),
    [lines, setLines] = useState<LineEdit[]>([]),
    [payments, setPayments] = useState<PaymentEdit[]>([]);
  const [source, setSource] = useState<string | null>(h?.source_document_id ?? null),
    [picking, setPicking] = useState(false),
    [ready, setReady] = useState(!initial),
    [dirty, setDirty] = useState(false);
  const task = useTask();
  const linesRef = useRef(lines);
  linesRef.current = lines;
  const [loadError, setLoadError] = useState<unknown>();
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [searching, setSearching] = useState(false);
  const desk = useQuery(allowed('cash.shift') ? { type: 'cash_desk' } : null);
  useDirty(dirty);
  useEffect(() => {
    if (!initial) return;
    let live = true;
    setLoadError(undefined);
    setReady(false);
    void (async () => {
      const productIds = [...new Set(initial.lines.map((l) => s(l, 'product_id')))];
      const details = await Promise.all(productIds.map((id) => api.query({ type: 'product', id })));
      if (!live) return;
      setLines(
        initial.lines.map((l) => {
          const product = details.find((p) => s(p.product[0], 'id') === s(l, 'product_id'))!;
          const receiving = initial.receiving.find((r) => s(r, 'document_line_id') === s(l, 'id'));
          return {
            key: crypto.randomUUID(),
            product: product.product[0],
            units: product.units,
            unit: s(l, 'product_unit_id'),
            quantity: editDecimal(s(l, 'quantity_micros'), 6),
            price: editDecimal(s(l, 'unit_price_minor'), digits),
            discount: editDecimal(s(l, 'line_discount_minor'), digits),
            tax: editDecimal(s(l, 'tax_rate_bps'), 2),
            orderLine: receiving?.purchase_order_line_id ?? null,
            damaged: editDecimal(
              baseQuantity(
                s(receiving, 'damaged_quantity_micros', '0'),
                s(l, 'factor_denominator'),
                s(l, 'factor_numerator'),
              ),
              6,
            ),
            short: editDecimal(
              baseQuantity(
                s(receiving, 'short_quantity_micros', '0'),
                s(l, 'factor_denominator'),
                s(l, 'factor_numerator'),
              ),
              6,
            ),
            matches: initial.matches
              .filter((m) => m.invoice_line_id === l.id)
              .map((m) => ({
                receipt_line_id: s(m, 'receipt_line_id'),
                base_quantity: s(m, 'base_quantity_micros'),
              })),
            notes: s(l, 'notes'),
          };
        }),
      );
      setPayments(
        initial.payments.map((p) => {
          const c = initial.cheques.find((c) => c.payment_leg_id === p.id);
          return {
            method: s(p, 'method') as PaymentEdit['method'],
            account: s(p, 'cash_account_id'),
            shift: s(p, 'shift_id'),
            amount: editDecimal(s(p, 'amount_minor'), digits),
            reference: s(p, 'reference'),
            number: s(c, 'cheque_number'),
            bank: s(c, 'bank_name'),
            due: displayDate(s(c, 'due_date', businessDate())),
          };
        }),
      );
      setReady(true);
    })().catch((error) => {
      if (live) setLoadError(error);
    });
    return () => {
      live = false;
    };
  }, [initial, loadAttempt]);
  const mark =
    <T,>(setter: (v: T) => void) =>
    (v: T) => {
      setDirty(true);
      setter(v);
    };
  const changeLine = (index: number, patch: Partial<LineEdit>) => {
    setDirty(true);
    setLines((v) => v.map((l, i) => (i === index ? { ...l, ...patch } : l)));
  };
  function linePayload(l: LineEdit): DraftLine {
    const unit = l.units.find((u) => u.id === l.unit);
    if (!unit) throw new Error(t('chooseUnit'));
    const quantity = decimal(l.quantity, 6),
      price = decimal(l.price, digits),
      tax = Number(decimal(l.tax, 2));
    if (
      kind === 'purchase' &&
      l.matches.reduce((a, m) => a + BigInt(m.base_quantity), 0n) !==
        BigInt(baseQuantity(quantity, s(unit, 'factor_numerator'), s(unit, 'factor_denominator')))
    )
      throw new Error(t('quantityMismatch'));
    return {
      unit_id: l.unit,
      quantity,
      price,
      discount: decimal(l.discount, digits),
      tax_bps: stock ? 0 : tax,
      order_line_id: l.orderLine,
      damaged: decimal(l.damaged, 6),
      short: decimal(l.short, 6),
      matches: l.matches,
      notes: l.notes,
    };
  }
  let calculation: ReturnType<typeof invoiceTotals> | undefined;
  let calcError = '';
  try {
    calculation = invoiceTotals(lines.map(linePayload), stock ? '0' : decimal(discount, digits));
  } catch (e) {
    calcError = e instanceof Error ? e.message : t('invalid');
  }
  const total = calculation?.total ?? 0n;
  let paid = 0n;
  try {
    paid = payments.reduce((a, p) => a + BigInt(decimal(p.amount, digits)), 0n);
  } catch {
    calcError = t('invalidNumber');
  }
  async function addProduct(row: Row, sourceLine?: Row, sourceDocument?: DocumentDetail) {
    const detail = await api.query({ type: 'product', id: s(row, 'id', s(row, 'product_id')) });
    const units = detail.units.filter((u) => u.active === '1');
    let selected =
      units.find((u) => u.id === (sourceLine?.product_unit_id ?? row.matched_unit_id)) ??
      units.find((u) => u.unit_id === detail.product[0]?.base_unit_id) ??
      units[0];
    if (!selected) throw new Error(t('chooseUnit'));
    const availability = sourceDocument?.availability.find((a) => a.id === sourceLine?.id);
    if (
      sourceLine &&
      linesRef.current.some(
        (l) =>
          l.orderLine === sourceLine.id ||
          l.matches.some((m) => m.receipt_line_id === sourceLine.id),
      )
    )
      return;
    let quantity = '1000000';
    if (sourceLine) {
      const remaining = s(
        availability,
        kind === 'purchase' ? 'unbilled_micros' : 'unreceived_micros',
        s(sourceLine, 'base_quantity_micros'),
      );
      try {
        quantity = baseQuantity(
          remaining,
          s(selected, 'factor_denominator'),
          s(selected, 'factor_numerator'),
        );
      } catch {
        selected = units.find((u) => u.unit_id === detail.product[0].base_unit_id)!;
        if (!selected) throw new Error(t('badConversion'));
        quantity = baseQuantity(
          remaining,
          s(selected, 'factor_denominator'),
          s(selected, 'factor_numerator'),
        );
      }
    }
    if (BigInt(quantity) <= 0n) return;
    const unitPrice = sourceLine
      ? rounded(
          BigInt(s(sourceLine, 'unit_price_minor')) *
            BigInt(s(sourceLine, 'factor_denominator')) *
            BigInt(s(selected, 'factor_numerator')),
          BigInt(s(sourceLine, 'factor_numerator')) * BigInt(s(selected, 'factor_denominator')),
        ).toString()
      : kind === 'transfer'
        ? '0'
        : sales
          ? s(selected, wholesale ? 'wholesale_price_minor' : 'retail_price_minor')
          : rounded(
              BigInt(s(detail.product[0], 'standard_cost_minor')) *
                BigInt(s(selected, 'factor_numerator')),
              BigInt(s(selected, 'factor_denominator')),
            ).toString();
    const added: LineEdit = {
      key: crypto.randomUUID(),
      product: detail.product[0],
      units,
      unit: s(selected, 'id'),
      quantity: editDecimal(quantity, 6),
      price: editDecimal(unitPrice, digits),
      discount: '0',
      tax: stock
        ? '0'
        : s(settings, 'vat_enabled') === '1'
          ? editDecimal(s(settings, 'default_vat_rate_bps'), 2)
          : '0',
      orderLine: kind === 'goods_receipt' && sourceLine ? s(sourceLine, 'id') : null,
      damaged: '0',
      short: '0',
      matches:
        kind === 'purchase' && sourceLine
          ? [
              {
                receipt_line_id: s(sourceLine, 'id'),
                base_quantity: baseQuantity(
                  quantity,
                  s(selected, 'factor_numerator'),
                  s(selected, 'factor_denominator'),
                ),
              },
            ]
          : [],
      notes: '',
    };
    const at =
      !sourceLine && kind !== 'purchase'
        ? linesRef.current.findIndex((l) => l.unit === added.unit && !l.orderLine)
        : -1;
    const nextQuantity =
      at >= 0 ? editDecimal(BigInt(decimal(linesRef.current[at].quantity, 6)) + 1000000n, 6) : '';
    const current = linesRef.current;
    const next =
      at < 0
        ? [...current, added]
        : current.map((l, i) => (i === at ? { ...l, quantity: nextQuantity } : l));
    linesRef.current = next;
    setLines(next);
    setDirty(true);
  }
  async function save() {
    if (!lines.length) throw new Error(t('noLines'));
    if (calcError) throw new Error(calcError);
    const draft: Draft = {
      id,
      revision,
      kind,
      date: parseDate(date),
      due_date: due ? parseDate(due) : null,
      party_id: party,
      warehouse_id: warehouse || null,
      destination_id: destination || null,
      source_id: source,
      wholesale,
      discount: stock ? '0' : decimal(discount, digits),
      lines: lines.map(linePayload),
      payments: payable ? paymentPayload(payments) : [],
      notes,
      reason,
    };
    const result = await api.mutate({ type: 'save_draft', draft });
    if (!result.id || !result.revision) throw new Error(t('unexpected'));
    setId(result.id);
    setRevision(result.revision);
    setDirty(false);
    return result as { id: string; revision: string };
  }
  async function execute(action: 'save' | 'post' | 'hold') {
    if (searching) {
      toast(t('waitForBarcode'), 'error');
      return;
    }
    if (action === 'post' && (await confirmAction(t('confirmPost'))) === null) return;
    await task.run(async () => {
      const saved = await save();
      if (action === 'save') {
        useStore.getState().refresh();
        toast(t('saved'));
        return;
      }
      const mutation = {
        type: 'document_action' as const,
        id: saved.id,
        revision: saved.revision,
        action,
        reason,
        acknowledge_credit: false,
      };
      try {
        await api.mutate(mutation);
      } catch (error) {
        if (action === 'post' && error instanceof ApiError && error.code === 'credit_warning') {
          const acknowledged = await confirmAction(error.message, true);
          if (acknowledged === null) return;
          await api.mutate({ ...mutation, reason: acknowledged, acknowledge_credit: true });
        } else throw error;
      }
      toast(t(action === 'post' ? 'postedSuccess' : 'saved'));
      useStore.getState().setDirty(false);
      useStore.getState().refresh();
      onComplete(saved.id);
    });
  }
  useHotkeys((e) => {
    if (document.querySelector('dialog[open]')) return;
    if (e.key === 'F4') {
      e.preventDefault();
      document.querySelector<HTMLInputElement>('[data-product-search]')?.focus();
    }
    if (e.key === 'F8') {
      e.preventDefault();
      void execute('save');
    }
    if (e.key === 'F9' && kind !== 'quotation' && kind !== 'purchase_order') {
      e.preventDefault();
      void execute('post');
    }
  });
  if (loadError) return <ErrorState error={loadError} retry={() => setLoadAttempt((v) => v + 1)} />;
  if (!ready) return <Loading />;
  const priceDisabled = sales && !allowed('sales.discount');
  return (
    <div className="space-y-4">
      <Panel>
        <div className={`grid grid-cols-2 gap-4 ${sales ? 'xl:grid-cols-5' : 'lg:grid-cols-4'}`}>
          <Field label="date">
            <Input dir="ltr" value={date} onChange={(e) => mark(setDate)(e.target.value)} />
          </Field>
          {!stock && (
            <Field label="due_date">
              <Input
                dir="ltr"
                placeholder={t('dateFormat')}
                value={due}
                onChange={(e) => mark(setDue)(e.target.value)}
              />
            </Field>
          )}
          {kind !== 'transfer' && kind !== 'opening_stock' && (
            <PartyPicker
              value={party}
              name={partyName}
              supplier={!sales}
              optional={sales}
              onChange={(id, r) => {
                setPartyName(s(r, 'name_ar'));
                mark(setParty)(id);
              }}
            />
          )}
          <MasterSelect
            table="warehouses"
            label="warehouse"
            value={warehouse}
            onChange={mark(setWarehouse)}
          />
          {kind === 'transfer' && (
            <MasterSelect
              table="warehouses"
              label="destination"
              value={destination}
              onChange={mark(setDestination)}
            />
          )}{' '}
          {sales && (
            <Field label="priceList">
              <Select
                value={wholesale ? 'wholesale' : 'retail'}
                onChange={(e) => {
                  const whole = e.target.value === 'wholesale';
                  setWholesale(whole);
                  setLines((ls) =>
                    ls.map((l) => ({
                      ...l,
                      price: editDecimal(
                        s(
                          l.units.find((u) => u.id === l.unit),
                          whole ? 'wholesale_price_minor' : 'retail_price_minor',
                        ),
                        digits,
                      ),
                    })),
                  );
                  setDirty(true);
                }}
              >
                <option value="retail">{t('retail')}</option>
                <option value="wholesale">{t('wholesale')}</option>
              </Select>
            </Field>
          )}
        </div>
      </Panel>
      <Panel>
        <div className="mb-4 flex items-start gap-3">
          {kind !== 'purchase' && (
            <div className="flex-1">
              <ProductSearch autoFocus onSelect={(r) => addProduct(r)} onPending={setSearching} />
              <p className="mt-2 text-xs text-slate-500">{t('keyboardHint')}</p>
            </div>
          )}
          {(kind === 'goods_receipt' || kind === 'purchase') && (
            <Button onClick={() => setPicking(true)}>
              {t(kind === 'purchase' ? 'linkedReceipt' : 'linkedOrder')}
            </Button>
          )}
        </div>
        {!lines.length ? (
          <Empty title="cartEmpty" hint="cartEmptyHint" />
        ) : (
          <div className="table-scroll">
            <table className="data-table editor-table">
              <thead>
                <tr>
                  {[
                    'product',
                    'unit',
                    stock ? 'accepted' : 'quantity',
                    'price',
                    ...(!stock ? ['lineDiscount', 'vatPercent'] : []),
                    ...(kind === 'goods_receipt' ? ['damaged', 'short'] : []),
                    'total',
                    'actions',
                  ].map((k) => (
                    <th key={k}>{t(k)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {lines.map((l, i) => (
                  <tr key={l.key}>
                    <td>
                      <div className="font-semibold">{s(l.product, 'name_ar')}</div>
                      <bdi className="text-xs text-slate-400">{s(l.product, 'code')}</bdi>
                      {(l.orderLine || l.matches.length > 0) && (
                        <span className="block text-xs text-teal-700">
                          {t('sourceLine')}:{' '}
                          <bdi>
                            {l.orderLine || l.matches.map((m) => m.receipt_line_id).join(', ')}
                          </bdi>
                        </span>
                      )}
                      <input
                        aria-label={t('notes')}
                        className="mt-1 w-full bg-transparent text-xs"
                        placeholder={t('notes')}
                        value={l.notes}
                        onChange={(e) => changeLine(i, { notes: e.target.value })}
                      />
                    </td>
                    <td>
                      <Select
                        aria-label={t('unit')}
                        value={l.unit}
                        disabled={l.matches.length > 0}
                        onChange={(e) => {
                          const unit = l.units.find((u) => u.id === e.target.value)!;
                          try {
                            const old = l.units.find((u) => u.id === l.unit)!;
                            const price = sales
                              ? s(unit, wholesale ? 'wholesale_price_minor' : 'retail_price_minor')
                              : rounded(
                                  BigInt(decimal(l.price, digits)) *
                                    BigInt(s(old, 'factor_denominator')) *
                                    BigInt(s(unit, 'factor_numerator')),
                                  BigInt(s(old, 'factor_numerator')) *
                                    BigInt(s(unit, 'factor_denominator')),
                                ).toString();
                            changeLine(i, {
                              unit: e.target.value,
                              price: editDecimal(price, digits),
                            });
                          } catch (error) {
                            toast(error instanceof Error ? error.message : t('invalid'), 'error');
                          }
                        }}
                      >
                        {l.units.map((u) => (
                          <option key={s(u, 'id')} value={s(u, 'id')}>
                            {s(u, 'name_ar')}
                          </option>
                        ))}
                      </Select>
                    </td>
                    <td>
                      <Input
                        aria-label={t('quantity')}
                        dir="ltr"
                        inputMode="decimal"
                        value={l.quantity}
                        onChange={(e) => {
                          const quantity = e.target.value;
                          let matches = l.matches;
                          if (kind === 'purchase' && matches.length === 1) {
                            try {
                              const u = l.units.find((u) => u.id === l.unit)!;
                              matches = [
                                {
                                  ...matches[0],
                                  base_quantity: baseQuantity(
                                    decimal(quantity, 6),
                                    s(u, 'factor_numerator'),
                                    s(u, 'factor_denominator'),
                                  ),
                                },
                              ];
                            } catch {}
                          }
                          changeLine(i, { quantity, matches });
                        }}
                      />
                    </td>
                    <td>
                      <Input
                        aria-label={t('price')}
                        disabled={priceDisabled || kind === 'transfer'}
                        dir="ltr"
                        inputMode="decimal"
                        value={l.price}
                        onChange={(e) => changeLine(i, { price: e.target.value })}
                      />
                    </td>
                    {!stock && (
                      <>
                        <td>
                          <Input
                            aria-label={t('lineDiscount')}
                            dir="ltr"
                            disabled={priceDisabled}
                            value={l.discount}
                            onChange={(e) => changeLine(i, { discount: e.target.value })}
                          />
                        </td>
                        <td>
                          <Input
                            aria-label={t('vatPercent')}
                            dir="ltr"
                            disabled={s(settings, 'vat_enabled') !== '1'}
                            value={l.tax}
                            onChange={(e) => changeLine(i, { tax: e.target.value })}
                          />
                        </td>
                      </>
                    )}
                    {kind === 'goods_receipt' && (
                      <>
                        <td>
                          <Input
                            aria-label={t('damaged')}
                            dir="ltr"
                            value={l.damaged}
                            onChange={(e) => changeLine(i, { damaged: e.target.value })}
                          />
                        </td>
                        <td>
                          <Input
                            aria-label={t('short')}
                            dir="ltr"
                            value={l.short}
                            onChange={(e) => changeLine(i, { short: e.target.value })}
                          />
                        </td>
                      </>
                    )}
                    <td>
                      <bdi className="font-semibold">
                        {calculation ? money(calculation.totals[i]) : '—'}
                      </bdi>
                    </td>
                    <td>
                      <Button
                        tone="ghost"
                        onClick={() => {
                          setLines((v) => v.filter((_, j) => i !== j));
                          setDirty(true);
                        }}
                        aria-label={t('remove')}
                      >
                        ×
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
      <div className="grid gap-4 xl:grid-cols-[1fr_340px]">
        <Panel>
          <div className="form-grid">
            <Field label="notes">
              <textarea
                className="input"
                value={notes}
                onChange={(e) => mark(setNotes)(e.target.value)}
              />
            </Field>
            <Field label="reason">
              <Input value={reason} onChange={(e) => mark(setReason)(e.target.value)} />
            </Field>
          </div>
          {payable && (
            <div className="mt-5">
              <Payments
                value={payments}
                onChange={mark(setPayments)}
                total={total}
                shifts={(desk.data?.shifts ?? []).map((r) => ({
                  id: s(r, 'id'),
                  cash_account_id: r.cash_account_id,
                }))}
              />
            </div>
          )}
        </Panel>
        <Panel className="invoice-summary">
          <div className="summary-line">
            <span>{t('subtotal')}</span>
            <bdi>{money(calculation?.gross)}</bdi>
          </div>
          <div className="summary-line">
            <span>{t('lineDiscount')}</span>
            <bdi>{money(calculation?.lineDiscount)}</bdi>
          </div>
          {!stock && (
            <Field label="invoiceDiscount">
              <Input
                dir="ltr"
                disabled={priceDisabled}
                value={discount}
                onChange={(e) => mark(setDiscount)(e.target.value)}
              />
            </Field>
          )}
          <div className="summary-line">
            <span>{t('tax')}</span>
            <bdi>{money(calculation?.tax)}</bdi>
          </div>
          <div className="summary-total">
            <span>{t('total')}</span>
            <bdi>{money(total)}</bdi>
          </div>
          {payable && (
            <>
              <div className="summary-line">
                <span>{t('paid')}</span>
                <bdi>{money(paid)}</bdi>
              </div>
              <div className="summary-line">
                <span>{t('remaining')}</span>
                <bdi>{money(total - paid)}</bdi>
              </div>
            </>
          )}
          {calcError && (
            <p role="alert" className="field-error my-2">
              {calcError}
            </p>
          )}
          <div className="mt-5 grid grid-cols-2 gap-2">
            <Button busy={task.busy} disabled={searching} onClick={() => void execute('save')}>
              {t('saveDraft')} <kbd>F8</kbd>
            </Button>
            {kind === 'sale' && (
              <Button busy={task.busy} disabled={searching} onClick={() => void execute('hold')}>
                {t('hold')}
              </Button>
            )}
            {kind !== 'quotation' && kind !== 'purchase_order' && (
              <Button
                className="col-span-2"
                tone="primary"
                busy={task.busy}
                disabled={searching || !lines.length || !!calcError}
                onClick={() => void execute('post')}
              >
                {t('post')} <kbd>F9</kbd>
              </Button>
            )}
            {onCancel && (
              <Button
                className="col-span-2"
                onClick={async () => {
                  if (dirty && (await confirmAction(t('unsaved'))) === null) return;
                  onCancel();
                }}
              >
                {t('back')}
              </Button>
            )}
          </div>
        </Panel>
      </div>
      {picking && (
        <SourcePicker
          kind={kind === 'purchase' ? 'goods_receipt' : 'purchase_order'}
          onClose={() => setPicking(false)}
          onSelect={async (document) => {
            const header = document.header[0];
            if (party && party !== header.party_id && lines.length) throw new Error(t('invalid'));
            setParty(header.party_id ?? null);
            setPartyName(s(header, 'party_name_snapshot'));
            setSource(s(header, 'id'));
            for (const line of document.lines)
              await addProduct({ id: s(line, 'product_id') }, line, document);
          }}
        />
      )}
    </div>
  );
}
