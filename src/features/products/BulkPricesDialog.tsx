import { useEffect, useState } from 'react';
import { api, s, type CalculationPreview } from '../../api/client';
import type { PriceBasis, PriceList } from '../../api/contracts';
import { allowed, toast, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { t } from '../../i18n';
import { decimal, editDecimal } from '../../lib/numbers';
import { useDebounced, useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  Check,
  ErrorState,
  Field,
  Input,
  Loading,
  Modal,
  Panel,
  Select,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';

export default function BulkPricesDialog({ onClose }: { onClose: () => void }) {
  const [search, setSearch] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [selected, setSelected] = useState<string[]>([]),
    [basis, setBasis] = useState<PriceBasis>('percentage'),
    [list, setList] = useState<PriceList>('retail'),
    [rate, setRate] = useState('0'),
    [reason, setReason] = useState(''),
    [preview, setPreview] = useState<CalculationPreview>(),
    [minimum, setMinimum] = useState('0'),
    [policyReason, setPolicyReason] = useState('');
  const task = useTask(),
    query = useQuery({
      type: 'price_units',
      search: useDebounced(search),
      after_id: cursors.at(-1)!,
      limit: 50,
    }),
    policy = useQuery({ type: 'pricing_policy' });
  useEffect(() => {
    if (policy.data?.[0]) setMinimum(editDecimal(s(policy.data[0], 'minimum_margin_bps'), 2));
  }, [policy.data]);
  return (
    <Modal title="bulkPrices" onClose={onClose} wide busy={task.busy}>
      <p className="mb-4 text-sm text-slate-600">{t('bulkPricesHint')}</p>
      {allowed('pricing.manage') && (
        <Panel title="marginPolicy">
          {policy.error ? (
            <ErrorState error={policy.error} />
          ) : (
            <div className="form-grid">
              <Field label="minimumMargin">
                <Input dir="ltr" value={minimum} onChange={(e) => setMinimum(e.target.value)} />
              </Field>
              <Field label="reason">
                <Input
                  value={policyReason}
                  onChange={(e) => setPolicyReason(e.target.value)}
                  maxLength={2000}
                />
              </Field>
              <Button
                disabled={!policy.data?.length || !policyReason.trim()}
                busy={task.busy}
                onClick={() =>
                  task.run(async () => {
                    if ((await confirmAction(t('confirmMarginPolicy'))) === null) return;
                    await api.mutate({
                      type: 'save_pricing_policy',
                      revision: s(policy.data?.[0], 'revision'),
                      minimum_margin_bps: Number(decimal(minimum, 2)),
                      reason: policyReason,
                    });
                    useStore.getState().refresh();
                    toast(t('saved'));
                  })
                }
              >
                {t('saveMarginPolicy')}
              </Button>
            </div>
          )}
        </Panel>
      )}
      <Input
        className="my-4"
        aria-label={t('searchProducts')}
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          setCursors(['0']);
        }}
      />
      <div className="mb-4 flex gap-2">
        <Button
          disabled={!query.data?.length}
          onClick={() => {
            setSelected((ids) => [
              ...new Set([...ids, ...(query.data ?? []).map((r) => s(r, 'unit_id'))]),
            ]);
            setPreview(undefined);
          }}
        >
          {t('selectVisibleUnits')}
        </Button>
        <Button
          onClick={() => {
            setSelected([]);
            setPreview(undefined);
          }}
        >
          {t('clearSelection')}
        </Button>
      </div>
      {query.error ? (
        <ErrorState error={query.error} />
      ) : query.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={query.data ?? []}
          columns={[
            {
              key: 'select',
              label: 'selectUnitForPrice',
              render: (r) => (
                <Check
                  label="selectUnitForPrice"
                  checked={selected.includes(s(r, 'unit_id'))}
                  onChange={(e) => {
                    setPreview(undefined);
                    setSelected((ids) =>
                      e.target.checked
                        ? [...ids, s(r, 'unit_id')]
                        : ids.filter((id) => id !== r.unit_id),
                    );
                  }}
                />
              ),
            },
            'code',
            'name_ar',
            'unit_name',
            'retail_price_minor',
            'wholesale_price_minor',
          ]}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={query.data?.length === 50}
        onPrevious={() => setCursors((c) => c.slice(0, -1))}
        onNext={() => setCursors((c) => [...c, s(query.data?.at(-1), 'unit_id')])}
      />
      <div className="form-grid my-4">
        <Field label="priceRule">
          <Select
            value={basis}
            onChange={(e) => {
              setBasis(e.target.value as PriceBasis);
              setPreview(undefined);
            }}
          >
            <option value="percentage">{t('pricePercentage')}</option>
            <option value="cost_markup">{t('priceCostMarkup')}</option>
          </Select>
        </Field>
        <Field label="priceList">
          <Select
            value={list}
            onChange={(e) => {
              setList(e.target.value as PriceList);
              setPreview(undefined);
            }}
          >
            {(['retail', 'wholesale', 'both'] as const).map((v) => (
              <option key={v} value={v}>
                {t(`priceLists.${v}`)}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="priceChangePercent">
          <Input
            dir="ltr"
            value={rate}
            onChange={(e) => {
              setRate(e.target.value);
              setPreview(undefined);
            }}
          />
        </Field>
        <Field label="reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
        </Field>
      </div>
      <Button
        disabled={!selected.length || selected.length > 500}
        busy={task.busy}
        onClick={() =>
          task.run(async () =>
            setPreview(
              await api.query({
                type: 'price_preview',
                unit_ids: selected,
                rule: { basis, price_list: list, rate_bps: decimal(rate, 2) },
              }),
            ),
          )
        }
      >
        {t('previewPrices')}
      </Button>
      {preview && (
        <>
          <DataTable
            rows={preview.rows}
            columns={[
              'code',
              'name_ar',
              'unit_name',
              'unit_cost_minor',
              'old_retail_minor',
              'new_retail_minor',
              'old_wholesale_minor',
              'new_wholesale_minor',
            ]}
          />
          <Button
            className="mt-4"
            tone="primary"
            disabled={!reason.trim()}
            busy={task.busy}
            onClick={() =>
              task.run(async () => {
                if ((await confirmAction(t('confirmBulkPrices'))) === null) return;
                await api.mutate({
                  type: 'apply_prices',
                  unit_ids: selected,
                  rule: { basis, price_list: list, rate_bps: decimal(rate, 2) },
                  preview_hash: preview.preview_hash,
                  reason,
                });
                setPreview(undefined);
                setSelected([]);
                useStore.getState().refresh();
                toast(t('saved'));
              })
            }
          >
            {t('applyPrices')}
          </Button>
        </>
      )}
    </Modal>
  );
}
