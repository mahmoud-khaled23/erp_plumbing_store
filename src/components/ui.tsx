import {
  cloneElement,
  isValidElement,
  forwardRef,
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactElement,
  type ReactNode,
  type SelectHTMLAttributes,
} from 'react';
import { t, enumLabel } from '../i18n';
import { errorMessage, useStore, useToasts } from '../app/store';
import { useConfirmation } from '../app/dialogs';

export function Button({
  tone = 'secondary',
  busy = false,
  children,
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: 'primary' | 'secondary' | 'danger' | 'ghost';
  busy?: boolean;
}) {
  return (
    <button
      type="button"
      {...props}
      disabled={props.disabled || busy}
      className={`btn btn-${tone} ${className}`}
    >
      {busy && <span className="spinner" />}
      {children}
    </button>
  );
}
export function Field({
  label,
  error,
  children,
  hint,
}: {
  label: string;
  error?: string;
  children: ReactNode;
  hint?: string;
}) {
  const id = useId();
  const control = isValidElement(children)
    ? cloneElement(children as ReactElement<Record<string, unknown>>, {
        'aria-labelledby': id,
        'aria-describedby': hint || error ? `${id}-description` : undefined,
      })
    : children;
  return (
    <label className="field">
      <span id={id} className="field-label">
        {t(label)}
      </span>
      {control}
      <span id={`${id}-description`}>
        {hint && <span className="field-hint">{t(hint)}</span>}
        {error && (
          <span className="field-error" role="alert">
            {error}
          </span>
        )}
      </span>
    </label>
  );
}
export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  (props, ref) => <input ref={ref} {...props} className={`input ${props.className ?? ''}`} />,
);
export const Select = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(
  (props, ref) => <select ref={ref} {...props} className={`input ${props.className ?? ''}`} />,
);
export const Check = forwardRef<
  HTMLInputElement,
  InputHTMLAttributes<HTMLInputElement> & { label: string }
