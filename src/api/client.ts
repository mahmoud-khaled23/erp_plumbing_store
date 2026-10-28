import { invoke, isTauri, convertFileSrc } from '@tauri-apps/api/core';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
import { z } from 'zod';
import type { Query, Mutation, Setup, Product, PrintSource, PrintPaper } from './contracts';
import { t } from '../i18n';

const rowSchema = z.record(z.string(), z.string().nullable());
const rowsSchema = z.array(rowSchema);
export type Row = z.infer<typeof rowSchema>;
export const s = (r: Row | undefined, key: string, fallback = '') => r?.[key] ?? fallback;
const bootstrapSchema = z.object({
  authenticated: z.boolean(),
  setup_required: z.boolean(),
  recovery_required: z.boolean().optional(),
  message: z.string().optional(),
  permissions: z.array(z.string()).default([]),
  user_id: z.string().optional(),
  display_name: z.string().optional(),
  must_change_password: z.boolean().optional(),
  settings: rowsSchema.default([]),
  categories: rowsSchema.default([]),
  units: rowsSchema.default([]),
  warehouses: rowsSchema.default([]),
  cash_accounts: rowsSchema.default([]),
  expense_categories: rowsSchema.default([]),
  accounts: rowsSchema.default([]),
  fiscal_periods: rowsSchema.default([]),
  sequences: rowsSchema.default([]),
  backup_runs: rowsSchema.default([]),
});
export type Bootstrap = z.infer<typeof bootstrapSchema>;
const documentSchema = z.object({
  header: rowsSchema,
  lines: rowsSchema,
  payments: rowsSchema,
  counts: rowsSchema,
  expenses: rowsSchema,
  events: rowsSchema,
  receiving: rowsSchema,
  matches: rowsSchema,
  cheques: rowsSchema,
  journal: rowsSchema.default([]),
  settlements: rowsSchema.default([]),
  availability: rowsSchema.default([]),
  returned: rowsSchema.default([]),
});
export type DocumentDetail = z.infer<typeof documentSchema>;
const printSnapshotSchema = z.object({
  document: documentSchema.nullable(),
  rows: rowsSchema,
  opening: rowsSchema,
  context: rowsSchema,
  store: rowSchema,
  preferences: rowSchema,
  references: z.record(z.string(), rowsSchema),
  generated_at: z.string(),
});
export type PrintSnapshot = z.infer<typeof printSnapshotSchema>;
const productSchema = z.object({
  product: rowsSchema,
  units: rowsSchema,
  warehouse_settings: rowsSchema.default([]),
});
export type ProductDetail = z.infer<typeof productSchema>;
const dashboardSchema = z.object({
  sales: rowsSchema,
  low_stock_count: z.string(),
  low_stock: rowsSchema,
  sales_trend: rowsSchema,
  top_products: rowsSchema,
  categories: rowsSchema,
  cash: rowsSchema.default([]),
  receivables: rowsSchema.default([]),
  payables: rowsSchema.default([]),
  cash_flow: rowsSchema.default([]),
  cheques_due: rowsSchema.default([]),
  overdue: rowsSchema.default([]),
  recent_invoices: rowsSchema.default([]),
  balances: rowsSchema.default([]),
});
export type DashboardData = z.infer<typeof dashboardSchema>;
const usersSchema = z.object({ users: rowsSchema, roles: rowsSchema, user_roles: rowsSchema });
export type UsersData = z.infer<typeof usersSchema>;
const cashDeskSchema = z.object({ shifts: rowsSchema, balances: rowsSchema });
export type ReportData = Row[] | { rows: Row[]; opening: Row[] };
export type QueryResult<Q extends Query> = Q extends { type: 'bootstrap' }
  ? Bootstrap
  : Q extends { type: 'print_snapshot' }
    ? PrintSnapshot
    : Q extends { type: 'product' }
      ? ProductDetail
      : Q extends { type: 'document' }
        ? DocumentDetail
        : Q extends { type: 'dashboard' }
          ? DashboardData
          : Q extends { type: 'users' }
            ? UsersData
            : Q extends { type: 'report' }
              ? ReportData
              : Q extends { type: 'cash_desk' }
                ? z.infer<typeof cashDeskSchema>
                : Row[];
export interface Operation {
  id?: string;
  revision?: string;
  status?: string;
  number?: string;
  cancelled?: boolean;
  restored?: boolean;
  recovered?: boolean;
  path?: string;
  imported?: string;
  token?: string;
  count?: string;
  preview?: unknown[];
  headers?: string[];
}
export type FileAction =
  | { type: 'export_products'; excel: boolean }
  | { type: 'import_products' }
  | { type: 'import_opening'; balances: boolean }
  | { type: 'backup' }
  | { type: 'backup_directory' }
  | { type: 'restore' }
  | { type: 'image'; product_id: string | null };
