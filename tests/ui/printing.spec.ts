import { mkdir, readFile, writeFile } from 'node:fs/promises';
import type { Page } from '@playwright/test';
import { test, expect, seed, nav, makeDraft, day, type Driver } from './fixtures';
import type { Bootstrap, DocumentDetail, Row } from '../../src/api/client';
import { settingsInput } from '../../src/features/settings/settingsModel';
import { formatDecimal } from '../../src/lib/numbers';

async function post(backend: Driver, draft: ReturnType<typeof makeDraft>) {
  const saved = await backend.mutate({ type: 'save_draft', draft });
  await backend.mutate({
    type: 'document_action',
    id: saved.id!,
    revision: saved.revision!,
    action: 'post',
    reason: 'اختبار الطباعة',
    acknowledge_credit: false,
  });
  return saved.id!;
}
async function capture(page: Page, name: string) {
  await expect(
    page
      .getByRole('dialog', { name: 'معاينة الطباعة' })
      .getByRole('button', { name: 'حفظ PDF', exact: true }),
  ).toBeEnabled();
  await expect(page.locator('#erp-print-root [role="alert"]')).toHaveCount(0);
  await page.emulateMedia({ media: 'print' });
  const geometry = await page.locator('[data-print-page]').evaluateAll((pages) =>
    pages.map((page) => {
      const footer = page.querySelector('.print-footer')?.getBoundingClientRect();
      const flow = page.querySelector('.print-flow')?.getBoundingClientRect();
      const box = page.getBoundingClientRect();
      return {
        fits: !footer || !flow || flow.bottom <= footer.top + 1,
        width: box.width,
        overflows: page.scrollWidth > page.clientWidth + 1,
      };
    }),
  );
  expect(geometry.length).toBeGreaterThan(0);
  expect(geometry.every((g) => g.fits && !g.overflows)).toBe(true);
  await mkdir('output/pdf', { recursive: true });
  await mkdir('output/print-fixtures', { recursive: true });
  await page.pdf({
    path: `output/pdf/${name}.pdf`,
    preferCSSPageSize: true,
    printBackground: true,
    displayHeaderFooter: false,
  });
  const html = await page.evaluate(() => ({
    body: document.getElementById('erp-print-root')!.outerHTML,
    css: Array.from(document.styleSheets)
      .flatMap((sheet) =>
        Array.from(sheet.cssRules)
          .filter((rule) => rule.type !== CSSRule.FONT_FACE_RULE)
          .map((rule) => rule.cssText),
      )
      .join('\n'),
  }));
  let fonts = await readFile('node_modules/@fontsource-variable/cairo/wght.css', 'utf8');
  for (const match of [...fonts.matchAll(/url\((?:['"])?\.\/files\/([^)'"\s]+)(?:['"])?\)/g)]) {
    const data = await readFile(`node_modules/@fontsource-variable/cairo/files/${match[1]}`);
    fonts = fonts.replace(match[0], `url(data:font/woff2;base64,${data.toString('base64')})`);
  }
  await writeFile(
    `output/print-fixtures/${name}.html`,
    `<!doctype html><html lang="ar" dir="rtl"><head><meta charset="utf-8"><style>${fonts}\n${html.css}</style></head><body>${html.body}</body></html>`,
  );
  await page.emulateMedia({ media: 'screen' });
  await page
    .locator('[data-print-page]')
    .first()
    .screenshot({ path: `output/pdf/${name}.png` });
}

