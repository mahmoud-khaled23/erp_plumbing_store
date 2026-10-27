import { useEffect, useRef, useState } from 'react';
import { api, type QueryResult } from '../api/client';
import type { Query } from '../api/contracts';
import { handleError, useStore } from '../app/store';
export function useQuery<Q extends Query>(query: Q | null) {
  const [data, setData] = useState<QueryResult<Q>>();
  const [error, setError] = useState<unknown>();
  const [loading, setLoading] = useState(false);
  const epoch = useStore((s) => s.epoch);
  const key = JSON.stringify(query);
  useEffect(() => {
    let live = true;
    setData(undefined);
    setError(undefined);
    if (!query) {
      setLoading(false);
      return;
    }
    setLoading(true);
    api
      .query(query)
      .then((x) => {
        if (live) setData(x);
      })
      .catch((e) => {
        if (live) setError(e);
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [key, epoch]);
  return { data, error, loading };
}
export function useDebounced<T>(value: T, delay = 160) {
  const [v, set] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => set(value), delay);
    return () => clearTimeout(id);
  }, [value, delay]);
  return v;
}
export function useTask() {
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    try {
      return await fn();
    } catch (e) {
      handleError(e);
    } finally {
      locked.current = false;
      setBusy(false);
    }
  }
  return { busy, run };
}
export function useDirty(dirty: boolean) {
  useEffect(() => {
    useStore.getState().setDirty(dirty);
    return () => useStore.getState().setDirty(false);
  }, [dirty]);
}
export function useHotkeys(handler: (event: KeyboardEvent) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const callback = (e: KeyboardEvent) => ref.current(e);
    window.addEventListener('keydown', callback);
    return () => window.removeEventListener('keydown', callback);
  }, []);
}
