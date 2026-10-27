import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { decimal, displayDate, parseDate } from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import { Button, Check, Field, Input, Modal, Select } from '../../components/ui';
import { MasterSelect, PartyPicker } from '../../components/Pickers';
import { Payments, paymentPayload, type PaymentEdit } from '../../components/Payments';
import { DataTable } from '../../components/Table';
import { ReportView } from '../reports/ReportsPage';
export type FinanceForm = 'voucher' | 'expense' | 'journal' | 'cashTransfer' | 'settle';
interface JournalEdit {
  account: string;
  party: string | null;
  partyName: string;
  due: string;
  debit: string;
  credit: string;
  memo: string;
}
interface ExpenseEdit {
  category: string;
  amount: string;
  tax: string;
  description: string;
}
export function FinanceDialog({
  kind,
  onClose,
  onDone,
}: {
  kind: FinanceForm;
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const b = useStore((s) => s.bootstrap)!;
  const [date, setDate] = useState(displayDate(businessDate())),
    [party, setParty] = useState<string | null>(null),
    [partyName, setPartyName] = useState(''),
    [supplier, setSupplier] = useState(false),
    [outgoing, setOutgoing] = useState(false),
    [opening, setOpening] = useState(false),
    [notes, setNotes] = useState(''),
    [payments, setPayments] = useState<PaymentEdit[]>([]),
    [from, setFrom] = useState(''),
    [to, setTo] = useState(''),
    [amount, setAmount] = useState('0'),
    [debitItem, setDebitItem] = useState<Row>(),
    [creditItem, setCreditItem] = useState<Row>();
  const [journal, setJournal] = useState<JournalEdit[]>([
      {
        account: '',
        party: null,
        partyName: '',
        due: displayDate(businessDate()),
        debit: '0',
        credit: '0',
        memo: '',
      },
      {
        account: '',
        party: null,
        partyName: '',
        due: displayDate(businessDate()),
        debit: '0',
        credit: '0',
        memo: '',
      },
    ]),
    [expenses, setExpenses] = useState<ExpenseEdit[]>([
      { category: s(b.expense_categories[0], 'id'), amount: '0', tax: '0', description: '' },
    ]);
  const task = useTask();
  const updateJournal = (i: number, p: Partial<JournalEdit>) =>
    setJournal((v) => v.map((r, k) => (i === k ? { ...r, ...p } : r)));
  const updateExpense = (i: number, p: Partial<ExpenseEdit>) =>
    setExpenses((v) => v.map((r, k) => (i === k ? { ...r, ...p } : r)));
  let balance: bigint | null = null;
  try {
    balance = journal.reduce(
      (a, l) =>
        a + BigInt(decimal(l.debit, moneyDigits())) - BigInt(decimal(l.credit, moneyDigits())),
      0n,
    );
  } catch {}
  const submit = () =>
    task.run(async () => {
      if ((await confirmAction(t('confirmPost'))) === null) return;
      const day = parseDate(date);
      let result;
      if (kind === 'voucher') {
        if (!party) throw new Error(t('required'));
        result = await api.mutate({
          type: 'voucher',
          date: day,
          party_id: party,
          supplier,
          outgoing,
          payments: paymentPayload(payments),
          notes,
        });
      } else if (kind === 'expense')
        result = await api.mutate({
          type: 'expense',
          date: day,
          party_id: party,
          lines: expenses.map((l) => ({
            category_id: l.category,
            amount: decimal(l.amount, moneyDigits()),
            tax_bps: Number(decimal(l.tax, 2)),
            description: l.description,
          })),
          payments: paymentPayload(payments),
          notes,
        });
      else if (kind === 'journal') {
        const lines = journal.map((l) => {
          const debit = BigInt(decimal(l.debit, moneyDigits())),
            credit = BigInt(decimal(l.credit, moneyDigits()));
          if (debit > 0n && credit > 0n) throw new Error(t('invalid'));
          return {
            account_id: l.account,
            party_id: l.party,
            due_date: l.party ? parseDate(l.due) : null,
            amount: (debit - credit).toString(),
            memo: l.memo,
          };
        });
        result = await api.mutate({ type: 'journal', date: day, opening, lines, notes });
      } else if (kind === 'cashTransfer')
        result = await api.mutate({
          type: 'cash_transfer',
          date: day,
          from_id: from,
          to_id: to,
          amount: decimal(amount, moneyDigits()),
          notes,
        });
      else {
        if (!debitItem || !creditItem) throw new Error(t('required'));
        result = await api.mutate({
          type: 'settle',
          date: day,
          debit_line_id: s(debitItem, 'journal_line_id'),
          credit_line_id: s(creditItem, 'journal_line_id'),
          amount: decimal(amount, moneyDigits()),
        });
      }
      toast(t('postedSuccess'));
      useStore.getState().refresh();
      onDone(result.id!);
    });
  return (
    <Modal title={kind} onClose={onClose} wide busy={task.busy}>
      <div className="form-grid mb-5">
        <Field label="date">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        {(kind === 'voucher' || kind === 'expense' || kind === 'settle') && (
          <PartyPicker
            value={party}
            name={partyName}
            supplier={kind === 'expense' || supplier}
            optional={kind === 'expense'}
            onChange={(id, r) => {
              setParty(id);
              setPartyName(s(r, 'name_ar'));
              setDebitItem(undefined);
              setCreditItem(undefined);
            }}
          />
        )}
        {kind === 'voucher' && (
          <>
            <Check
              label="supplierAccount"
              checked={supplier}
              onChange={(e) => {
                setSupplier(e.target.checked);
                setParty(null);
              }}
            />
            <Check
              label="outgoing"
              checked={outgoing}
              onChange={(e) => setOutgoing(e.target.checked)}
            />
          </>
        )}
        {kind === 'settle' && (
          <Check
            label="supplierAccount"
            checked={supplier}
            onChange={(e) => {
              setSupplier(e.target.checked);
              setParty(null);
            }}
          />
        )}
        {kind === 'journal' && (
          <Check
            label="openingBalance"
            checked={opening}
            onChange={(e) => setOpening(e.target.checked)}
          />
        )}
        <Field label="notes">
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} />
        </Field>
      </div>
      {kind === 'journal' && (
        <>
          <div className="space-y-3">
            {journal.map((l, i) => (
              <div key={i} className="rounded-xl border border-slate-200 p-3">
                <div className="grid gap-3 md:grid-cols-4">
                  <MasterSelect
                    table="accounts"
                    label="account"
                    value={l.account}
                    onChange={(account) =>
                      updateJournal(i, { account, party: null, partyName: '' })
                    }
                    filter={(r) => r.is_postable === '1' && r.control_kind !== 'inventory'}
                  />
                  <Field label="debit">
                    <Input
                      dir="ltr"
                      value={l.debit}
                      onChange={(e) => updateJournal(i, { debit: e.target.value })}
                    />
                  </Field>
                  <Field label="creditAmount">
                    <Input
                      dir="ltr"
                      value={l.credit}
                      onChange={(e) => updateJournal(i, { credit: e.target.value })}
                    />
                  </Field>
                  <Field label="description">
                    <Input
                      value={l.memo}
                      onChange={(e) => updateJournal(i, { memo: e.target.value })}
                    />
                  </Field>
                  {['receivable', 'payable'].includes(
                    s(
                      b.accounts.find((a) => a.id === l.account),
                      'control_kind',
                    ),
                  ) && (
                    <>
                      <PartyPicker
                        value={l.party}
                        name={l.partyName}
                        supplier={
                          b.accounts.find((a) => a.id === l.account)?.control_kind === 'payable'
                        }
                        optional={false}
                        onChange={(party, r) =>
                          updateJournal(i, { party, partyName: s(r, 'name_ar') })
                        }
                      />
                      <Field label="due_date">
                        <Input
                          dir="ltr"
                          value={l.due}
                          onChange={(e) => updateJournal(i, { due: e.target.value })}
                        />
                      </Field>
                    </>
                  )}
                  <Button
                    tone="danger"
                    disabled={journal.length <= 2}
                    onClick={() => setJournal((v) => v.filter((_, k) => i !== k))}
                  >
                    {t('remove')}
                  </Button>
                </div>
              </div>
            ))}
          </div>
          <div className="mt-3 flex items-center justify-between">
            <Button
              onClick={() =>
                setJournal((v) => [
                  ...v,
                  {
                    account: '',
                    party: null,
                    partyName: '',
                    due: date,
                    debit: '0',
                    credit: '0',
                    memo: '',
                  },
                ])
              }
            >
              {t('add')}
            </Button>
            <span className={balance === 0n ? 'text-teal-700' : 'text-red-700'}>
              {t(balance === 0n ? 'balanced' : 'unbalanced')}:{' '}
              <bdi>{balance === null ? '—' : money(balance)}</bdi>
            </span>
          </div>
        </>
      )}
      {kind === 'expense' && (
        <>
          <div className="space-y-3">
            {expenses.map((l, i) => (
              <div
                key={i}
                className="grid gap-3 rounded-xl border border-slate-200 p-3 md:grid-cols-4"
              >
                <MasterSelect
                  table="expense_categories"
                  label="expenseCategory"
                  value={l.category}
                  onChange={(category) => updateExpense(i, { category })}
                />
                <Field label="amount">
                  <Input
                    dir="ltr"
                    value={l.amount}
                    onChange={(e) => updateExpense(i, { amount: e.target.value })}
                  />
                </Field>
                <Field label="vatPercent">
                  <Input
                    dir="ltr"
                    value={l.tax}
                    onChange={(e) => updateExpense(i, { tax: e.target.value })}
                  />
                </Field>
                <Field label="description">
                  <Input
                    value={l.description}
                    onChange={(e) => updateExpense(i, { description: e.target.value })}
                  />
                </Field>
                <Button
                  tone="danger"
                  disabled={expenses.length === 1}
                  onClick={() => setExpenses((v) => v.filter((_, k) => i !== k))}
                >
                  {t('remove')}
                </Button>
              </div>
            ))}
          </div>
          <Button
            className="my-4"
            onClick={() =>
              setExpenses((v) => [...v, { category: '', amount: '0', tax: '0', description: '' }])
            }
          >
            {t('addExpense')}
          </Button>
        </>
      )}
      {(kind === 'voucher' || kind === 'expense') && (
        <Payments value={payments} onChange={setPayments} />
      )}{' '}
      {kind === 'cashTransfer' && (
        <div className="form-grid">
          <MasterSelect table="cash_accounts" label="fromAccount" value={from} onChange={setFrom} />
          <MasterSelect table="cash_accounts" label="toAccount" value={to} onChange={setTo} />
          <Field label="amount">
            <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
          </Field>
        </div>
      )}
      {kind === 'settle' && (
        <>
          {party && (
            <ReportView
              key={party}
              initial="open_items"
              partyId={party}
              partyName={partyName}
              actions={(r) => (
                <Button
                  onClick={() => {
                    if (BigInt(s(r, 'outstanding_signed_minor', '0')) > 0n) setDebitItem(r);
                    else setCreditItem(r);
                  }}
                >
                  {t(
                    BigInt(s(r, 'outstanding_signed_minor', '0')) > 0n ? 'debitItem' : 'creditItem',
                  )}
                </Button>
              )}
            />
          )}
          <div className="form-grid mt-4">
            <Field label="debitItem">
              <Input readOnly value={s(debitItem, 'document_number')} />
            </Field>
            <Field label="creditItem">
              <Input readOnly value={s(creditItem, 'document_number')} />
            </Field>
            <Field label="amount">
              <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
            </Field>
          </div>
        </>
      )}
      <div className="mt-6 flex justify-end">
        <Button
          tone="primary"
          busy={task.busy}
          disabled={kind === 'journal' && balance !== 0n}
          onClick={() => void submit()}
        >
          {t('post')}
        </Button>
      </div>
    </Modal>
  );
}
