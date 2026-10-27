import type { Settings } from '../../api/contracts';
import { s, type Row } from '../../api/client';
export function settingsInput(row: Row): Settings {
  return {
    revision: s(row, 'revision'),
    store_name: s(row, 'store_name'),
    tax_number: s(row, 'tax_number'),
    address: s(row, 'address'),
    phones: s(row, 'phones'),
    currency_code: s(row, 'currency_code'),
    currency_symbol: s(row, 'currency_symbol'),
    currency_digits: Number(s(row, 'currency_minor_digits')),
    arabic_digits: s(row, 'digit_style') === 'arabic_indic',
    timezone: s(row, 'timezone'),
    vat_enabled: s(row, 'vat_enabled') === '1',
    vat_bps: Number(s(row, 'default_vat_rate_bps')),
    negative_stock: s(row, 'allow_negative_stock') === '1',
    block_credit: s(row, 'credit_limit_policy') === 'block',
    thermal: s(row, 'default_print_layout') === 'thermal80',
    receipt_footer: s(row, 'receipt_footer'),
    print_copies: Number(s(row, 'print_copies')),
    auto_backup: s(row, 'auto_backup') === '1',
  };
}
