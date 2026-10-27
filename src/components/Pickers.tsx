import { useEffect, useId, useRef, useState } from 'react';
import { api, type Row, s } from '../api/client';
import { useDebounced, useQuery } from '../lib/hooks';
import { t } from '../i18n';
import { handleError, useStore } from '../app/store';
import { Button, Field, Icon, Input, Modal, Select, ErrorState, Loading } from './ui';
import { DataTable, Pager } from './Table';
export function masterOptions(
  key: 'categories' | 'units' | 'warehouses' | 'cash_accounts' | 'accounts' | 'expense_categories',
  activeOnly = true,
) {
  return (useStore.getState().bootstrap?.[key] ?? [])
    .filter((r) => !activeOnly || r.active !== '0')
    .map((r) => ({ value: s(r, 'id'), label: s(r, 'name_ar', s(r, 'name')) }));
}
export function MasterSelect({
  table,
  value,
  onChange,
  label,
  optional = false,
  filter,
}: {
  table: Parameters<typeof masterOptions>[0];
  value: string;
  onChange: (id: string) => void;
  label: string;
  optional?: boolean;
  filter?: (r: Row) => boolean;
}) {
  const bootstrap = useStore((s) => s.bootstrap);
  const list = (bootstrap?.[table] ?? []).filter((r) => r.active !== '0' && (!filter || filter(r)));
  return (
    <Field label={label}>
      <Select value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">{t(optional ? 'none' : 'select')}</option>
        {list.map((r) => (
          <option value={s(r, 'id')} key={s(r, 'id')}>
            {s(r, 'name_ar', s(r, 'name'))}
          </option>
        ))}
      </Select>
    </Field>
  );
}
export function PartyPicker({
  value,
  name,
  onChange,
  supplier = false,
  optional = true,
}: {
  value: string | null;
  name?: string;
  onChange: (id: string | null, row?: Row) => void;
  supplier?: boolean;
  optional?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [cursors, setCursors] = useState(['0']);
  const q = useDebounced(search);
  const result = useQuery(
    open ? { type: 'parties', search: q, after_id: cursors.at(-1)!, limit: 50 } : null,
  );
  const [selectedName, setSelectedName] = useState(name ?? '');
  useEffect(() => {
    setSelectedName(name ?? '');
  }, [name, value]);
  const rows = (result.data ?? []).filter(
    (r) => r.active === '1' && r[supplier ? 'is_supplier' : 'is_customer'] === '1',
  );
  return (
    <>
      <div className="field">
        <span className="field-label">{t(supplier ? 'supplier' : 'party')}</span>
        <div className="flex gap-2">
          <Button className="min-w-0 flex-1 justify-between" onClick={() => setOpen(true)}>
            <span className="truncate">
              {value ? selectedName || value : t(optional ? 'walkIn' : 'select')}
            </span>
            <Icon name="search" size={16} />
          </Button>
          {value && optional && (
            <Button aria-label={t('remove')} onClick={() => onChange(null)}>
              <Icon name="close" size={16} />
            </Button>
          )}
        </div>
      </div>
      {open && (
        <Modal title="parties" onClose={() => setOpen(false)} wide>
          <Input
            autoFocus
            placeholder={t('searchParties')}
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setCursors(['0']);
            }}
          />
          {result.error ? (
            <ErrorState error={result.error} />
          ) : result.loading ? (
            <Loading />
          ) : (
            <DataTable
              rows={rows}
              columns={['code', 'name_ar', 'phone']}
              onRow={(r) => {
                setSelectedName(s(r, 'name_ar'));
                onChange(s(r, 'id'), r);
                setOpen(false);
              }}
            />
          )}
          <Pager
            page={cursors.length - 1}
            canNext={(result.data?.length ?? 0) === 50}
            onPrevious={() => setCursors((x) => x.slice(0, -1))}
            onNext={() => setCursors((x) => [...x, s(result.data?.at(-1), 'id')])}
          />
        </Modal>
      )}
    </>
  );
}
export function ProductSearch({
  onSelect,
  autoFocus = false,
  onPending,
}: {
  onSelect: (row: Row) => void | Promise<void>;
  autoFocus?: boolean;
  onPending?: (pending: boolean) => void;
}) {
  const [search, setSearch] = useState('');
  const [active, setActive] = useState(0);
  const [focused, setFocused] = useState(false);
  const [consumed, setConsumed] = useState(false);
  const q = useDebounced(search, 110);
  const result = useQuery(
    q.trim() ? { type: 'products', search: q, after_id: '0', limit: 30 } : null,
  );
  const rows = (result.data ?? []).filter((r) => r.active === '1');
  const queue = useRef<Promise<void>>(Promise.resolve());
  const select = useRef(onSelect);
  select.current = onSelect;
  const pendingCount = useRef(0);
  const pendingChanged = useRef(onPending);
  pendingChanged.current = onPending;
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  const listId = useId();
  // Capture and clear each scan immediately; serialize lookups and additions so
  // a scanner can submit the next barcode while SQLite resolves the previous one.
  const enqueue = (resolve: () => Promise<Row | undefined>) => {
    pendingCount.current += 1;
    pendingChanged.current?.(true);
    setSearch('');
    setConsumed(true);
    setActive(0);
    queue.current = queue.current
      .then(async () => {
        if (!live.current) return;
        const row = await resolve();
        if (live.current && row?.active === '1') await select.current(row);
      })
      .catch(handleError)
      .finally(() => {
        pendingCount.current -= 1;
        if (live.current) pendingChanged.current?.(pendingCount.current > 0);
      });
  };
  const choose = (row: Row) => enqueue(async () => row);
  return (
    <div className="relative">
      <div className="search-field">
        <Icon name="search" />
        <input
          data-product-search
          aria-label={t('searchProducts')}
          role="combobox"
          aria-controls={listId}
          aria-expanded={focused && !!search && !consumed}
          aria-autocomplete="list"
          aria-activedescendant={rows[active] ? `${listId}-${active}` : undefined}
          autoFocus={autoFocus}
          value={search}
          placeholder={t('searchProducts')}
          autoComplete="off"
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onChange={(e) => {
            setSearch(e.target.value);
            setConsumed(false);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive((i) => Math.max(0, Math.min(i + 1, rows.length - 1)));
            }
            if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive((i) => Math.max(0, i - 1));
            }
            if (e.key === 'Escape') {
              setConsumed(true);
            }
            if (e.key === 'Enter') {
              e.preventDefault();
              const term = search.trim();
              if (!term) return;
              const index = active;
              enqueue(async () => {
                const exact = await api.query({
                  type: 'products',
                  search: term,
                  after_id: '0',
                  limit: 30,
                });
                const hit = exact.find((r) => r.matched_unit_id) || exact[index];
                if (!hit || hit.active !== '1') throw new Error(t('noResults'));
                return hit;
              });
            }
          }}
        />
        <kbd>F4</kbd>
      </div>
      {focused && !!search && !consumed && (
        <div role="listbox" id={listId} className="search-results">
          {rows.map((r, i) => (
            <button
              id={`${listId}-${i}`}
              role="option"
              aria-selected={i === active}
              type="button"
              key={s(r, 'id')}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => choose(r)}
              className={i === active ? 'selected' : ''}
            >
              <span>{s(r, 'name_ar')}</span>
              <bdi className="text-xs text-slate-500">{s(r, 'code')}</bdi>
            </button>
          ))}
          {!rows.length && (
            <p className="p-4 text-sm text-slate-500">
              {t(result.loading ? 'loading' : 'noResults')}
            </p>
          )}
          {!!result.error && <ErrorState error={result.error} />}
        </div>
      )}
    </div>
  );
}
