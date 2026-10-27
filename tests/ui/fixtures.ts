import { test as base, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createInterface } from 'node:readline';
import type { Draft, Mutation, Query } from '../../src/api/contracts';
import type { Bootstrap, Operation, Row } from '../../src/api/client';
export class Driver {
  process: ChildProcessWithoutNullStreams;
  private next = 0;
  private pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (v: unknown) => void }
  >();
  constructor() {
    this.process = spawn(
      'docker',
      [
        'run',
        '--rm',
        '-i',
        '--user',
        `${process.getuid?.() ?? 1000}:${process.getgid?.() ?? 1000}`,
        '-v',
        `${process.cwd()}:/work:Z`,
        'rust:1.95-slim@sha256:e14e87345b4d5964ddcc3491d27ee046a0f23820f340c3c1e24da6880141f7c0',
        '/work/output/target/debug/examples/ui_driver',
      ],
      { stdio: 'pipe' },
    );
    createInterface({ input: this.process.stdout }).on('line', (line) => {
      const response = JSON.parse(line);
      const p = this.pending.get(response.id);
      this.pending.delete(response.id);
      if (response.error) p?.reject(response.error);
      else p?.resolve(response.value);
    });
    let stderr = '';
    this.process.stderr.on('data', (v) => {
      stderr = (stderr + String(v)).slice(-4000);
    });
    this.process.on('exit', () => {
      for (const p of this.pending.values()) p.reject(new Error(stderr || 'UI driver closed'));
      this.pending.clear();
    });
  }
  call<T = unknown>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.process.stdin.write(JSON.stringify({ id, command, args }) + '\n');
    });
  }
  mutate(mutation: Mutation) {
    return this.call<Operation>('erp_mutate', {
      request: { request_id: crypto.randomUUID(), mutation },
    });
  }
  query<T = Row[]>(query: Query) {
    return this.call<T>('erp_query', { query });
  }
  async close() {
    this.process.stdin.end();
    await new Promise<void>((resolve) => {
      if (this.process.exitCode !== null) resolve();
      else this.process.once('exit', () => resolve());
    });
  }
}
export const day = new Intl.DateTimeFormat('en-CA', {
  timeZone: 'Africa/Cairo',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
}).format(new Date());
export const password = 'CorrectHorse12!';
export async function seed(backend: Driver) {
  await backend.call('erp_setup', {
    input: {
      store_name: 'المنار لأدوات السباكة',
      username: 'admin',
      display_name: 'مدير المتجر',
      password,
      fiscal_year: Number(day.slice(0, 4)),
      demo: false,
    },
  });
  await backend.call('erp_login', { username: 'admin', password });
  const product = await backend.mutate({
    type: 'save_product',
    product: {
      id: null,
      revision: '0',
      code: 'PPR20',
      name: 'ماسورة مياه PPR ٢٠ مم',
      category_id: '1',
      brand: 'محلي',
      material: 'PPR',
      size: '٢٠ مم',
      base_unit_id: '1',
      standard_cost: '1000',
      min_stock: '2000000',
      active: true,
      units: [
        {
          id: null,
          unit_id: '1',
          numerator: '1',
          denominator: '1',
          barcode: '6220000000015',
          retail_price: '1500',
          wholesale_price: '1400',
          active: true,
        },
        {
          id: null,
          unit_id: '2',
          numerator: '12',
          denominator: '1',
          barcode: '6220000000022',
          retail_price: '18000',
          wholesale_price: '16800',
          active: true,
        },
      ],
    },
  });
  const detail = await backend.query<{ units: Row[] }>({ type: 'product', id: product.id! });
  const party = await backend.mutate({
    type: 'save_party',
    party: {
      id: null,
      code: 'C001',
      name: 'شركة النور للمقاولات',
      customer: true,
      supplier: true,
      phone: '01000000000',
      address: 'القاهرة',
      tax_number: '',
      credit_limit: null,
      terms_days: 30,
      active: true,
      notes: '',
    },
  });
  const unit = detail.units[0].id!;
  const opening = await backend.mutate({
    type: 'save_draft',
    draft: makeDraft('opening_stock', unit, {
      date: day.slice(0, 4) + '-01-01',
      lines: [
        {
          unit_id: unit,
          quantity: '100000000',
          price: '1000',
          discount: '0',
          tax_bps: 0,
          order_line_id: null,
          damaged: '0',
          short: '0',
          matches: [],
          notes: '',
        },
      ],
    }),
  });
  await backend.mutate({
    type: 'document_action',
    id: opening.id!,
    revision: opening.revision!,
    action: 'post',
    reason: 'رصيد بداية الاختبار',
    acknowledge_credit: false,
  });
  return { product: product.id!, unit, party: party.id! };
}
export function makeDraft(
  kind: Draft['kind'],
  unit: string,
  overrides: Partial<Draft> = {},
): Draft {
  return {
    id: null,
    revision: '0',
    kind,
    date: day,
    due_date: null,
    party_id: null,
    warehouse_id: '1',
    destination_id: null,
    source_id: null,
    wholesale: false,
    discount: '0',
    lines: [
      {
        unit_id: unit,
        quantity: '1000000',
        price: kind === 'sale' ? '1500' : '1000',
        discount: '0',
        tax_bps: 0,
        order_line_id: null,
        damaged: '0',
        short: '0',
        matches: [],
        notes: '',
      },
    ],
    payments: [],
    notes: '',
    reason: '',
    ...overrides,
  };
}
export const test = base.extend<{ backend: Driver }>({
  backend: async ({}, use) => {
    const driver = new Driver();
    await driver.query<Bootstrap>({ type: 'bootstrap' });
    try {
      await use(driver);
    } finally {
      await driver.close();
    }
  },
  page: async ({ page, backend }, use) => {
    await page.exposeFunction(
      '__ERP_TEST_INVOKE__',
      (command: string, args: Record<string, unknown>) => backend.call(command, args),
    );
    await page.addInitScript(() => {
      Object.defineProperty(window, 'isTauri', { value: true });
      Object.defineProperty(window, '__TAURI_INTERNALS__', {
        value: {
          invoke: (command: string, args: Record<string, unknown>) =>
            (
              window as unknown as {
                __ERP_TEST_INVOKE__: (c: string, a: Record<string, unknown>) => Promise<unknown>;
              }
            ).__ERP_TEST_INVOKE__(command, args),
        },
      });
    });
    await use(page);
  },
});
export { expect };
export async function nav(page: Page, label: string) {
  await page.getByRole('navigation').getByRole('button', { name: label, exact: true }).click();
}
