import { useEffect, useState } from 'react';
import { api, s, type BundleDetail } from '../../api/client';
import { t } from '../../i18n';
import { toast, useStore } from '../../app/store';
import { useDebounced, useQuery, useTask } from '../../lib/hooks';
import {
  ComponentLines,
  componentPayload,
  loadComponents,
  type ComponentEdit,
} from '../../components/ComponentLines';
import { Button, Check, ErrorState, Field, Input, Loading, Modal } from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';

function BundleForm({ detail, onDone }: { detail?: BundleDetail; onDone: () => void }) {
  const header = detail?.header[0],
    task = useTask();
  const [code, setCode] = useState(s(header, 'code')),
    [name, setName] = useState(s(header, 'name_ar')),
    [active, setActive] = useState(header?.active !== '0'),
    [reason, setReason] = useState(''),
    [items, setItems] = useState<ComponentEdit[]>([]),
    [ready, setReady] = useState(!detail);
  useEffect(() => {
    let live = true;
    if (detail)
      void task.run(async () => {
        const rows = await loadComponents(detail.items);
        if (live) {
          setItems(rows);
          setReady(true);
        }
      });
    return () => {
      live = false;
    };
  }, [detail]);
  return (
    <>
      <p className="mb-4 text-sm text-slate-600">{t('bundlesHint')}</p>
      <div className="form-grid">
        <Field label="code">
          <Input value={code} onChange={(e) => setCode(e.target.value)} maxLength={64} />
        </Field>
        <Field label="name">
          <Input value={name} onChange={(e) => setName(e.target.value)} maxLength={200} />
        </Field>
      </div>
      <Check label="active" checked={active} onChange={(e) => setActive(e.target.checked)} />
      {ready ? (
        <ComponentLines items={items} onChange={setItems} disabled={task.busy} />
      ) : (
        <Loading />
      )}
      <Field label="reason">
        <Input value={reason} onChange={(e) => setReason(e.target.value)} maxLength={2000} />
      </Field>
      <div className="mt-4 flex gap-3">
        <Button
          tone="primary"
          busy={task.busy}
          disabled={!ready || !items.length || !reason.trim()}
          onClick={() =>
            task.run(async () => {
              await api.mutate({
                type: 'save_bundle',
                bundle: {
                  id: header ? s(header, 'id') : null,
                  revision: s(header, 'revision', '0'),
                  code,
                  name,
                  active,
                  items: componentPayload(items),
                },
                reason,
              });
              useStore.getState().refresh();
              toast(t('saved'));
              onDone();
            })
          }
        >
          {t('save')}
        </Button>
        <Button onClick={onDone} disabled={task.busy}>
          {t('back')}
        </Button>
      </div>
    </>
  );
}
export default function BundlesDialog({ onClose }: { onClose: () => void }) {
  const [search, setSearch] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [editing, setEditing] = useState<BundleDetail | 'new'>();
  const task = useTask(),
    query = useQuery({
      type: 'bundles',
      search: useDebounced(search),
      after_id: cursors.at(-1)!,
      limit: 50,
    });
  return (
    <Modal title="salesBundles" onClose={onClose} wide busy={task.busy}>
      {editing ? (
        <BundleForm
          key={editing === 'new' ? 'new' : s(editing.header[0], 'id')}
          detail={editing === 'new' ? undefined : editing}
          onDone={() => setEditing(undefined)}
        />
      ) : (
        <>
          <div className="mb-4 flex gap-3">
            <Input
              aria-label={t('searchBundles')}
              placeholder={t('searchBundles')}
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setCursors(['0']);
              }}
            />
            <Button onClick={() => setEditing('new')}>{t('newBundle')}</Button>
          </div>
          {query.error ? (
            <ErrorState error={query.error} />
          ) : query.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={query.data ?? []}
              columns={['code', 'name_ar', 'active']}
              actions={(r) => (
                <Button
                  busy={task.busy}
                  onClick={() =>
                    task.run(async () =>
                      setEditing(await api.query({ type: 'bundle', id: s(r, 'id') })),
                    )
                  }
                >
                  {t('edit')}
                </Button>
              )}
            />
          )}
          <Pager
            page={cursors.length - 1}
            canNext={query.data?.length === 50}
            onPrevious={() => setCursors((v) => v.slice(0, -1))}
            onNext={() => setCursors((v) => [...v, s(query.data?.at(-1), 'id')])}
          />
        </>
      )}
    </Modal>
  );
}
