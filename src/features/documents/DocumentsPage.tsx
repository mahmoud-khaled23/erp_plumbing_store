import { useState } from 'react';
import { api, s, type DocumentDetail as Detail } from '../../api/client';
import type { DocumentKind } from '../../api/contracts';
import { t } from '../../i18n';
import { allowed, useStore } from '../../app/store';
import { confirmAction } from '../../app/dialogs';
import { useQuery, useTask } from '../../lib/hooks';
import {
  Button,
  ErrorState,
  Icon,
  Loading,
  PageHeader,
  Panel,
  Select,
  Tabs,
} from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { DocumentEditor } from './DocumentEditor';
import { DocumentDetail, documentPermission } from './DocumentDetail';
import { LandedCost } from '../purchasing/LandedCost';
export function DocumentsPage({ module }: { module: 'sales' | 'purchasing' | 'receiving' }) {
  const types =
    module === 'sales'
      ? ['sale', 'quotation', 'sale_return']
      : module === 'purchasing'
        ? ['purchase_order', 'purchase', 'purchase_return', 'landed_cost']
        : ['goods_receipt'];
  const [kind, setKind] = useState(types[0]),
    [status, setStatus] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [selected, setSelected] = useState<string | null>(null),
    [edit, setEdit] = useState<Detail | 'new' | null>(null),
    [landed, setLanded] = useState(false);
  const list = useQuery(
    selected || edit
      ? null
      : { type: 'documents', kind, status: status || null, after_id: cursors.at(-1)!, limit: 50 },
  );
  const detail = useQuery(selected ? { type: 'document', id: selected } : null);
  const task = useTask();
  const open = (id: string) => {
    setSelected(id);
    setEdit(null);
  };
  return (
    <>
      <PageHeader
        title={module}
        hint={
          module === 'sales'
            ? 'salesHint'
            : module === 'purchasing'
              ? 'purchasingHint'
              : 'receivingHint'
        }
      >
        {!kind.includes('return') &&
          kind !== 'landed_cost' &&
          allowed(documentPermission(kind)) && (
            <Button
              tone="primary"
              onClick={async () => {
                if (edit === 'new') return;
                if (useStore.getState().dirty && (await confirmAction(t('unsaved'))) === null)
                  return;
                setSelected(null);
                setEdit('new');
              }}
            >
              <Icon name="plus" />
              {t('new')}
            </Button>
          )}
        {module === 'purchasing' && allowed('purchases.write') && (
          <Button onClick={() => setLanded(true)}>{t('landedCost')}</Button>
        )}
      </PageHeader>
      {edit ? (
        <DocumentEditor
          key={edit === 'new' ? `new-${kind}` : s(edit.header[0], 'id')}
          kind={(edit === 'new' ? kind : s(edit.header[0], 'kind')) as DocumentKind}
          initial={edit === 'new' ? undefined : edit}
          onComplete={open}
          onCancel={() => {
            setEdit(null);
            useStore.getState().setDirty(false);
            useStore.getState().refresh();
          }}
        />
      ) : selected ? (
        detail.error ? (
          <ErrorState error={detail.error} />
        ) : !detail.data ? (
          <Loading />
        ) : (
          <DocumentDetail
            detail={detail.data}
            onEdit={setEdit}
            onChanged={(id) => {
              setSelected(id);
              useStore.getState().refresh();
            }}
            onBack={() => setSelected(null)}
          />
        )
      ) : (
        <>
          <Tabs
            value={kind}
            onChange={(k) => {
              setKind(k);
              setCursors(['0']);
              setStatus('');
            }}
            items={types.map((value) => ({ value, label: `enums.${value}` }))}
          />
          <Panel>
            <div className="mb-4 max-w-56">
              <Select
                aria-label={t('status')}
                value={status}
                onChange={(e) => {
                  setStatus(e.target.value);
                  setCursors(['0']);
                }}
              >
                <option value="">{t('all')}</option>
                {['draft', 'held', 'approved', 'posted', 'closed', 'cancelled'].map((v) => (
                  <option value={v} key={v}>
                    {t(`enums.${v}`)}
                  </option>
                ))}
              </Select>
            </div>
            {list.error ? (
              <ErrorState error={list.error} />
            ) : list.loading ? (
              <Loading />
            ) : (
              <DataTable
                rows={list.data ?? []}
                columns={[
                  'number',
                  'document_date',
                  'party_name_snapshot',
                  'total_minor',
                  'status',
                  ...(kind === 'purchase_order' ? ['progress'] : []),
                  'reversal_number',
                ]}
                onRow={(r) => open(s(r, 'id'))}
              />
            )}
            <Pager
              page={cursors.length - 1}
              canNext={list.data?.length === 50}
              onPrevious={() => setCursors((v) => v.slice(0, -1))}
              onNext={() => setCursors((v) => [...v, s(list.data?.at(-1), 'id')])}
            />
          </Panel>
        </>
      )}
      {landed && (
        <LandedCost
          onClose={() => setLanded(false)}
          onDone={(id) => {
            setLanded(false);
            open(id);
          }}
        />
      )}
    </>
  );
}
export function PosPage() {
  const target = useStore((s) => s.targetDocument);
  const initial = useQuery(target ? { type: 'document', id: target } : null);
  const [completed, setCompleted] = useState<string | null>(null),
    [generation, setGeneration] = useState(0);
  const detail = useQuery(completed ? { type: 'document', id: completed } : null);
  return (
    <>
      <PageHeader title="pos" hint="scanHint">
        <Button
          onClick={async () => {
            const store = useStore.getState();
            if (await store.navigate('pos')) {
              setCompleted(null);
              setGeneration((g) => g + 1);
            }
          }}
        >
          <Icon name="plus" />
          {t('newInvoice')}
        </Button>
      </PageHeader>
      {completed ? (
        detail.error ? (
          <ErrorState error={detail.error} />
        ) : detail.data ? (
          <DocumentDetail
            detail={detail.data}
            onChanged={setCompleted}
            onBack={() => {
              setCompleted(null);
              useStore.setState({ targetDocument: null });
              setGeneration((g) => g + 1);
            }}
          />
        ) : (
          <Loading />
        )
      ) : target && !initial.data ? (
        initial.error ? (
          <ErrorState error={initial.error} />
        ) : (
          <Loading />
        )
      ) : initial.data && s(initial.data.header[0], 'status') !== 'draft' ? (
        <DocumentDetail
          detail={initial.data}
          onChanged={() => useStore.getState().refresh()}
          onEdit={() => useStore.getState().refresh()}
        />
      ) : (
        <DocumentEditor
          key={`${target ?? 'new'}-${generation}`}
          kind="sale"
          initial={initial.data}
          onComplete={setCompleted}
        />
      )}
    </>
  );
}
