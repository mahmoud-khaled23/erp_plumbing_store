import { useMemo, useId, type ReactNode } from 'react';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { t } from '../i18n';
import { decimal, parseDate } from '../lib/numbers';
import { useTask } from '../lib/hooks';
import { Button } from './ui';
export type FormValues = Record<string, string | boolean>;
export interface FormField {
  name: string;
  label: string;
  type?: 'text' | 'password' | 'number' | 'date' | 'textarea' | 'select' | 'check';
  required?: boolean;
  min?: number;
  max?: number;
  digits?: number;
  signed?: boolean;
  options?: { value: string; label: string }[];
  hint?: string;
  disabled?: boolean;
  span?: boolean;
}
export function SmartForm({
  fields,
  initial,
  onSubmit,
  submit = 'save',
  children,
  onCancel,
  validate,
}: {
  fields: FormField[];
  initial: FormValues;
  onSubmit: (values: FormValues) => Promise<unknown>;
  submit?: string;
  children?: ReactNode;
  onCancel?: () => void;
  validate?: (values: FormValues) => string | undefined;
}) {
  const prefix = useId();
  const schema = useMemo(
    () =>
      z.object(
        Object.fromEntries(
          fields.map((f) => [
            f.name,
            f.type === 'check'
              ? z.boolean()
              : z.string().superRefine((v, ctx) => {
                  const fail = (key: string) => ctx.addIssue({ code: 'custom', message: t(key) });
                  if (f.required && !v.trim()) return fail('required');
                  if (!v.trim()) return;
                  if (f.min !== undefined && v.length < f.min) fail('passwordHint');
                  if (f.max !== undefined && v.length > f.max) fail('tooLarge');
                  if (f.type === 'number') {
                    try {
                      decimal(v, f.digits ?? 0, f.signed);
                    } catch {
                      fail('invalidNumber');
                    }
                  }
                  if (f.type === 'date') {
                    try {
                      parseDate(v);
                    } catch {
                      fail('invalidDate');
                    }
                  }
                }),
          ]),
        ),
      ),
    [fields],
  );
  const form = useForm<FormValues>({ defaultValues: initial, resolver: zodResolver(schema) });
  const task = useTask();
  return (
    <form
      noValidate
      onSubmit={form.handleSubmit(async (values) => {
        const error = validate?.(values);
        if (error) {
          form.setError('root', { message: error });
          return;
        }
        await task.run(() => onSubmit(values));
      })}
    >
      <div className="form-grid">
        {fields.map((f) => (
          <label key={f.name} className={`field ${f.span ? 'md:col-span-2' : ''}`}>
            <span className="field-label">
              <span id={`${prefix}-${f.name}-label`}>{t(f.label)}</span>
              {f.required && (
                <span aria-hidden="true" className="text-red-600 ms-1">
                  *
                </span>
              )}
            </span>
            {f.type === 'check' ? (
              <input
                type="checkbox"
                {...form.register(f.name)}
                aria-labelledby={`${prefix}-${f.name}-label`}
                aria-describedby={`${prefix}-${f.name}-hint`}
                disabled={f.disabled || task.busy}
                className="h-5 w-5 accent-accent"
              />
            ) : f.type === 'select' ? (
              <select
                className="input"
                {...form.register(f.name)}
                aria-labelledby={`${prefix}-${f.name}-label`}
                aria-describedby={`${prefix}-${f.name}-hint`}
                disabled={f.disabled || task.busy}
              >
                <option value="">{t('select')}</option>
                {f.options?.map((o) => (
                  <option value={o.value} key={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            ) : f.type === 'textarea' ? (
              <textarea
                className="input min-h-24"
                {...form.register(f.name)}
                aria-labelledby={`${prefix}-${f.name}-label`}
                aria-describedby={`${prefix}-${f.name}-hint`}
                disabled={task.busy}
              />
            ) : (
              <input
                className="input"
                type={f.type === 'password' ? 'password' : 'text'}
                dir={f.type === 'number' || f.type === 'date' ? 'ltr' : undefined}
                inputMode={
                  f.type === 'number' ? 'decimal' : f.type === 'date' ? 'numeric' : undefined
                }
                autoComplete={f.type === 'password' ? 'new-password' : undefined}
                {...form.register(f.name)}
                aria-labelledby={`${prefix}-${f.name}-label`}
                aria-describedby={`${prefix}-${f.name}-hint`}
                aria-invalid={!!form.formState.errors[f.name]}
                disabled={f.disabled || task.busy}
              />
            )}{' '}
            {f.hint && (
              <span id={`${prefix}-${f.name}-hint`} className="field-hint">
                {t(f.hint)}
              </span>
            )}
            {form.formState.errors[f.name] && (
              <span role="alert" className="field-error">
                {String(form.formState.errors[f.name]?.message ?? '')}
              </span>
            )}
          </label>
        ))}
      </div>
      {children}
      {form.formState.errors.root && (
        <p role="alert" className="field-error mt-3">
          {form.formState.errors.root.message}
        </p>
      )}
      <div className="mt-6 flex justify-end gap-2">
        {onCancel && (
          <Button disabled={task.busy} onClick={onCancel}>
            {t('cancel')}
          </Button>
        )}
        <Button type="submit" tone="primary" busy={task.busy}>
          {t(submit)}
        </Button>
      </div>
    </form>
  );
}
export const textValue = (v: FormValues, key: string) => String(v[key] ?? '');
