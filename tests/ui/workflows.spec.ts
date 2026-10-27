import { test, expect, seed, nav, day, password, makeDraft } from './fixtures';
import type { DocumentDetail, Row } from '../../src/api/client';

test('first run, validation and offline RTL font', async ({ page, backend }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'إعداد المتجر' })).toBeVisible();
  await page.getByLabel('اسم المتجر', { exact: true }).fill('المنار لأدوات السباكة');
  await page.getByLabel('اسم العرض', { exact: true }).fill('مدير المتجر');
  await page.getByLabel('اسم المستخدم', { exact: true }).fill('admin');
  await page.getByLabel('كلمة المرور', { exact: true }).fill(password);
  await page.getByLabel('تأكيد كلمة المرور', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'إعداد المتجر', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'الأرصدة الافتتاحية' })).toBeVisible();
  await page.getByRole('button', { name: 'الانتقال إلى المتجر' }).click();
  await expect(page.getByRole('heading', { name: 'نظرة عامة' })).toBeVisible();
  expect(await page.locator('html').getAttribute('dir')).toBe('rtl');
  await page.evaluate(() => document.fonts.ready);
  expect(await page.evaluate(() => document.fonts.check('16px "Cairo Variable"'))).toBe(true);
  const requests: string[] = [];
  page.on('request', (r) => requests.push(r.url()));
  await nav(page, 'الأصناف');
  expect(requests.every((u) => u.startsWith('http://127.0.0.1:5173'))).toBe(true);
});

