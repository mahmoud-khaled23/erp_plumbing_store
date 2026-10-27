import { it, expect, vi, beforeEach } from 'vitest';
const mocked = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocked.invoke,
  isTauri: () => true,
  convertFileSrc: (v: string) => v,
}));
import { api, ApiError } from '../../src/api/client';
beforeEach(() => {
  mocked.invoke.mockReset();
});
it('reuses a request ID after an uncertain transport result', async () => {
  mocked.invoke.mockRejectedValueOnce(new Error('transport')).mockResolvedValueOnce({ id: '1' });
  const mutation = { type: 'open_shift' as const, cash_account_id: '9007199254740993' };
  await expect(api.mutate(mutation)).rejects.toBeInstanceOf(ApiError);
  await api.mutate(mutation);
  const first = mocked.invoke.mock.calls[0][1].request,
    second = mocked.invoke.mock.calls[1][1].request;
  expect(second.request_id).toBe(first.request_id);
  expect(second.mutation.cash_account_id).toBe('9007199254740993');
});
it('does not silently accept numbers in database result fields', async () => {
  mocked.invoke.mockResolvedValue([{ id: 3, name_ar: 'test' }]);
  await expect(
    api.query({ type: 'products', search: '', after_id: '0', limit: 50 }),
  ).rejects.toMatchObject({ code: 'contract' });
});
it('preserves the structured Arabic backend error', async () => {
  mocked.invoke.mockRejectedValue({
    code: 'credit_warning',
    message: 'تأكيد الحد الائتماني مطلوب',
    retryable: false,
  });
  await expect(api.mutate({ type: 'open_shift', cash_account_id: '1' })).rejects.toMatchObject({
    code: 'credit_warning',
    message: 'تأكيد الحد الائتماني مطلوب',
    retryable: false,
  });
});