test('A4 invoice pagination, exact totals, historical profile and 80mm receipt', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  const bootstrap = await backend.query<Bootstrap>({ type: 'bootstrap' });
  await backend.mutate({
    type: 'save_settings',
    settings: {
      ...settingsInput(bootstrap.settings[0]),
      address: 'القاهرة - شارع الجمهورية - عمارة ٢٤',
      phones: '01012345678',
      tax_number: '123-456-789',
      arabic_digits: true,
      vat_enabled: true,
      vat_bps: 1400,
    },
  });
  const draft = makeDraft('sale', ids.unit, {
    party_id: ids.party,
    discount: '175',
    notes: 'يرجى مراجعة المقاسات عند الاستلام. الأصناف مطابقة لأمر العميل.',
    payments: [
      {
        method: 'cash',
        cash_account_id: '1',
        shift_id: null,
        amount: '2500',
        reference: 'دفعة مقدمة',
        cheque: null,
      },
    ],
  });
  draft.lines = Array.from({ length: 38 }, (_, i) => ({
    ...draft.lines[0],
    quantity: '1250000',
    tax_bps: 1400,
    discount: '5',
    notes: `البند ${i + 1} - مواسير مياه مقاومة للضغط مع مراجعة المقاس قبل التركيب`,
  }));
  const id = await post(backend, draft);
  const next = await backend.query<Bootstrap>({ type: 'bootstrap' });
  await backend.mutate({
    type: 'save_settings',
    settings: {
      ...settingsInput(next.settings[0]),
      store_name: 'اسم جديد لا يظهر على الفاتورة السابقة',
    },
  });
  await page.goto('/');
  await nav(page, 'المبيعات');
  await page.getByRole('row', { name: /شركة النور/ }).click();
  await page.getByRole('button', { name: 'طباعة المستند' }).click();
  await expect(page.locator('[data-print-page]')).not.toHaveCount(0);
  const detail = await backend.query<DocumentDetail>({ type: 'document', id });
  await expect(page.locator('.print-grand-total td').last()).toHaveText(
    `${formatDecimal(detail.header[0].total_minor, 2, true)} ج.م`,
  );
  const cells = page.locator('#erp-print-root [data-flow-id^="lines:"]');
  await expect(cells).toHaveCount(38);
  await expect(
    page.locator('#erp-print-root').getByText('المنار لأدوات السباكة', { exact: true }).first(),
  ).toBeVisible();
  await expect(
    page
      .locator('#erp-print-root')
      .getByText('اسم جديد لا يظهر على الفاتورة السابقة', { exact: true }),
  ).toHaveCount(0);
  expect(await page.locator('[data-print-page]').count()).toBeGreaterThan(1);
  await capture(page, 'invoice-a4');
  await page.getByRole('button', { name: 'إغلاق المعاينة' }).click();
  const current = await backend.query<Bootstrap>({ type: 'bootstrap' });
  await backend.mutate({
    type: 'save_settings',
    settings: { ...settingsInput(current.settings[0]), store_name: 'المنار لأدوات السباكة' },
  });
  const receiptId = await post(
    backend,
    makeDraft('sale', ids.unit, {
      party_id: ids.party,
      lines: [{ ...draft.lines[0], quantity: '2500000', notes: 'ماسورة مقطوعة حسب المقاس' }],
      payments: [],
    }),
  );
  await nav(page, 'نقطة البيع');
  await nav(page, 'المبيعات');
  await page
    .getByRole('row', { name: /شركة النور/ })
    .last()
    .click();
  await page.getByRole('button', { name: 'طباعة المستند' }).click();
  await page.getByRole('combobox', { name: 'مقاس الطباعة' }).selectOption('thermal80');
  await capture(page, 'receipt-80mm');
  const width = await page
    .locator('[data-print-page]')
    .first()
    .evaluate((el) => el.getBoundingClientRect().width);
  expect(Math.abs(width - (80 * 96) / 25.4)).toBeLessThan(1);
});

test('goods receipt, whole report beyond one UI page, and printable barcode labels', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  const draft = makeDraft('goods_receipt', ids.unit, {
    party_id: ids.party,
    notes: 'تم استلام البضاعة وفحصها بالمخزن',
    lines: [
      {
        ...makeDraft('goods_receipt', ids.unit).lines[0],
        quantity: '7500000',
        damaged: '500000',
        short: '2000000',
        notes: 'تالف منفصل عن الكمية المقبولة',
      },
    ],
  });
  await post(backend, draft);
  await page.goto('/');
  await nav(page, 'استلام البضاعة');
  await page.getByRole('row', { name: /شركة النور/ }).click();
  await page.getByRole('button', { name: 'طباعة المستند' }).click();
  await capture(page, 'goods-received');
  await page.getByRole('button', { name: 'إغلاق المعاينة' }).click();
  // More than 100 statement entries ensures the print snapshot includes all screen pages.
  for (let i = 0; i < 105; i++)
    await backend.mutate({
      type: 'voucher',
      date: day,
      party_id: ids.party,
      supplier: false,
      outgoing: false,
      payments: [
        {
          method: 'cash',
          cash_account_id: '1',
          shift_id: null,
          amount: '1',
          reference: '',
          cheque: null,
        },
      ],
      notes: `دفعة مراجعة ${i + 1}`,
    });
  await nav(page, 'التقارير');
  await page
    .getByRole('combobox', { name: 'التقرير', exact: true })
    .selectOption('party_statement');
  await page.getByRole('button', { name: 'عميل نقدي', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'العملاء والموردون' })
    .getByRole('row', { name: /C001/ })
    .click();
  await page.getByRole('button', { name: 'طباعة التقرير' }).click();
  await expect(page.locator('#erp-print-root [data-flow-id^="report:"]')).toHaveCount(105);
  await capture(page, 'report-a4');
  await page.getByRole('button', { name: 'إغلاق المعاينة' }).click();
  await nav(page, 'الأصناف');
  await page.getByRole('row', { name: /PPR20/ }).click();
  await page.getByRole('button', { name: 'طباعة ملصقات كل الوحدات' }).click();
  await page.getByRole('spinbutton', { name: 'ملصقات لكل وحدة' }).fill('15');
  await expect(page.locator('[data-print-page]')).toHaveCount(2);
  await capture(page, 'barcode-labels');
  await expect(page.locator('.barcode-label svg')).toHaveCount(30);
});
