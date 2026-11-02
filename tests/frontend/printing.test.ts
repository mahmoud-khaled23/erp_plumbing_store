import { expect, it } from 'vitest';
import { paginate } from '../../src/features/printing/paginate';
import { buildPrintModel, printFormat } from '../../src/features/printing/model';
import type { PrintSnapshot } from '../../src/api/client';
import { t } from '../../src/i18n';

it('prints frozen component warranties on A4 and thermal with selected digits', () => {
  const snapshot: PrintSnapshot = {
    document: {
      header: [
        {
          kind: 'sale',
          status: 'posted',
          number: 'S-1',
          document_date: '2026-10-27',
          currency_minor_digits: '2',
          currency_symbol: 'ج.م',
          total_minor: '1500',
          subtotal_minor: '1500',
          tax_minor: '0',
          line_discount_minor: '0',
          invoice_discount_minor: '0',
        },
      ],
      lines: [
        {
          product_name_snapshot: 'ماسورة',
          product_code_snapshot: 'PPR',
          unit_name_snapshot: 'قطعة',
          quantity_micros: '1000000',
          unit_price_minor: '1500',
          total_minor: '1500',
          warranty_days: '365',
          notes: 'مكون طقم',
        },
      ],
      payments: [],
      counts: [],
      expenses: [],
      events: [],
      receiving: [],
      matches: [],
      cheques: [],
      journal: [],
      settlements: [],
      availability: [],
      returned: [],
    },
    store: {},
    preferences: { digit_style: 'arabic_indic' },
    context: [],
    rows: [],
    opening: [],
    summary: [],
    references: {},
    generated_at: '2026-10-27T12:00:00Z',
  };
  for (const layout of ['a4', 'thermal80'] as const) {
    const model = buildPrintModel(snapshot, { type: 'document', id: '1' }, layout);
    const line = model.sections.find((s) => s.id === 'lines')!.rows[0];
    expect(line[0]).toContain(`${t('warrantyDays')}: ٣٦٥`);
    expect(line[0]).toContain('مكون طقم');
    expect(line.at(-1)).toContain('١٥.٠٠');
  }
  snapshot.document!.lines[0].warranty_days = null;
  expect(
    buildPrintModel(snapshot, { type: 'document', id: '1' }, 'a4').sections.find(
      (s) => s.id === 'lines',
    )!.rows[0][0],
  ).not.toContain(t('warrantyDays'));
});
it('repeats table headings and preserves every measured row in order', () => {
  const rows = Array.from({ length: 7 }, (_, i) => ({
    id: String(i),
    height: 25,
    group: 'lines',
    heading: 20,
  }));
  expect(paginate(rows, 100)).toEqual([['0', '1', '2'], ['3', '4', '5'], ['6']]);
  const mixed = [
    { id: 'a', height: 55, group: 'one', heading: 10 },
    { id: 'b', height: 25, group: 'two', heading: 15 },
  ];
  expect(paginate(mixed, 100)).toEqual([['a'], ['b']]);
  const totals = Array.from({ length: 3 }, (_, i) => ({
    id: `total${i}`,
    height: 20,
    heading: 10,
    group: 'totals',
    keepTogether: true,
  }));
  expect(paginate([mixed[0], ...totals], 100)).toEqual([['a'], ['total0', 'total1', 'total2']]);
  expect(() => paginate([{ id: 'long', height: 85, group: 'one', heading: 20 }], 100)).toThrow(
    'row-too-tall',
  );
});
it('formats historical document currency and large amounts without rounding', () => {
  const snapshot = {
    document: { header: [{ currency_minor_digits: '3', currency_symbol: 'د.ك' }] },
    store: { currency_minor_digits: '2', currency_symbol: 'ج.م' },
    preferences: { digit_style: 'western' },
    references: {},
  } as unknown as PrintSnapshot;
  const f = printFormat(snapshot);
  expect(f.money('9007199254740993')).toBe('9,007,199,254,740.993 د.ك');
  expect(f.decimal('1250000')).toBe('1.25');
  snapshot.preferences.digit_style = 'arabic_indic';
  expect(printFormat(snapshot).date('2026-10-27')).toBe('٢٧/١٠/٢٠٢٦');
});

it('prints reconciliation exceptions and whole-report summaries without page recomputation', () => {
  const snapshot = {
    document: null,
    store: { currency_minor_digits: '2', currency_symbol: 'ج.م', currency_code: 'EGP' },
    preferences: { digit_style: 'arabic_indic', timezone: 'Africa/Cairo' },
    references: {},
    generated_at: '2026-10-27T12:00:00Z',
    context: [],
    opening: [],
    rows: [
      {
        name_ar: 'المتجر بالكامل',
        stock_value_minor: '9007199254740993',
        ledger_value_minor: '9007199254740992',
        difference_minor: '1',
      },
    ],
    summary: [{ metric: 'reconciliationDifference', amount_minor: '1' }],
  } as unknown as PrintSnapshot;
  const model = buildPrintModel(
    snapshot,
    {
      type: 'report',
      report: 'inventory_reconciliation',
      from: '2026-10-01',
      to: '2026-10-27',
      party_id: null,
    },
    'a4',
  );
  expect(model.sections.find((s) => s.id === 'report')?.warningRows).toEqual([0]);
  expect(model.sections.find((s) => s.id === 'report')?.rows[0][1]).toBe('٩٠,٠٧١,٩٩٢,٥٤٧,٤٠٩.٩٣');
  expect(model.sections.find((s) => s.id === 'reportSummary')?.rows).toEqual([
    ['فرق المطابقة', '٠.٠١ ج.م'],
  ]);
  expect(model.sections.find((s) => s.id === 'reportSummary')?.warningRows).toEqual([0]);
  const f = printFormat(snapshot);
  expect(f.cell({ tax_rate_bps: null }, 'tax_rate_bps')).toBe('تسوية دون نسبة محددة');
  expect(f.cell({ tax_rate_bps: '1400' }, 'tax_rate_bps')).toBe('١٤%');
  expect(f.cell({ turnover_milli: '1104' }, 'turnover_milli')).toBe('١.١٠٤');
});