test('barcode sale, fractional quantity, post and partial return', async ({ page, backend }) => {
  const ids = await seed(backend);
  await page.goto('/');
  await nav(page, 'نقطة البيع');
  await page.getByRole('combobox', { name: 'اسم الصنف، الكود أو الباركود' }).fill('6220000000015');
  await page.getByRole('combobox', { name: 'اسم الصنف، الكود أو الباركود' }).press('Enter');
  await expect(page.getByRole('cell', { name: /ماسورة مياه/ })).toBeVisible();
  await page.getByRole('textbox', { name: 'الكمية', exact: true }).fill('2.5');
  await page.getByRole('button', { name: 'سداد الإجمالي' }).click();
  await page.keyboard.press('F9');
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(page.getByText('تم الترحيل بنجاح', { exact: true })).toBeVisible();
  const rows = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(rows[0].quantity_micros).toBe('97500000');
  await page.getByRole('button', { name: 'مرتجع مرتبط', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'مرتجع مرتبط' });
  await dialog.getByLabel('سبب العملية').fill('مرتجع جزء من الكمية');
  await dialog.getByRole('textbox', { name: /كمية المرتجع ماسورة/ }).fill('0.5');
  await dialog.getByRole('button', { name: 'سداد الإجمالي' }).click();
  await dialog.getByRole('button', { name: 'ترحيل', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  const updated = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(updated[0].quantity_micros).toBe('98000000');
  const returns = await backend.query<Row[]>({
    type: 'documents',
    kind: 'sale_return',
    status: 'posted',
    after_id: '0',
    limit: 50,
  });
  expect(returns[0].total_minor).toBe('750');
});

test('catalogue units persist and navigation guards an unsaved invoice', async ({
  page,
  backend,
}) => {
  await seed(backend);
  await page.goto('/');
  await nav(page, 'الأصناف');
  await page.getByRole('button', { name: 'تعديل', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'تعديل الصنف' });
  await expect(dialog.getByLabel('الكود', { exact: true })).toHaveValue('PPR20');
  await dialog.getByLabel('العلامة التجارية').fill('علامة جديدة');
  await dialog.getByRole('button', { name: 'حفظ', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await nav(page, 'نقطة البيع');
  const search = page.getByRole('combobox', { name: 'اسم الصنف، الكود أو الباركود' });
  await search.fill('6220000000022');
  await search.press('Enter');
  await expect(page.getByRole('textbox', { name: 'السعر', exact: true })).toHaveValue('180.00');
  await nav(page, 'نقطة البيع');
  await page.keyboard.press('F2');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await nav(page, 'الأصناف');
  await expect(page.getByRole('dialog', { name: 'تأكيد العملية' })).toBeVisible();
  await page.getByRole('dialog').getByRole('button', { name: 'إلغاء', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'نقطة البيع' })).toBeVisible();
  await page.keyboard.press('F8');
  await expect(page.getByText('تم الحفظ بنجاح', { exact: true }).last()).toBeVisible();
  await nav(page, 'المبيعات');
  await expect(page.getByRole('cell', { name: 'مسودة', exact: true })).toBeVisible();
});

test('partial purchase-order receipt and purchase matching through UI', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  const po = await backend.mutate({
    type: 'save_draft',
    draft: makeDraft('purchase_order', ids.unit, {
      party_id: ids.party,
      lines: [
        {
          unit_id: ids.unit,
          quantity: '5000000',
          price: '1100',
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
    id: po.id!,
    revision: po.revision!,
    action: 'approve',
    reason: '',
    acknowledge_credit: false,
  });
  await page.goto('/');
  await nav(page, 'استلام البضاعة');
  await page.getByRole('button', { name: 'جديد', exact: true }).click();
  await page.getByRole('button', { name: 'أمر الشراء المرتبط' }).click();
  await page.getByRole('button', { name: 'استخدام المصدر' }).click();
  await page.getByRole('textbox', { name: 'الكمية', exact: true }).fill('2');
  await page.keyboard.press('F9');
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(page.getByText('تم الترحيل بنجاح', { exact: true })).toBeVisible();
  const progress = await backend.query<Row[]>({
    type: 'report',
    report: 'purchase_orders',
    from: day,
    to: day,
    party_id: null,
    limit: 100,
    offset: 0,
  });
  expect(progress[0].progress).toBe('partially_received');
  await nav(page, 'المشتريات');
  await page.getByRole('tab', { name: 'فاتورة مشتريات', exact: true }).click();
  await page.getByRole('button', { name: 'جديد', exact: true }).click();
  await page.getByRole('button', { name: 'إذن الاستلام المرتبط' }).click();
  await page.getByRole('button', { name: 'استخدام المصدر' }).click();
  await expect(page.getByRole('dialog', { name: 'اختيار مستند مصدر' })).not.toBeVisible();
  await expect(page.getByRole('textbox', { name: 'الكمية', exact: true })).toHaveValue('2.000000');
  await page.keyboard.press('F9');
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'المشتريات' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'مرتجع مرتبط', exact: true })).toBeVisible();
  const stock = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(stock[0].quantity_micros).toBe('102000000');
});

test('stocktake saves actual count and posts the difference', async ({ page, backend }) => {
  const ids = await seed(backend);
  await page.goto('/');
  await nav(page, 'المخزون');
  await page.getByRole('button', { name: 'بدء جرد' }).click();
  const dialog = page.getByRole('dialog', { name: 'بدء جرد' });
  await dialog.getByLabel('كل الأصناف').check();
  await dialog.getByLabel('سبب العملية').fill('جرد نهاية الوردية');
  await dialog.getByRole('button', { name: 'بدء جرد', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await page.getByRole('textbox', { name: /الكمية الفعلية ماسورة/ }).fill('98');
  await page.getByRole('button', { name: 'ترحيل', exact: true }).last().click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(page.getByText('تم الترحيل بنجاح', { exact: true })).toBeVisible();
  const stock = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(stock[0].quantity_micros).toBe('98000000');
});

test('all module screens and Arabic settings render without page errors', async ({
  page,
  backend,
}) => {
  await seed(backend);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  for (const label of [
    'الأصناف',
    'المخزون',
    'المبيعات',
    'المشتريات',
    'استلام البضاعة',
    'العملاء والموردون',
    'الخزينة والحسابات',
    'التقارير',
    'الإعدادات',
    'المستخدمون',
    'سجل المراجعة',
    'نظرة عامة',
  ]) {
    await nav(page, label);
    await expect(page.locator('main h1')).toBeVisible();
    await expect(page.getByText('جارٍ التحميل', { exact: true })).toHaveCount(0);
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  await page.screenshot({ path: 'output/ui-dashboard.png', fullPage: true });
  await nav(page, 'نقطة البيع');
  await page.screenshot({ path: 'output/ui-pos.png', fullPage: true });
  expect(errors).toEqual([]);
});

test('queued barcode scans and session renewal preserve the invoice', async ({ page, backend }) => {
  await seed(backend);
  await page.goto('/');
  await nav(page, 'نقطة البيع');
  await page.evaluate(() => {
    const ipc = (
      window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args: any) => Promise<unknown> };
      }
    ).__TAURI_INTERNALS__;
    const invoke = ipc.invoke;
    ipc.invoke = async (command, args) => {
      if (command === 'erp_query' && ['product', 'products'].includes(args.query.type)) {
        await new Promise((resolve) => setTimeout(resolve, 120));
      }
      return invoke(command, args);
    };
  });
  const search = page.getByRole('combobox', { name: 'اسم الصنف، الكود أو الباركود' });
  for (let i = 0; i < 5; i++) {
    await search.fill('6220000000015');
    await search.press('Enter');
  }
  const quantity = page.getByRole('textbox', { name: 'الكمية', exact: true });
  await expect(quantity).toHaveValue('5.000000');
  await backend.call('erp_logout');
  await page.keyboard.press('F8');
  const dialog = page.getByRole('dialog', { name: 'إعادة تسجيل الدخول' });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel('اسم المستخدم').fill('admin');
  await dialog.getByLabel('كلمة المرور', { exact: true }).fill(password);
  await dialog.getByRole('button', { name: 'تسجيل الدخول', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(quantity).toHaveValue('5.000000');
  await page.keyboard.press('F8');
  await expect(page.getByText('تم الحفظ بنجاح', { exact: true })).toBeVisible();
  const drafts = await backend.query<Row[]>({
    type: 'documents',
    kind: 'sale',
    status: 'draft',
    after_id: '0',
    limit: 50,
  });
  expect(drafts).toHaveLength(1);
  const detail = await backend.query<DocumentDetail>({ type: 'document', id: drafts[0].id! });
  expect(detail.lines[0].quantity_micros).toBe('5000000');
});

test('customer advance and paid expense post balanced journals', async ({ page, backend }) => {
  await seed(backend);
  await page.goto('/');
  await nav(page, 'الخزينة والحسابات');
  await page.getByRole('button', { name: 'سند قبض أو صرف', exact: true }).click();
  const voucher = page.getByRole('dialog', { name: 'سند قبض أو صرف' });
  await voucher.getByRole('button', { name: 'اختر', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'العملاء والموردون' })
    .getByRole('row', { name: /C001/ })
    .click();
  await voucher.getByRole('button', { name: 'إضافة دفعة' }).click();
  await voucher.getByLabel('المبلغ', { exact: true }).fill('100');
  await voucher.getByRole('button', { name: 'ترحيل', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(voucher).not.toBeVisible();
  await page.getByRole('button', { name: 'مصروف', exact: true }).click();
  const expense = page.getByRole('dialog', { name: 'مصروف', exact: true });
  await expense.getByLabel('المبلغ', { exact: true }).fill('12.50');
  await expense.getByLabel('الوصف', { exact: true }).fill('نقل بضاعة');
  await expense.getByRole('button', { name: 'إضافة دفعة' }).click();
  await expense.getByLabel('المبلغ', { exact: true }).last().fill('12.50');
  await expense.getByRole('button', { name: 'ترحيل', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: 'تأكيد', exact: true })
    .click();
  await expect(expense).not.toBeVisible();
  const desk = await backend.query<{ balances: Row[] }>({ type: 'cash_desk' });
  expect(desk.balances.find((r) => r.cash_account_id === '1')?.balance_minor).toBe('8750');
  for (const kind of ['receipt_voucher', 'expense']) {
    const docs = await backend.query<Row[]>({
      type: 'documents',
      kind,
      status: 'posted',
      after_id: '0',
      limit: 50,
    });
    expect(docs).toHaveLength(1);
    const detail = await backend.query<DocumentDetail>({ type: 'document', id: docs[0].id! });
    expect(
      detail.journal.reduce(
        (sum, line) => sum + BigInt(line.debit_minor!) - BigInt(line.credit_minor!),
        0n,
      ),
    ).toBe(0n);
    expect(detail.journal.length).toBeGreaterThan(1);
  }
});