export class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public retryable = false,
  ) {
    super(message);
  }
}
const failureSchema = z.object({ code: z.string(), message: z.string(), retryable: z.boolean() });
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new ApiError('desktop', t('desktopRequired'));
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    const parsed = failureSchema.safeParse(error);
    if (parsed.success) {
      if (parsed.data.code === 'unauthenticated' && typeof window !== 'undefined')
        window.dispatchEvent(new Event('erp-session-expired'));
      throw new ApiError(parsed.data.code, parsed.data.message, parsed.data.retryable);
    }
    throw new ApiError('transport', t('unexpected'), true);
  }
}
const pending = new Map<string, string>();
export const api = {
  async query<Q extends Query>(query: Q): Promise<QueryResult<Q>> {
    const result = await call<unknown>('erp_query', { query });
    const schema =
      query.type === 'print_snapshot'
        ? printSnapshotSchema
        : query.type === 'bootstrap'
          ? bootstrapSchema
          : query.type === 'document'
            ? documentSchema
            : query.type === 'product'
              ? productSchema
              : query.type === 'dashboard'
                ? dashboardSchema
                : query.type === 'users'
                  ? usersSchema
                  : query.type === 'cash_desk'
                    ? cashDeskSchema
                    : query.type === 'report'
                      ? z.union([rowsSchema, z.object({ rows: rowsSchema, opening: rowsSchema })])
                      : rowsSchema;
    const parsed = schema.safeParse(result);
    if (!parsed.success) throw new ApiError('contract', t('unexpected'));
    return parsed.data as QueryResult<Q>;
  },
  async mutate(mutation: Mutation): Promise<Operation> {
    const key = JSON.stringify(mutation);
    const request_id = pending.get(key) ?? crypto.randomUUID();
    pending.set(key, request_id);
    try {
      const result = await call<Operation>('erp_mutate', { request: { request_id, mutation } });
      pending.delete(key);
      return result;
    } catch (error) {
      if (!(error instanceof ApiError) || !error.retryable) pending.delete(key);
      throw error;
    }
  },
  setup: (input: Setup) => call('erp_setup', { input }),
  login: (username: string, password: string) =>
    call<{ id: string; name: string; permissions: string[] }>('erp_login', { username, password }),
  logout: async () => {
    await call('erp_logout');
    pending.clear();
  },
  password: (old: string, next: string) => call('erp_change_password', { old, new: next }),
  createUser: (username: string, name: string, password: string, roles: string[]) =>
    call('erp_create_user', { username, name, password, roles }),
  resetPassword: (userId: string, password: string) =>
    call('erp_reset_password', { userId, password }),
  file: (action: FileAction) => call<Operation>('erp_file', { action }),
  frontendReady: () => (isTauri() ? call<void>('erp_frontend_ready') : Promise.resolve()),
  print: (source: PrintSource, paper: PrintPaper) => call<void>('erp_print', { source, paper }),
  exportPdf: (source: PrintSource, paper: PrintPaper) =>
    call<Operation>('erp_export_pdf', { source, paper }),
  importProducts: (token: string, replace: boolean) =>
    call<Operation>('erp_import_apply', { token, replace }),
  importOpening: (token: string, balances: boolean, date: string, warehouseId: string | null) =>
    call<Operation>('erp_import_opening_apply', { token, balances, date, warehouseId }),
  recover: (username: string, password: string) =>
    call<Operation>('erp_recover', { username, password }),
};
let assetRoot: Promise<string> | undefined;
export async function assetUrl(relative: string): Promise<string> {
  if (!/^assets\/[a-zA-Z0-9-]+\.(png|jpg)$/.test(relative)) return '';
  assetRoot ??= appLocalDataDir().then((p) => join(p, 'store'));
  return convertFileSrc(await join(await assetRoot, relative));
}
export function productInput(detail: ProductDetail): Product {
  const p = detail.product[0];
  return {
    id: s(p, 'id'),
    revision: s(p, 'revision'),
    code: s(p, 'code'),
    name: s(p, 'name_ar'),
    category_id: s(p, 'category_id'),
    brand: s(p, 'brand'),
    material: s(p, 'material'),
    size: s(p, 'size'),
    base_unit_id: s(p, 'base_unit_id'),
    standard_cost: s(p, 'standard_cost_minor'),
    min_stock: s(p, 'min_stock_micros'),
    active: s(p, 'active') === '1',
    units: detail.units.map((u) => ({
      id: s(u, 'id'),
      unit_id: s(u, 'unit_id'),
      numerator: s(u, 'factor_numerator'),
      denominator: s(u, 'factor_denominator'),
      barcode: u.barcode,
      retail_price: s(u, 'retail_price_minor'),
      wholesale_price: s(u, 'wholesale_price_minor'),
      active: s(u, 'active') === '1',
    })),
  };
}
