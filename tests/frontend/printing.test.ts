import { expect, it } from 'vitest';
import { paginate } from '../../src/features/printing/paginate';
import { printFormat } from '../../src/features/printing/model';
import type { PrintSnapshot } from '../../src/api/client';
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
