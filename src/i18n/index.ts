import ar from './ar.json';
export function t(key: string): string {
  let value: unknown = ar;
  for (const part of key.split('.'))
    value =
      value && typeof value === 'object' ? (value as Record<string, unknown>)[part] : undefined;
  return typeof value === 'string' ? value : ar.value;
}
export function hasLabel(key: string): boolean {
  return Object.hasOwn(ar, key);
}
export function enumLabel(value: string): string {
  return (
    (ar.enums as Record<string, string>)[value] ??
    (ar.auditActions as Record<string, string>)[value] ??
    t(value)
  );
}
export function columnLabel(key: string): string {
  return (ar.columns as Record<string, string>)[key] ?? t(key);
}
