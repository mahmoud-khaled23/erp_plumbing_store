import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import type { BankStatementLine, ReconciliationView } from '../../api/contracts';
import { businessDate, money, moneyDigits, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { decimal, displayDate, editDecimal, parseDate } from '../../lib/numbers';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, ErrorState, Field, Input, Loading, Modal, Panel, Tabs } from '../../components/ui';
import { MasterSelect } from '../../components/Pickers';
import { DataTable, Pager } from '../../components/Table';

function StatementForm({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const [account, setAccount] = useState('2'),
    [reference, setReference] = useState(''),
    [from, setFrom] = useState(displayDate(businessDate())),
    [to, setTo] = useState(displayDate(businessDate())),
    [opening, setOpening] = useState('0'),
    [closing, setClosing] = useState('0');
  const [lines, setLines] = useState<BankStatementLine[]>([]),
    [date, setDate] = useState(displayDate(businessDate())),
    [amount, setAmount] = useState('0'),
    [lineReference, setLineReference] = useState(''),
    [description, setDescription] = useState(''),
    [page, setPage] = useState(0);
  const task = useTask();
  const rows = lines.map((l, i) => ({
    id: String(i),
    value_date: l.date,
    amount_minor: l.amount,
    reference: l.reference,
    description: l.description,
  }));
  return (
    <Modal title="newBankStatement" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm">{t('bankCsvHint')}</p>
      <div className="form-grid">
        <MasterSelect
          table="cash_accounts"
          label="cashAccount"
          value={account}
          onChange={setAccount}
        />
        <Field label="reference">
          <Input value={reference} onChange={(e) => setReference(e.target.value)} maxLength={200} />
        </Field>
        <Field label="from">
          <Input dir="ltr" value={from} onChange={(e) => setFrom(e.target.value)} />
        </Field>
        <Field label="to">
          <Input dir="ltr" value={to} onChange={(e) => setTo(e.target.value)} />
        </Field>
        <Field label="openingBalance">
          <Input dir="ltr" value={opening} onChange={(e) => setOpening(e.target.value)} />
        </Field>
        <Field label="statementClosing">
          <Input dir="ltr" value={closing} onChange={(e) => setClosing(e.target.value)} />
        </Field>
        <Field label="importStatementCsv">
          <Input
            type="file"
            accept=".csv,text/csv"
            onChange={(e) => {
              const file = e.target.files?.[0];
              e.target.value = '';
              if (file)
                void task.run(async () => {
                  if (file.size > 2_000_000) throw new Error(t('statementFileTooLarge'));
                  const parsed = await api.query({
                    type: 'parse_statement_csv',
                    csv: await file.text(),
                  });
                  setLines(
                    parsed.map((l) => ({
                      date: s(l, 'date'),
                      amount: s(l, 'amount'),
                      reference: s(l, 'reference'),
                      description: s(l, 'description'),
                    })),
                  );
                  setPage(0);
                });
            }}
          />
        </Field>
      </div>
      <div className="my-4 grid gap-3 md:grid-cols-4">
        <Field label="date">
          <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
        </Field>
        <Field label="signedStatementAmount">
          <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
        </Field>
        <Field label="reference">
          <Input
            value={lineReference}
            onChange={(e) => setLineReference(e.target.value)}
            maxLength={200}
          />
        </Field>
        <Field label="description">
          <Input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            maxLength={1000}
          />
        </Field>
      </div>
      <Button
        busy={task.busy}
        disabled={lines.length >= 5000}
        onClick={() =>
          task.run(async () => {
            const minor = decimal(amount, moneyDigits());
            if (BigInt(minor) === 0n) throw new Error(t('invalidNumber'));
            setLines((l) => [
              ...l,
              { date: parseDate(date), amount: minor, reference: lineReference, description },
            ]);
          })
        }
      >
        {t('addLine')}
      </Button>
      <DataTable
        rows={rows.slice(page * 50, (page + 1) * 50)}
        columns={['value_date', 'amount_minor', 'reference', 'description']}
        actions={(r) => (
          <Button
            tone="danger"
            onClick={() => {
              setLines((l) => l.filter((_, i) => String(i) !== r.id));
              setPage(0);
            }}
          >
            {t('remove')}
          </Button>
        )}
      />
      <Pager
        page={page}
        canNext={(page + 1) * 50 < lines.length}
        onPrevious={() => setPage((p) => p - 1)}
        onNext={() => setPage((p) => p + 1)}
      />
      <Button
        className="mt-4"
        tone="primary"
        busy={task.busy}
        disabled={!lines.length || !reference.trim()}
        onClick={() =>
          task.run(async () => {
            if ((await confirmAction(t('confirmStatement'))) === null) return;
            const result = await api.mutate({
              type: 'save_bank_statement',
              statement: {
                cash_account_id: account,
                reference,
                starts_on: parseDate(from),
                ends_on: parseDate(to),
                opening: decimal(opening, moneyDigits()),
                closing: decimal(closing, moneyDigits()),
                lines,
              },
            });
            useStore.getState().refresh();
            toast(t('done'));
            onSaved(result.id!);
          })
        }
      >
        {t('save')}
      </Button>
    </Modal>
  );
}

export default function ReconciliationPage({ onDocument }: { onDocument: (id: string) => void }) {
  const [selected, setSelected] = useState<Row>(),
    [creating, setCreating] = useState(false),
    [cursors, setCursors] = useState(['0']),
    [view, setView] = useState<ReconciliationView>('statement'),
    [page, setPage] = useState(0);
  const [line, setLine] = useState<Row>(),
    [journal, setJournal] = useState<Row>(),
    [amount, setAmount] = useState('0'),
    [action, setAction] = useState<{ type: 'match' | 'unmatch' | 'void'; id: string }>(),
    [reason, setReason] = useState(''),
    [date, setDate] = useState(displayDate(businessDate()));
  const task = useTask(),
    list = useQuery(
      !selected ? { type: 'bank_statements', after_id: cursors.at(-1)!, limit: 50 } : null,
    ),
    data = useQuery(
      selected
        ? {
            type: 'reconciliation',
            statement_id: s(selected, 'id'),
            view,
            limit: 50,
            offset: page * 50,
          }
        : null,
    );
  const start = (type: 'match' | 'unmatch' | 'void', id: string) => {
    setReason('');
    setDate(displayDate(businessDate()));
    setAction({ type, id });
  };
  return (
    <Panel title="bankReconciliation">
      <p className="mb-4 text-sm text-slate-600">{t('bankReconciliationHint')}</p>
      <div className="mb-4 flex flex-wrap gap-3">
        <Button onClick={() => setCreating(true)}>{t('newBankStatement')}</Button>
        {selected && (
          <Button
            onClick={() => {
              setSelected(undefined);
              setLine(undefined);
              setJournal(undefined);
              setPage(0);
            }}
          >
            {t('back')}
          </Button>
        )}
        {selected?.status === 'active' && (
          <Button tone="danger" onClick={() => start('void', s(selected, 'id'))}>
            {t('voidStatement')}
          </Button>
        )}
      </div>
      {!selected ? (
        <>
          {list.error ? (
            <ErrorState error={list.error} />
          ) : list.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={list.data ?? []}
              columns={[
                'reference',
                'name_ar',
                'starts_on',
                'ends_on',
                'opening_minor',
                'closing_minor',
                'status',
              ]}
              onRow={(r) => {
                setSelected(r);
                setView('statement');
                setPage(0);
              }}
            />
          )}
          <Pager
            page={cursors.length - 1}
            canNext={list.data?.length === 50}
            onPrevious={() => setCursors((c) => c.slice(0, -1))}
            onNext={() => setCursors((c) => [...c, s(list.data?.at(-1), 'id')])}
          />
        </>
      ) : (
        <>
          <h3 className="mb-3 font-bold">
            {s(selected, 'reference')} · {s(selected, 'name_ar')}
          </h3>
          <div className="mb-4 grid gap-3 md:grid-cols-3">
            {data.data?.summary.map((r) => (
              <div
                key={r.metric}
                className={`rounded-lg border p-3 ${r.metric === 'bankReconciliationDifference' && r.amount_minor !== '0' ? 'border-red-300 bg-red-50' : ''}`}
              >
                <p>{t(s(r, 'metric'))}</p>
                <bdi className="font-bold">{money(s(r, 'amount_minor'))}</bdi>
              </div>
            ))}
          </div>
          {line && (
            <p className="my-3">
              {t('selectedStatementLine')} {s(line, 'reference')} ·{' '}
              <bdi>{money(s(line, 'amount_minor'))}</bdi>{' '}
              <Button
                onClick={() => {
                  setLine(undefined);
                  setJournal(undefined);
                }}
              >
                {t('clearSelection')}
              </Button>
            </p>
          )}
          <Tabs
            value={view}
            onChange={(v) => {
              setView(v as ReconciliationView);
              setPage(0);
            }}
            items={['statement', 'journal', 'matches'].map((v) => ({
              value: v,
              label: `reconciliationTabs.${v}`,
            }))}
          />
          {data.error ? (
            <ErrorState error={data.error} />
          ) : data.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={data.data?.rows ?? []}
              columns={
                view === 'statement'
                  ? [
                      'line_number',
                      'value_date',
                      'amount_minor',
                      'remaining_minor',
                      'reference',
                      'description',
                    ]
                  : view === 'journal'
                    ? [
                        'document_number',
                        'document_date',
                        'amount_minor',
                        'remaining_minor',
                        'memo',
                      ]
                    : [
                        'document_number',
                        'document_date',
                        'line_number',
                        'amount_minor',
                        'status',
                        'reason',
                      ]
              }
              actions={(r) => (
                <>
                  {view !== 'statement' && (
                    <Button
                      onClick={() =>
                        onDocument(s(r, view === 'matches' ? 'source_document_id' : 'document_id'))
                      }
                    >
                      {t('viewDocument')}
                    </Button>
                  )}
                  {selected.status === 'active' &&
                    (view === 'statement' && BigInt(s(r, 'remaining_minor', '0')) > 0n ? (
                      <Button
                        onClick={() => {
                          setLine(r);
                          setView('journal');
                          setPage(0);
                        }}
                      >
                        {t('selectForMatch')}
                      </Button>
                    ) : view === 'journal' &&
                      line &&
                      BigInt(s(r, 'amount_minor')) > 0n === BigInt(s(line, 'amount_minor')) > 0n ? (
                      <Button
                        onClick={() => {
                          setJournal(r);
                          const a = BigInt(s(line, 'remaining_minor')),
                            b = BigInt(s(r, 'remaining_minor'));
                          setAmount(editDecimal((a < b ? a : b).toString(), moneyDigits()));
                          start('match', s(line, 'id'));
                        }}
                      >
                        {t('matchStatement')}
                      </Button>
                    ) : view === 'matches' && r.status === 'active' ? (
                      <Button tone="danger" onClick={() => start('unmatch', s(r, 'id'))}>
                        {t('unmatchStatement')}
                      </Button>
                    ) : null)}
                </>
              )}
            />
          )}
          <Pager
            page={page}
            canNext={data.data?.rows.length === 50}
            onPrevious={() => setPage((p) => p - 1)}
            onNext={() => setPage((p) => p + 1)}
          />
        </>
      )}
      {creating && (
        <StatementForm
          onClose={() => setCreating(false)}
          onSaved={() => {
            setCreating(false);
            setSelected(undefined);
            setCursors(['0']);
          }}
        />
      )}
      {action && (
        <Modal
          title={
            action.type === 'match'
              ? 'matchStatement'
              : action.type === 'unmatch'
                ? 'unmatchStatement'
                : 'voidStatement'
          }
          onClose={() => setAction(undefined)}
          busy={task.busy}
        >
          <div className="form-grid">
            <Field label="date">
              <Input dir="ltr" value={date} onChange={(e) => setDate(e.target.value)} />
            </Field>
            {action.type === 'match' && (
              <Field label="amount">
                <Input dir="ltr" value={amount} onChange={(e) => setAmount(e.target.value)} />
              </Field>
            )}
            <Field label="reason">
              <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
            </Field>
          </div>
          <Button
            className="mt-4"
            tone="primary"
            busy={task.busy}
            disabled={!reason.trim()}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmPost'))) === null) return;
                const common = { date: parseDate(date), reason };
                if (action.type === 'match')
                  await api.mutate({
                    type: 'match_statement',
                    ...common,
                    statement_line_id: action.id,
                    journal_line_id: s(journal, 'id'),
                    amount: decimal(amount, moneyDigits()),
                  });
                else if (action.type === 'unmatch')
                  await api.mutate({ type: 'unmatch_statement', ...common, match_id: action.id });
                else {
                  await api.mutate({
                    type: 'void_bank_statement',
                    ...common,
                    statement_id: action.id,
                  });
                  setSelected(undefined);
                }
                setAction(undefined);
                setLine(undefined);
                setJournal(undefined);
                useStore.getState().refresh();
                toast(t('done'));
              })
            }
          >
            {t('confirm')}
          </Button>
        </Modal>
      )}
    </Panel>
  );
}
