import { create } from 'zustand';
import type { Bootstrap } from '../api/client';
import { api, ApiError, s } from '../api/client';
import { t } from '../i18n';
import { today, formatDecimal, setNumberStyle } from '../lib/numbers';
import { confirmAction } from './dialogs';
export type Route =
  | 'dashboard'
  | 'pos'
  | 'products'
  | 'inventory'
  | 'sales'
  | 'purchasing'
  | 'receiving'
  | 'parties'
  | 'finance'
  | 'reports'
  | 'settings'
  | 'users'
  | 'audit';
interface State {
  bootstrap: Bootstrap | null;
  epoch: number;
  route: Route;
  targetDocument: string | null;
  wizard: boolean;
  dirty: boolean;
  setBootstrap: (b: Bootstrap | null) => void;
  refresh: () => void;
  navigate: (route: Route, document?: string | null) => Promise<boolean>;
  setDirty: (v: boolean) => void;
  setWizard: (v: boolean) => void;
}
export const useStore = create<State>((set, get) => ({
  bootstrap: null,
  epoch: 0,
  route: 'dashboard',
  targetDocument: null,
  wizard: false,
  dirty: false,
  setBootstrap: (bootstrap) => {
    setNumberStyle(bootstrap?.settings[0]?.digit_style === 'arabic_indic');
    set({ bootstrap });
  },
  refresh: () => set((s) => ({ epoch: s.epoch + 1 })),
  setDirty: (dirty) => set({ dirty }),
  setWizard: (wizard) => set({ wizard }),
  navigate: async (route, document = null) => {
    if (get().dirty && (await confirmAction(t('unsaved'))) === null) return false;
    set({ route, targetDocument: document, dirty: false });
    return true;
  },
}));
export function allowed(permission: string): boolean {
  return useStore.getState().bootstrap?.permissions.includes(permission) ?? false;
}
export function currentSettings() {
  return useStore.getState().bootstrap?.settings[0];
}
export function moneyDigits() {
  return Number(s(currentSettings(), 'currency_minor_digits', '2'));
}
export function arabicDigits() {
  return s(currentSettings(), 'digit_style') === 'arabic_indic';
}
export function businessDate() {
  return today(s(currentSettings(), 'timezone', 'Africa/Cairo'));
}
export function money(value: string | bigint | null | undefined) {
  return `${formatDecimal(value, moneyDigits(), arabicDigits())} ${s(currentSettings(), 'currency_symbol', t('currencySymbol'))}`;
}
export async function refreshBootstrap() {
  const b = await api.query({ type: 'bootstrap' });
  useStore.getState().setBootstrap(b);
  return b;
}
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : t('unexpected');
}
export function handleError(error: unknown) {
  if (error instanceof ApiError && error.code === 'unauthenticated') return;
  toast(errorMessage(error), 'error');
}
type Toast = { id: string; message: string; tone: 'success' | 'error' };
export const useToasts = create<{ items: Toast[]; remove: (id: string) => void }>((set) => ({
  items: [],
  remove: (id) => set((s) => ({ items: s.items.filter((x) => x.id !== id) })),
}));
export function toast(message: string, tone: Toast['tone'] = 'success') {
  const id = crypto.randomUUID();
  useToasts.setState((s) => ({ items: [...s.items.slice(-3), { id, message, tone }] }));
  setTimeout(() => useToasts.getState().remove(id), tone === 'error' ? 12000 : 5000);
}
