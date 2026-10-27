import type { Payment } from '../api/contracts';
import { s } from '../api/client';
import { t } from '../i18n';
import { money, moneyDigits, businessDate, useStore } from '../app/store';
import { decimal, displayDate, editDecimal, parseDate } from '../lib/numbers';
import { Button, Field, Input, Select } from './ui';
export interface PaymentEdit {
  method: 'cash' | 'bank' | 'cheque';
  account: string;
  shift: string;
  amount: string;
  reference: string;
  number: string;
  bank: string;
  due: string;
}
export function emptyPayment(): PaymentEdit {
  return {
    method: 'cash',
    account:
      useStore
        .getState()
        .bootstrap?.cash_accounts.find((r) => r.kind === 'cash' && r.active === '1')?.id ?? '',
    shift: '',
    amount: '0',
    reference: '',
    number: '',
    bank: '',
    due: displayDate(businessDate()),
  };
}
export function paymentPayload(rows: PaymentEdit[]): Payment[] {
  return rows.map((p) => ({
    method: p.method,
    cash_account_id: p.method === 'cheque' ? null : p.account || null,
    shift_id: p.method === 'cheque' ? null : p.shift || null,
    amount: decimal(p.amount, moneyDigits()),
    reference: p.reference,
    cheque:
      p.method === 'cheque' ? { number: p.number, bank: p.bank, due_date: parseDate(p.due) } : null,
  }));
}
export function Payments({
  value,
  onChange,
  total,
  shifts = [],
}: {
  value: PaymentEdit[];
  onChange: (value: PaymentEdit[]) => void;
  total?: bigint;
  shifts?: { id: string; cash_account_id: string | null }[];
}) {
  const b = useStore((s) => s.bootstrap);
  const set = (i: number, patch: Partial<PaymentEdit>) =>
    onChange(value.map((p, k) => (k === i ? { ...p, ...patch } : p)));
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <h3 className="font-bold text-ink">{t('payments')}</h3>
        <div className="flex gap-2">
          {total !== undefined && (
            <Button
              onClick={() =>
                onChange([{ ...emptyPayment(), amount: editDecimal(total, moneyDigits()) }])
              }
            >
              {t('fullPayment')}
            </Button>
          )}
          <Button
            disabled={value.length >= 20}
            onClick={() => onChange([...value, emptyPayment()])}
          >
            {t('addPayment')}
          </Button>
        </div>
      </div>
      {!value.length && (
        <p className="rounded-lg bg-slate-50 p-3 text-sm text-slate-500">{t('credit')}</p>
      )}
      {value.map((p, i) => (
        <div key={i} className="rounded-xl border border-slate-200 p-3">
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <Field label="pay">
              <Select
                value={p.method}
                onChange={(e) =>
                  set(i, {
                    method: e.target.value as PaymentEdit['method'],
                    account: '',
                    shift: '',
                  })
                }
              >
                {['cash', 'bank', 'cheque'].map((m) => (
                  <option key={m} value={m}>
                    {t(m)}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="amount">
              <Input
                value={p.amount}
                dir="ltr"
                inputMode="decimal"
                onChange={(e) => set(i, { amount: e.target.value })}
              />
            </Field>
            {p.method !== 'cheque' && (
              <>
                <Field label="cashAccount">
                  <Select
                    value={p.account}
                    onChange={(e) => set(i, { account: e.target.value, shift: '' })}
                  >
                    <option value="">{t('select')}</option>
                    {b?.cash_accounts
                      .filter((a) => a.kind === p.method && a.active === '1')
                      .map((a) => (
                        <option key={s(a, 'id')} value={s(a, 'id')}>
                          {s(a, 'name_ar')}
                        </option>
                      ))}
                  </Select>
                </Field>
                <Field label="shift">
                  <Select value={p.shift} onChange={(e) => set(i, { shift: e.target.value })}>
                    <option value="">{t('autoShift')}</option>
                    {shifts
                      .filter((a) => a.cash_account_id === p.account)
                      .map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.id}
                        </option>
                      ))}
                  </Select>
                </Field>
              </>
            )}
            {p.method === 'cheque' && (
              <>
                <Field label="chequeNumber">
                  <Input value={p.number} onChange={(e) => set(i, { number: e.target.value })} />
                </Field>
                <Field label="bankName">
                  <Input value={p.bank} onChange={(e) => set(i, { bank: e.target.value })} />
                </Field>
                <Field label="due_date">
                  <Input
                    dir="ltr"
                    value={p.due}
                    onChange={(e) => set(i, { due: e.target.value })}
                  />
                </Field>
              </>
            )}
            <Field label="reference">
              <Input value={p.reference} onChange={(e) => set(i, { reference: e.target.value })} />
            </Field>
            <div className="flex items-end">
              <Button tone="danger" onClick={() => onChange(value.filter((_, k) => k !== i))}>
                {t('remove')}
              </Button>
            </div>
          </div>
        </div>
      ))}
      {total !== undefined && (
        <p className="text-sm text-slate-500">
          {t('total')}: <bdi>{money(total)}</bdi>
        </p>
      )}
    </div>
  );
}
