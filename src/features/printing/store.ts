import { create } from 'zustand';
import type { PrintSource } from '../../api/contracts';
export const usePrint = create<{ job: { id: string; source: PrintSource } | null }>(() => ({
  job: null,
}));
export function openPrint(source: PrintSource) {
  usePrint.setState({ job: { id: crypto.randomUUID(), source } });
}
export function closePrint() {
  usePrint.setState({ job: null });
}
