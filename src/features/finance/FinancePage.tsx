import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { allowed, businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { decimal, displayDate, parseDate } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  ErrorState,
  Field,
  Input,
  Loading,
  Modal,
  PageHeader,
  Panel,
  Select,
  Tabs,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { MasterSelect } from '../../components/Pickers';
import { ReportView } from '../reports/ReportsPage';
import { DocumentDetail } from '../documents/DocumentDetail';
import { FinanceDialog, type FinanceForm } from './FinanceForms';
export default function FinancePage() {
  const [tab, setTab] = useState('cashShifts'),
    [form, setForm] = useState<FinanceForm | null>(null),
    [selected, setSelected] = useState<string | null>(null),
    [cheque, setCheque] = useState<{ row: Row; cleared: boolean } | null>(null),
    [bank, setBank] = useState(''),
    [reason, setReason] = useState(''),
    [date, setDate] = useState(displayDate(businessDate())),
    [shift, setShift] = useState<Row | 'open' | null>(null),
    [cashAccount, setCashAccount] = useState('1'),
    [counted, setCounted] = useState('0'),
    [docKind, setDocKind] = useState('receipt_voucher'),
    [cursors, setCursors] = useState(['0']);
  const desk = useQuery(allowed('cash.shift') ? { type: 'cash_desk' } : null),
    detail = useQuery(selected ? { type: 'document', id: selected } : null),
    documents = useQuery(
      tab === 'documents' && allowed('finance.read')
        ? { type: 'documents', kind: docKind, status: null, after_id: cursors.at(-1)!, limit: 50 }
        : null,
    );
  const task = useTask();
  const onDone = (id: string) => {
    setForm(null);
    if (allowed('finance.read')) setSelected(id);
  };
  return (
    <>
      <PageHeader title="finance" hint="financeHint">
        {allowed('payments.write') && (
          <Button tone="primary" onClick={() => setForm('voucher')}>
            {t('voucher')}
          </Button>
        )}
        {allowed('finance.write') && (
          <>
            <Button onClick={() => setForm('expense')}>{t('expense')}</Button>
            <Button onClick={() => setForm('journal')}>{t('journal')}</Button>
            <Button onClick={() => setForm('cashTransfer')}>{t('cashTransfer')}</Button>
          </>
        )}
        {allowed('payments.write') && allowed('reports.read') && (
          <Button onClick={() => setForm('settle')}>{t('settle')}</Button>
        )}
      </PageHeader>
      {selected ? (
        detail.error ? (
          <ErrorState error={detail.error} />
        ) : detail.data ? (
          <DocumentDetail
            detail={detail.data}
            onChanged={setSelected}
            onBack={() => setSelected(null)}
          />
        ) : (
          <Loading />
        )
      ) : (
        <>
          <Tabs
            value={tab}
            onChange={setTab}
            items={[
              ...(allowed('cash.shift') ? [{ value: 'cashShifts', label: 'cashShifts' }] : []),
              ...(allowed('reports.read')
                ? [
                    { value: 'cheques', label: 'cheques' },
                    { value: 'cashFlow', label: 'cashFlow' },
                  ]
                : []),
              ...(allowed('finance.read') ? [{ value: 'documents', label: 'journalLines' }] : []),
            ]}
          />
          {tab === 'cashShifts' && (
            <>
              <div className="mb-5 grid gap-4 md:grid-cols-3">
                {desk.data?.balances.map((r) => (
                  <Panel key={s(r, 'cash_account_id')}>
                    <p className="text-sm text-slate-500">{s(r, 'name_ar')}</p>
                    <p className="mt-2 text-2xl font-bold text-accent">
                      <bdi>{money(s(r, 'balance_minor'))}</bdi>
                    </p>
                  </Panel>
                ))}
              </div>
              <Panel title="cashShifts">
                {allowed('cash.shift') && (
                  <Button className="mb-4" tone="primary" onClick={() => setShift('open')}>
                    {t('openShift')}
                  </Button>
                )}
                {desk.error ? (
                  <ErrorState error={desk.error} />
                ) : desk.loading ? (
                  <Loading />
                ) : (
                  <DataTable
                    rows={desk.data?.shifts ?? []}
                    columns={[
                      'id',
                      'cash_account_id',
                      'opened_at',
                      'opening_balance_minor',
                      'opened_by',
                    ]}
                    actions={(r) => (
                      <Button
                        onClick={() => {
                          setShift(r);
                          setReason('');
                          setCounted('0');
                        }}
                      >
                        {t('closeShift')}
                      </Button>
                    )}
                  />
                )}
              </Panel>
            </>
          )}
          {tab === 'cheques' && (
            <Panel>
              <ReportView
                initial="cheques"
                actions={(r) =>
                  allowed('cheques.write') ? (
                    <>
                      {r.status === 'pending' && (
                        <Button
                          onClick={() => {
                            setCheque({ row: r, cleared: true });
                            setReason('');
                          }}
                        >
                          {t('clearCheque')}
                        </Button>
                      )}
                      {r.status !== 'bounced' && (
                        <Button
                          tone="danger"
                          onClick={() => {
                            setCheque({ row: r, cleared: false });
                            setReason('');
                          }}
                        >
                          {t('bounceCheque')}
                        </Button>
                      )}
                    </>
                  ) : null
                }
              />
            </Panel>
          )}
          {tab === 'cashFlow' && (
            <Panel>
              <ReportView initial="cash_flow" />
            </Panel>
          )}
          {tab === 'documents' && (
            <Panel>
              <div className="mb-4 max-w-xs">
                <Select
                  aria-label={t('kind')}
                  value={docKind}
                  onChange={(e) => {
                    setDocKind(e.target.value);
                    setCursors(['0']);
                  }}
                >
                  {[
                    'receipt_voucher',
                    'payment_voucher',
                    'expense',
                    'manual_journal',
                    'opening_balance',
                    'cash_transfer',
                    'settlement',
                    'cheque_clear',
                    'cheque_bounce',
                    'cash_close',
                    'cost_adjustment',
                    'reversal',
                  ].map((v) => (
                    <option value={v} key={v}>
                      {t(`enums.${v}`)}
                    </option>
                  ))}
                </Select>
              </div>
              {documents.error ? (
                <ErrorState error={documents.error} />
              ) : documents.loading ? (
                <Loading />
              ) : (
                <DataTable
                  rows={documents.data ?? []}
                  columns={[
                    'number',
                    'document_date',
                    'party_name_snapshot',
                    'total_minor',
                    'status',
                    'notes',
                  ]}
                  onRow={(r) => setSelected(s(r, 'id'))}
                />
              )}
              <Pager
                page={cursors.length - 1}
                canNext={documents.data?.length === 50}
                onPrevious={() => setCursors((c) => c.slice(0, -1))}
                onNext={() => setCursors((c) => [...c, s(documents.data?.at(-1), 'id')])}
              />
            </Panel>
          )}
        </>
      )}
      {form && <FinanceDialog kind={form} onClose={() => setForm(null)} onDone={onDone} />}{' '}
      {cheque && (
        <Modal
          title={cheque.cleared ? 'clearCheque' : 'bounceCheque'}
          onClose={() => setCheque(null)}
          busy={task.busy}
        >
          <p className="mb-4 font-bold">
            <bdi>
              {s(cheque.row, 'cheque_number')} · {money(s(cheque.row, 'amount_minor'))}
            </bdi>
          </p>
          <div className="form-grid">
            <Field label="date">
              <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
            </Field>
            {cheque.cleared && (
              <MasterSelect
                table="cash_accounts"
                label="bank"
                value={bank}
                onChange={setBank}
                filter={(r) => r.kind === 'bank'}
              />
            )}
            <Field label="reason">
              <Input value={reason} onChange={(e) => setReason(e.target.value)} />
            </Field>
          </div>
          <Button
            className="mt-5"
            tone="primary"
            busy={task.busy}
            onClick={() =>
              task.run(async () => {
                await api.mutate({
                  type: 'cheque_event',
                  date: parseDate(date),
                  cheque_id: s(cheque.row, 'id'),
                  cleared: cheque.cleared,
                  bank_account_id: cheque.cleared ? bank : null,
                  reason,
                });
                toast(t('done'));
                setCheque(null);
                useStore.getState().refresh();
              })
            }
          >
            {t('confirm')}
          </Button>
        </Modal>
      )}
      {shift && (
        <Modal
          title={shift === 'open' ? 'openShift' : 'closeShift'}
          onClose={() => setShift(null)}
          busy={task.busy}
        >
          {shift === 'open' ? (
            <MasterSelect
              table="cash_accounts"
              label="cashAccount"
              value={cashAccount}
              onChange={setCashAccount}
              filter={(r) => r.kind === 'cash'}
            />
          ) : (
            <div className="form-grid">
              <Field label="countedCash">
                <Input dir="ltr" value={counted} onChange={(e) => setCounted(e.target.value)} />
              </Field>
              <Field label="date">
                <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
              </Field>
              <Field label="reason">
                <Input value={reason} onChange={(e) => setReason(e.target.value)} />
              </Field>
            </div>
          )}
          <Button
            className="mt-5"
            tone="primary"
            busy={task.busy}
            onClick={() =>
              task.run(async () => {
                if (shift === 'open')
                  await api.mutate({ type: 'open_shift', cash_account_id: cashAccount });
                else {
                  if ((await confirmAction(t('confirmPost'))) === null) return;
                  await api.mutate({
                    type: 'close_shift',
                    id: s(shift, 'id'),
                    date: parseDate(date),
                    counted: decimal(counted, moneyDigits()),
                    reason,
                  });
                }
                toast(t('done'));
                setShift(null);
                useStore.getState().refresh();
              })
            }
          >
            {t('confirm')}
          </Button>
        </Modal>
      )}
    </>
  );
}
