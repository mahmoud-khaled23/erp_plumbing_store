import { describe, it, expect } from 'vitest';
import {
  allocate,
  baseQuantity,
  decimal,
  displayDate,
  editDecimal,
  formatDecimal,
  parseDate,
  rounded,
} from '../../src/lib/numbers';
import { invoiceTotals } from '../../src/features/documents/calculation';
describe('exact Arabic input and formatting', () => {
  it('parses Arabic and Persian digits without binary floating point', () => {
    expect(decimal('١٢٣٫٤٥', 2)).toBe('12345');
    expect(decimal('۱۲۳.۴۵', 2)).toBe('12345');
    expect(decimal('90071992547409.93', 2)).toBe('9007199254740993');
    expect(decimal('-٠٫١٢٥', 6, true)).toBe('-125000');
  });
  it('rejects lost precision and ambiguous inputs', () => {
    for (const value of ['1e3', 'NaN', '1,250', '1.001', '', '2.5.1'])
      expect(() => decimal(value, 2)).toThrow();
    expect(() => decimal('-1', 2)).toThrow();
  });
  it('preserves every digit in large formatted balances', () => {
    expect(formatDecimal('9007199254740993', 2)).toBe('90,071,992,547,409.93');
    expect(formatDecimal('1250000', 6, true, true)).toBe('١.٢٥');
    expect(formatDecimal('100000000', 6, false, true)).toBe('100');
    expect(editDecimal('-1', 2)).toBe('-0.01');
  });
  it('conserves remainders and rejects inexact unit conversions', () => {
    expect(allocate(5n, [1n, 1n, 1n])).toEqual([2n, 2n, 1n]);
    expect(allocate(0n, [0n, 0n])).toEqual([0n, 0n]);
    expect(rounded(15n, 10n)).toBe(2n);
    expect(rounded(-15n, 10n)).toBe(-2n);
    expect(baseQuantity('2500000', '12', '1')).toBe('30000000');
    expect(() => baseQuantity('1000000', '1', '3')).toThrow();
  });
  it('validates Gregorian calendar dates and displays dd/MM/yyyy', () => {
    expect(parseDate('٢٩/٠٢/٢٠٢٤')).toBe('2024-02-29');
    expect(() => parseDate('29/02/2025')).toThrow();
    expect(() => parseDate('31/04/2026')).toThrow();
    expect(displayDate('2026-10-27', true)).toBe('٢٧/١٠/٢٠٢٦');
  });
});
describe('invoice preview matches integer posting rules', () => {
  it('allocates invoice discounts before rounding VAT per line', () => {
    const rows = [
      { quantity: '1500000', price: '101', discount: '2', tax_bps: 1400 },
      { quantity: '250000', price: '399', discount: '0', tax_bps: 1400 },
    ];
    const result = invoiceTotals(rows, '3');
    expect(result.gross).toBe(252n);
    expect(result.lineDiscount).toBe(2n);
    expect(result.discount).toBe(3n);
    expect(result.tax).toBe(35n);
    expect(result.total).toBe(282n);
    expect(result.totals.reduce((a, b) => a + b, 0n)).toBe(result.total);
  });
  it('rejects discounts above the available net amount', () => {
    expect(() =>
      invoiceTotals([{ quantity: '1000000', price: '100', discount: '101', tax_bps: 0 }], '0'),
    ).toThrow();
    expect(() =>
      invoiceTotals([{ quantity: '1000000', price: '100', discount: '0', tax_bps: 0 }], '101'),
    ).toThrow();
  });
});
