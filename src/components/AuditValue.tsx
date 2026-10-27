import { useState } from 'react';
import { columnLabel, enumLabel, t } from '../i18n';
import { CellValue } from './Table';
import { Button } from './ui';
export function AuditValue({ value, name = 'details' }: { value: unknown; name?: string }) {
  const [open, setOpen] = useState(false),
    [page, setPage] = useState(0);
  if (value === null || typeof value !== 'object')
    return (
      <div className="flex flex-wrap gap-3 py-1 text-sm">
        <span className="text-slate-500">{columnLabel(name)}</span>
        {name === 'type' ? (
          enumLabel(String(value))
        ) : typeof value === 'boolean' ? (
          t(value ? 'yes' : 'no')
        ) : (
          <CellValue field={name} value={value == null ? null : String(value)} />
        )}
      </div>
    );
  const entries = Object.entries(value);
  return (
    <div className="my-2 border-s-2 border-slate-100 ps-3">
      <Button tone="ghost" onClick={() => setOpen((v) => !v)}>
        {columnLabel(name)} <bdi>({entries.length})</bdi>
      </Button>
      {open && (
        <>
          <div className="ps-3">
            {entries.slice(page * 50, page * 50 + 50).map(([k, v]) => (
              <AuditValue key={k} name={Array.isArray(value) ? 'details' : k} value={v} />
            ))}
          </div>
          {entries.length > 50 && (
            <div className="flex gap-2">
              <Button disabled={!page} onClick={() => setPage((p) => p - 1)}>
                {t('previous')}
              </Button>
              <Button
                disabled={(page + 1) * 50 >= entries.length}
                onClick={() => setPage((p) => p + 1)}
              >
                {t('next')}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
export function AuditJson({ text }: { text: string }) {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    value = text;
  }
  return <AuditValue value={value} />;
}
