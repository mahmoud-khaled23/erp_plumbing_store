import type { DraftLine } from '../../api/contracts';
import { allocate, rounded, SCALE, MAX_MONEY } from '../../lib/numbers';
import { t } from '../../i18n';
export function invoiceTotals(
  lines: Pick<DraftLine, 'quantity' | 'price' | 'discount' | 'tax_bps'>[],
  discount: string,
) {
  const gross = lines.map((l) => rounded(BigInt(l.quantity) * BigInt(l.price), SCALE));
  const net = gross.map((g, i) => g - BigInt(lines[i].discount));
  if (
    gross.some((v) => v < 0n || v > MAX_MONEY) ||
    net.some((v) => v < 0n) ||
    lines.some((l) => BigInt(l.discount) < 0n)
  )
    throw new Error(t('invalidDiscount'));
  const sum = net.reduce((a, b) => a + b, 0n);
  const disc = BigInt(discount);
  if (disc < 0n || disc > sum) throw new Error(t('invalidDiscount'));
  const shares = allocate(disc, net);
  const tax = net.map((v, i) => rounded((v - shares[i]) * BigInt(lines[i].tax_bps), 10000n));
  const totals = net.map((v, i) => v - shares[i] + tax[i]);
  return {
    gross: gross.reduce((a, b) => a + b, 0n),
    lineDiscount: lines.reduce((a, l) => a + BigInt(l.discount), 0n),
    discount: disc,
    tax: tax.reduce((a, b) => a + b, 0n),
    total: totals.reduce((a, b) => a + b, 0n),
    totals,
  };
}