>(({ label, ...props }, ref) => (
  <label className="flex items-center gap-2 py-2 text-sm">
    <input ref={ref} {...props} type="checkbox" className="h-4 w-4 accent-accent" />
    {t(label)}
  </label>
));
export function PageHeader({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children?: ReactNode;
}) {
  return (
    <header className="mb-6 flex flex-wrap items-center justify-between gap-4">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-ink">{t(title)}</h1>
        {hint && <p className="mt-1 text-sm text-slate-500">{t(hint)}</p>}
      </div>
      <div className="flex flex-wrap items-center gap-2">{children}</div>
    </header>
  );
}
export function Panel({
  title,
  children,
  className = '',
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`panel ${className}`}>
      {title && <h2 className="mb-4 text-base font-bold text-ink">{t(title)}</h2>}
      {children}
    </section>
  );
}
export function Empty({ title = 'empty', hint = 'emptyHint' }: { title?: string; hint?: string }) {
  return (
    <div className="empty-state">
      <div className="empty-glyph">
        <Icon name="box" size={28} />
      </div>
      <p className="font-semibold text-ink">{t(title)}</p>
      <p className="mt-1 text-sm text-slate-500">{t(hint)}</p>
    </div>
  );
}
export function Loading() {
  return (
    <div className="flex min-h-40 items-center justify-center gap-3 text-slate-500" role="status">
      <span className="spinner" />
      {t('loading')}
    </div>
  );
}
export function ErrorState({ error, retry }: { error: unknown; retry?: () => void }) {
  return (
    <div role="alert" className="rounded-xl border border-red-200 bg-red-50 p-5 text-red-800">
      <p>{errorMessage(error)}</p>
      <Button className="mt-3" onClick={retry ?? (() => useStore.getState().refresh())}>
        {t('retry')}
      </Button>
    </div>
  );
}
export function Status({ value }: { value: string }) {
  const color = ['posted', 'approved', 'cleared', 'verified', 'active', 'closed'].includes(value)
    ? 'bg-teal-50 text-teal-800'
    : ['cancelled', 'bounced', 'failed', 'inactive'].includes(value)
      ? 'bg-red-50 text-red-700'
      : 'bg-amber-50 text-amber-800';
  return (
    <span
      className={`inline-flex whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-semibold ${color}`}
    >
      {enumLabel(value)}
    </span>
  );
}
export function Tabs<T extends string>({
  value,
  onChange,
  items,
}: {
  value: T;
  onChange: (value: T) => void;
  items: { value: T; label: string }[];
}) {
  return (
    <div role="tablist" className="tabs">
      {items.map((item) => (
        <button
          key={item.value}
          type="button"
          role="tab"
          aria-selected={value === item.value}
          className={value === item.value ? 'selected' : ''}
          onClick={() => onChange(item.value)}
        >
          {t(item.label)}
        </button>
      ))}
    </div>
  );
}
export function Modal({
  title,
  children,
  onClose,
  wide = false,
  busy = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
  busy?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null),
    titleId = useId();
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const items = useToasts((s) => s.items);
  const initialToasts = useRef(new Set(items.map((x) => x.id)));
  const errors = items.filter((x) => x.tone === 'error' && !initialToasts.current.has(x.id));
  useEffect(() => {
    const dialog = ref.current!;
    const focused = document.activeElement as HTMLElement | null;
    dialog.showModal();
    return () => {
      dialog.close();
      focused?.focus();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) closeRef.current();
      }}
      className={`modal ${wide ? 'modal-wide' : ''}`}
    >
      <div className="modal-header">
        <h2 id={titleId}>{t(title)}</h2>
        <Button tone="ghost" disabled={busy} onClick={onClose} aria-label={t('close')}>
          <Icon name="close" />
        </Button>
      </div>
      <div className="p-6">
        {errors.map((e) => (
          <p key={e.id} role="alert" className="mb-4 rounded-lg bg-red-50 p-3 text-sm text-red-800">
            {e.message}
          </p>
        ))}
        {children}
      </div>
    </dialog>
  );
}
export function ConfirmHost() {
  const request = useConfirmation((s) => s.request);
  const [reason, setReason] = useState('');
  useEffect(() => setReason(''), [request]);
  if (!request) return null;
  const done = (answer: string | null) => {
    useConfirmation.setState({ request: null });
    request.resolve(answer);
  };
  return (
    <Modal title="confirmAction" onClose={() => done(null)}>
      <p className="mb-5 leading-8">{request.message}</p>
      {request.reason && (
        <Field label="reason">
          <Input autoFocus value={reason} onChange={(e) => setReason(e.target.value)} />
        </Field>
      )}
      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={() => done(null)}>{t('cancel')}</Button>
        <Button
          tone="primary"
          autoFocus={!request.reason}
          disabled={request.reason && !reason.trim()}
          onClick={() => done(reason)}
        >
          {t('confirm')}
        </Button>
      </div>
    </Modal>
  );
}
export function ToastHost() {
  const { items, remove } = useToasts();
  return (
    <div className="toast-host" aria-live="polite">
      {items.map((x) => (
        <div
          key={x.id}
          role={x.tone === 'error' ? 'alert' : 'status'}
          className={`toast ${x.tone === 'error' ? 'toast-error' : ''}`}
        >
          <span>{x.message}</span>
          <button aria-label={t('close')} onClick={() => remove(x.id)}>
            <Icon name="close" size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}
const paths: Record<string, string> = {
  dashboard: 'M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z',
  pos: 'M3 4h18v12H3z M7 20h10 M12 16v4',
  box: 'M12 3 3 7.5v9L12 21l9-4.5v-9L12 3Z M3 7.5l9 4.5 9-4.5 M12 12v9 M7.5 5.25l9 4.5',
  inventory: 'M3 21V9l9-6 9 6v12 M7 21v-8h10v8 M7 17h10',
  sales: 'M5 3h14v18l-3-2-4 2-4-2-3 2V3Z M8 7h8 M8 11h8 M8 15h4',
  purchasing: 'M3 3h2l3 12h10l3-8H6 M9 20h.01 M18 20h.01',
  receiving: 'M12 3v12 M8 11l4 4 4-4 M4 17v4h16v-4',
  parties:
    'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75',
  finance: 'M3 7h18v14H3z M3 7V3h14 M16 12h5v5h-5z',
  reports: 'M4 3v18h17 M8 16v-4 M13 16V8 M18 16V5',
  settings:
    'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M12 2v3 M12 19v3 M2 12h3 M19 12h3 M5 5l2 2 M17 17l2 2 M5 19l2-2 M17 7l2-2',
  users: 'M12 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M4 21v-2a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v2',
  audit: 'M4 3h16v18H4z M8 7h8 M8 12h8 M8 17h5',
  search: 'M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14 M15 15l6 6',
  plus: 'M12 5v14 M5 12h14',
  close: 'M6 6l12 12 M18 6 6 18',
  chevron: 'm9 5 7 7-7 7',
  logout: 'M9 3H3v18h6 M13 8l5 4-5 4 M8 12h13',
  check: 'm5 12 4 4L19 6',
  arrow: 'M5 12h14 M13 6l6 6-6 6',
  clock: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18 M12 7v5l3 2',
  menu: 'M3 6h18 M3 12h18 M3 18h18',
};
export function Icon({ name, size = 20 }: { name: string; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name] ?? paths.box} />
    </svg>
  );
}
