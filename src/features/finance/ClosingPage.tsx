import { useState } from 'react';
import { api, s, type ClosingPreview } from '../../api/client';
import { t } from '../../i18n';
import { money, toast, useStore, refreshBootstrap } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { displayDate, parseDate } from '../../lib/numbers';
import { useTask } from '../../lib/hooks';
import { Button, Field, Input, Panel, Select } from '../../components/ui';
import { DataTable } from '../../components/Table';

export default function ClosingPage({ onDocument }: { onDocument: (id: string) => void }) {
  const periods = useStore((s) => s.bootstrap?.fiscal_periods) ?? [];
  const [period, setPeriod] = useState(s(periods[0], 'id'));
  const [date, setDate] = useState(displayDate(s(periods[0], 'ends_on')));
  const [reason, setReason] = useState('');
  const [preview, setPreview] = useState<ClosingPreview>();
  const task = useTask();
  return (
    <Panel title="yearClosing">
      <p className="mb-4 text-sm text-slate-600">{t('yearClosingHint')}</p>
      <div className="form-grid">
        <Field label="fiscalPeriod">
          <Select
            value={period}
            onChange={(e) => {
              setPeriod(e.target.value);
              setDate(
                displayDate(
                  s(
                    periods.find((p) => p.id === e.target.value),
                    'ends_on',
                  ),
                ),
              );
              setPreview(undefined);
            }}
          >
            {periods.map((p) => (
              <option key={p.id} value={s(p, 'id')}>
                {s(p, 'name')}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="date">
          <Input
            dir="ltr"
            value={date}
            onChange={(e) => {
              setDate(e.target.value);
              setPreview(undefined);
            }}
          />
        </Field>
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
        </Field>
      </div>
      <Button
        className="my-4"
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            setPreview(undefined);
            setPreview(
              await api.query({
                type: 'closing_preview',
                period_id: period,
                date: parseDate(date),
              }),
            );
          })
        }
      >
        {t('previewClosing')}
      </Button>
      {preview && (
        <>
          <p className="mb-4 font-bold">
            {t('closingResult')} <bdi>{money(preview.result_minor)}</bdi>
          </p>
          <DataTable
            rows={preview.rows}
            columns={['account_code', 'account_name', 'balance_minor']}
          />
          <Button
            className="mt-4"
            tone="primary"
            busy={task.busy}
            disabled={!reason.trim()}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmYearClosing'))) === null) return;
                const result = await api.mutate({
                  type: 'close_year',
                  period_id: period,
                  date: parseDate(date),
                  preview_hash: preview.preview_hash,
                  reason,
                });
                setPreview(undefined);
                await refreshBootstrap();
                toast(t('done'));
                onDocument(result.id!);
              })
            }
          >
            {t('postYearClosing')}
          </Button>
        </>
      )}
    </Panel>
  );
}
