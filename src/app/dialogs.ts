import { create } from 'zustand';
interface Confirmation {
  message: string;
  reason: boolean;
  resolve: (answer: string | null) => void;
}
export const useConfirmation = create<{ request: Confirmation | null }>(() => ({ request: null }));
export function confirmAction(message: string, reason = false): Promise<string | null> {
  if (useConfirmation.getState().request) return Promise.resolve(null);
  return new Promise((resolve) =>
    useConfirmation.setState({ request: { message, reason, resolve } }),
  );
}
