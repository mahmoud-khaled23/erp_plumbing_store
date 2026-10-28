import { s, type Row, type PrintSnapshot } from '../../api/client';
import type { PrintSource } from '../../api/contracts';
import { columnLabel, enumLabel, t } from '../../i18n';
import { baseQuantity, displayDate, formatDecimal, localDigits } from '../../lib/numbers';
import { reportColumns } from '../reports/definitions';

export type PrintLayout = 'a4' | 'landscape' | 'thermal80' | 'labels';
export interface PrintSection {
  id: string;
  title: string;
  headers: string[];
  widths?: number[];
  rows: string[][];
}
export interface PrintModel {
  title: string;
  number: string;
  status: string;
  warning: string;
  metadata: [string, string][];
  sections: PrintSection[];
  footer: string;
}
export function printFormat(snapshot: PrintSnapshot) {
  const h = snapshot.document?.header[0];
  const digits = Number(s(h ?? snapshot.store, 'currency_minor_digits', '2'));
  const symbol = s(h ?? snapshot.store, 'currency_symbol');
  const arabic = s(snapshot.preferences, 'digit_style') === 'arabic_indic';
  const decimal = (v: string | bigint | null | undefined, places = 6) =>
    formatDecimal(v, places, arabic, true);
  return {
    arabic,
    decimal,
    money: (v: string | bigint | null | undefined) =>
      `${formatDecimal(v, digits, arabic)} ${symbol}`,
    date: (v: string | null | undefined) => displayDate(v, arabic),
    number: (v: string | number) => localDigits(String(v), arabic),
    cell: (row: Row, key: string): string => {
      const v = s(row, key);
      if (!v) return '—';
      const table: Record<string, string> = {
        warehouse_id: 'warehouses',
        cash_account_id: 'cash_accounts',
        account_id: 'accounts',
        party_id: 'parties',
      };
      if (table[key])
        return s(
          snapshot.references[table[key]]?.find((r) => r.id === v),
          'name_ar',
          v,
        );
      if (
        [
          'kind',
          'document_kind',
          'account_kind',
          'control_kind',
          'movement_type',
          'direction',
          'status',
          'progress',
          'method',
        ].includes(key)
      )
        return enumLabel(v);
      if (/^-?\d+$/.test(v)) {
        if (key.endsWith('_minor')) return formatDecimal(v, digits, arabic);
        if (key.endsWith('_micros')) return decimal(v);
        return localDigits(v, arabic);
      }
      if (/^\d{4}-\d\d-\d\d/.test(v) && (key.endsWith('_date') || key.endsWith('_at')))
        return displayDate(v, arabic);
      return v;
    },
  };
}
function textSections(id: string, title: string, text: string): PrintSection[] {
  // Long notes remain complete; modest chunks can flow across pages without clipping.
  if (!text.trim()) return [];
  const chunks = text.match(/[\s\S]{1,600}(?:\s|$)|[\s\S]{1,600}/g) ?? [text];
  return [{ id, title, headers: [], rows: chunks.map((c) => [c]) }];
}
export function buildPrintModel(
  snapshot: PrintSnapshot,
  source: PrintSource,
  layout: PrintLayout,
): PrintModel {
  const f = printFormat(snapshot);
  const model: PrintModel = {
    title: '',
    number: '',
    status: '',
    warning: '',
    metadata: [],
    sections: [],
    footer: s(snapshot.store, 'receipt_footer', s(snapshot.preferences, 'receipt_footer')),
  };
  const add = (key: string, value: string | null | undefined) => {
    if (value) model.metadata.push([t(key), value]);
  };
  if (source.type === 'document' && snapshot.document) {
    const d = snapshot.document,
      h = d.header[0],
      ctx = snapshot.context[0],
      kind = s(h, 'kind');
    const stock = [
      'goods_receipt',
      'transfer',
      'opening_stock',
      'adjustment',
      'stocktake',
    ].includes(kind);
    model.title = enumLabel(kind);
    model.number = s(h, 'number') || `${t('draftNumber')} ${f.number(source.id)}`;
    model.status = enumLabel(s(h, 'status'));
    model.warning = s(ctx, 'reversal_number')
      ? `${t('reversedBy')} ${s(ctx, 'reversal_number')}`
      : ['draft', 'held', 'cancelled'].includes(s(h, 'status'))
        ? t('notPostedPrint')
        : '';
    add('date', f.date(s(h, 'document_date')));
    if (!stock) add('due_date', h.due_date ? f.date(h.due_date) : '');
    add(
      kind.includes('purchase') || kind === 'goods_receipt' ? 'supplier' : 'party',
      s(h, 'party_name_snapshot') || (kind === 'sale' ? t('walkIn') : ''),
    );
    add('address', s(h, 'party_address_snapshot'));
    add('partyTaxNumber', s(h, 'party_tax_number_snapshot'));
    add('warehouse', s(ctx, 'warehouse_name'));
    add('destination', s(ctx, 'destination_name'));
    add('sourceDocument', s(ctx, 'source_number'));
    add('operator', s(ctx, 'operator_name'));
    if (d.lines.length) {
      const thermal = layout === 'thermal80';
      const fields = stock
        ? ['productName', 'unit', 'quantity']
        : thermal
          ? ['productName', 'quantity', 'price', 'total']
          : ['productName', 'unit', 'quantity', 'price', 'discount', 'tax', 'total'];
      const rows = d.lines.map((l) => {
        const name = [s(l, 'product_name_snapshot'), s(l, 'product_code_snapshot'), s(l, 'notes')]
          .filter(Boolean)
          .join('\n');
        const discount = (
          BigInt(s(l, 'line_discount_minor', '0')) +
          BigInt(s(l, 'invoice_discount_share_minor', '0'))
        ).toString();
        if (stock) return [name, s(l, 'unit_name_snapshot'), f.decimal(l.quantity_micros)];
        if (thermal)
          return [
            [
              name,
              s(l, 'unit_name_snapshot'),
              BigInt(discount) ? `${t('discount')}: ${f.money(discount)}` : '',
              BigInt(s(l, 'tax_minor', '0')) ? `${t('tax')}: ${f.money(l.tax_minor)}` : '',
            ]
              .filter(Boolean)
              .join('\n'),
            f.decimal(l.quantity_micros),
            f.money(l.unit_price_minor),
            f.money(l.total_minor),
          ];
        return [
          name,
          s(l, 'unit_name_snapshot'),
          f.decimal(l.quantity_micros),
          f.money(l.unit_price_minor),
          f.money(discount),
          `${f.money(l.tax_minor)}\n${f.decimal(l.tax_rate_bps, 2)}%`,
          f.money(l.total_minor),
        ];
      });
      model.sections.push({
        id: 'lines',
        title: '',
        headers: fields.map(t),
        widths: stock ? [62, 18, 20] : thermal ? [40, 17, 20, 23] : [30, 10, 10, 13, 12, 12, 13],
        rows,
      });
    }
    if (d.receiving.length)
      model.sections.push({
        id: 'inspection',
        title: t('receivingInspection'),
        headers: ['productName', 'damaged', 'short', 'notes'].map(t),
        widths: [40, 15, 15, 30],
        rows: d.receiving.map((r) => {
          const l = d.lines.find((l) => l.id === r.document_line_id)!;
          const qty = (key: string) =>
            f.decimal(
              baseQuantity(s(r, key, '0'), s(l, 'factor_denominator'), s(l, 'factor_numerator')),
            );
          return [
            s(l, 'product_name_snapshot'),
            `${qty('damaged_quantity_micros')} ${s(l, 'unit_name_snapshot')}`,
            `${qty('short_quantity_micros')} ${s(l, 'unit_name_snapshot')}`,
            s(r, 'inspection_notes'),
          ];
        }),
      });
    if (d.counts.length)
      model.sections.push({
        id: 'counts',
        title: t('countSheet'),
        headers: ['code', 'productName', 'expected', 'counted', 'difference', 'reason'].map(t),
        rows: d.counts.map((r) => [
          s(r, 'code'),
          s(r, 'name_ar'),
          f.decimal(r.expected_quantity_micros),
          r.counted_quantity_micros === null ? '' : f.decimal(r.counted_quantity_micros),
          r.counted_quantity_micros === null
            ? ''
            : f.decimal(
                BigInt(r.counted_quantity_micros) - BigInt(s(r, 'expected_quantity_micros', '0')),
              ),
          s(r, 'reason'),
        ]),
      });
    if (d.expenses.length)
      model.sections.push({
        id: 'expenses',
        title: t('expense'),
        headers: ['description', 'net', 'tax'].map(t),
        rows: d.expenses.map((r) => [
          s(r, 'description'),
          f.money(r.net_minor),
          f.money(r.tax_minor),
        ]),
      });
    if (!stock && d.lines.length) {
      const paid = d.payments.reduce((v, p) => v + BigInt(s(p, 'amount_minor', '0')), 0n);
      const rows = [
        ['subtotal', h.subtotal_minor],
        ['lineDiscount', h.line_discount_minor],
        ['invoiceDiscount', h.invoice_discount_minor],
        ['tax', h.tax_minor],
        ['total', h.total_minor],
      ].map(([label, value]) => [t(label!), f.money(value)]);
      if (['sale', 'purchase', 'sale_return', 'purchase_return'].includes(kind))
        rows.push(
          [t(kind.endsWith('return') ? 'refundedAtIssue' : 'paidAtIssue'), f.money(paid)],
          [t('remainingAtIssue'), f.money(BigInt(s(h, 'total_minor', '0')) - paid)],
        );
      model.sections.push({
        id: 'totals',
        title: t('invoiceSummary'),
        headers: [],
        widths: [65, 35],
        rows,
      });
    }
    if (d.payments.length)
      model.sections.push({
        id: 'payments',
        title: t('payments'),
        headers: ['pay', 'amount', 'reference'].map(t),
        widths: [25, 25, 50],
        rows: d.payments.map((p) => {
          const cheque = d.cheques.find((c) => c.payment_leg_id === p.id);
          return [
            enumLabel(s(p, 'method')),
            f.money(p.amount_minor),
            [
              s(p, 'reference'),
              s(cheque, 'cheque_number'),
              s(cheque, 'bank_name'),
              cheque?.due_date ? f.date(cheque.due_date) : '',
            ]
              .filter(Boolean)
              .join(' · '),
          ];
        }),
      });
    if (!d.lines.length && !stock && d.journal.length)
      model.sections.push({
        id: 'journal',
        title: t('journalLines'),
        headers: ['account', 'debit', 'creditAmount', 'description'].map(t),
        rows: d.journal.map((l) => [
          `${s(l, 'account_code')} ${s(l, 'account_name')}`,
          f.money(l.debit_minor),
          f.money(l.credit_minor),
          s(l, 'memo'),
        ]),
      });
    model.sections.push(
      ...textSections('notes', t('notes'), s(h, 'notes')),
      ...textSections('reason', t('reason'), s(h, 'reason')),
    );
    model.sections.push({
      id: 'signatures',
      title: '',
      headers: [],
      rows: [[`${t('preparedBy')}\n\n________________`, `${t('receivedBy')}\n\n________________`]],
    });
  } else if (source.type === 'report' || source.type === 'stock_card') {
    const card = source.type === 'stock_card';
    model.title = card ? t('stockCard') : t(`reportNames.${source.report}`);
    add(
      'currency',
      `${s(snapshot.store, 'currency_code')} (${s(snapshot.store, 'currency_symbol')})`,
    );
    add(
      'reportGeneratedAt',
      f.number(
        new Intl.DateTimeFormat('en-GB', {
          timeZone: s(snapshot.preferences, 'timezone', 'Africa/Cairo'),
          dateStyle: 'short',
          timeStyle: 'short',
          hourCycle: 'h23',
        }).format(new Date(snapshot.generated_at)),
      ),
    );
    if (!card) {
      if (['low_stock', 'open_items'].includes(source.report))
        add('reportScope', t('currentSnapshot'));
      else if (
        ['valuation', 'receivables', 'payables', 'aging', 'trial_balance'].includes(source.report)
      )
        add('asOf', f.date(source.to));
      else {
        add('from', f.date(source.from));
        add('to', f.date(source.to));
      }
    }
    add(card ? 'productName' : 'party', s(snapshot.context[0], 'name_ar'));
    if (card) add('warehouse', s(snapshot.context[0], 'warehouse_name'));
    if (snapshot.opening.length)
      model.sections.push({
        id: 'opening',
        title: t('opening'),
        headers: ['account', 'balance'].map(t),
        rows: snapshot.opening.map((r) => [f.cell(r, 'account_id'), f.money(r.balance_minor)]),
      });
    const columns = card
      ? [
          'document_date',
          'number',
          'movement_type',
          'quantity_micros',
          'value_minor',
          'running_quantity_micros',
          'running_value_minor',
          'reason',
        ]
      : reportColumns[source.report];
    model.sections.push({
      id: 'report',
      title: '',
      headers: columns.map(columnLabel),
      rows: snapshot.rows.map((r) => columns.map((c) => f.cell(r, c))),
    });
    const totals: Record<string, string[]> = {
      trial_balance: ['opening_minor', 'debit_minor', 'credit_minor', 'closing_minor'],
      income_statement: ['profit_signed_minor'],
      cash_flow: ['cash_in_minor', 'cash_out_minor'],
      product_profit: ['net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
      category_profit: ['net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
      customer_profit: ['net_sales_minor', 'cogs_minor', 'gross_profit_minor'],
      receivables: ['balance_minor'],
      payables: ['balance_minor'],
      valuation: ['value_minor'],
    };
    const keys = card ? [] : (totals[source.report] ?? []);
    if (keys.length && snapshot.rows.length)
      model.sections.push({
        id: 'reportTotals',
        title: t('reportTotals'),
        headers: [],
        rows: keys.map((key) => [
          columnLabel(key),
          f.money(snapshot.rows.reduce((v, r) => v + BigInt(s(r, key, '0')), 0n)),
        ]),
      });
    add('rowCount', f.number(snapshot.rows.length));
  }
  return model;
}
