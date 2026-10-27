import { t } from '../i18n';
export const SCALE = 1_000_000n;
export const MAX_MONEY = 1_000_000_000_000n;
let preferredArabic = false;
export function setNumberStyle(arabic: boolean) {
  preferredArabic = arabic;
}
export function western(value: string): string {
  return value
    .replace(/[٠-٩]/g, (c) => String(c.charCodeAt(0) - 1632))
    .replace(/[۰-۹]/g, (c) => String(c.charCodeAt(0) - 1776))
    .replace(/٫/g, '.')
    .replace(/٬/g, ',');
}
export function decimal(value: string, digits: number, signed = false): string {
  const input = western(value.trim());
  if (!/^-?\d+(?:\.\d+)?$/.test(input)) throw new Error(t('invalidNumber'));
  const negative = input.startsWith('-');
  if (negative && !signed) throw new Error(t('negativeNotAllowed'));
  const [whole, fraction = ''] = input.replace(/^-/, '').split('.');
  if (fraction.length > digits) throw new Error(t('invalidNumber'));
  const n = BigInt(whole) * 10n ** BigInt(digits) + BigInt(fraction.padEnd(digits, '0') || '0');
  if (n > 9_000_000_000_000_000_000n) throw new Error(t('tooLarge'));
  return ((negative ? -1n : 1n) * n).toString();
}
export function editDecimal(value: string | bigint | null | undefined, digits: number): string {
  let n = BigInt(value || '0');
  const negative = n < 0n;
  if (negative) n = -n;
  const s = n.toString().padStart(digits + 1, '0');
  return `${negative ? '-' : ''}${digits ? `${s.slice(0, -digits)}.${s.slice(-digits)}` : s}`;
}
export function localDigits(value: string, arabic = preferredArabic): string {
  return arabic ? value.replace(/\d/g, (d) => '٠١٢٣٤٥٦٧٨٩'[Number(d)]) : value;
}
export function formatDecimal(
  value: string | bigint | null | undefined,
  digits: number,
  arabic = preferredArabic,
  trim = false,
): string {
  let s = editDecimal(value, digits);
  if (trim && digits) s = s.replace(/\.?0+$/, '');
  const [w, f] = s.split('.');
  return localDigits(w.replace(/\B(?=(\d{3})+(?!\d))/g, ',') + (f ? `.${f}` : ''), arabic);
}
export function rounded(n: bigint, denominator: bigint): bigint {
  if (denominator <= 0n) throw new Error(t('invalidNumber'));
  return n < 0n ? -((-n + denominator / 2n) / denominator) : (n + denominator / 2n) / denominator;
}
export function allocate(total: bigint, weights: bigint[]): bigint[] {
  const sum = weights.reduce((a, b) => a + b, 0n);
  if (total < 0n || weights.some((v) => v < 0n) || (sum === 0n && total !== 0n))
    throw new Error(t('invalidDiscount'));
  if (!sum) return weights.map(() => 0n);
  const shares = weights.map((w) => (total * w) / sum);
  const order = weights
    .map((w, i) => ({ i, rem: (total * w) % sum }))
    .sort((a, b) => (a.rem === b.rem ? a.i - b.i : a.rem > b.rem ? -1 : 1));
  let extra = total - shares.reduce((a, b) => a + b, 0n);
  for (const { i } of order) {
    if (!extra) break;
    shares[i]++;
    extra--;
  }
  return shares;
}
export function baseQuantity(q: string, numerator: string, denominator: string): string {
  const n = BigInt(q) * BigInt(numerator),
    d = BigInt(denominator);
  if (d <= 0n || n % d !== 0n) throw new Error(t('badConversion'));
  return (n / d).toString();
}
export function today(timezone = 'Africa/Cairo'): string {
  const p = new Intl.DateTimeFormat('en-CA', {
    timeZone: timezone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).formatToParts(new Date());
  const get = (k: string) => p.find((x) => x.type === k)!.value;
  return `${get('year')}-${get('month')}-${get('day')}`;
}
export function displayDate(iso: string | null | undefined, arabic = preferredArabic): string {
  if (!iso) return '—';
  return localDigits(`${iso.slice(8, 10)}/${iso.slice(5, 7)}/${iso.slice(0, 4)}`, arabic);
}
export function parseDate(value: string): string {
  const s = western(value.trim());
  const m = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(s);
  if (!m) throw new Error(t('invalidDate'));
  const iso = `${m[3]}-${m[2]}-${m[1]}`;
  const d = new Date(iso + 'T12:00:00Z');
  if (!Number.isFinite(d.getTime()) || d.toISOString().slice(0, 10) !== iso)
    throw new Error(t('invalidDate'));
  return iso;
}
