import { test, expect, seed, nav } from './fixtures';
import type { DocumentDetail, ProductDetail, Row } from '../../src/api/client';
import { t } from '../../src/i18n';

test('variant grid copies warranty and units without creating stock', async ({ page, backend }) => {
  const ids = await seed(backend);
  await page.goto('/');
  await nav(page, t('products'));
  await page.getByRole('button', { name: t('plumbingProduct'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('plumbingProduct'), exact: true });
  await dialog.getByLabel(t('warrantyDays')).fill('365');
  await dialog.getByLabel(t('reason'), { exact: true }).fill('ضمان المورد');
  await dialog.getByRole('button', { name: t('saveWarranty') }).click();
  await expect(page.getByText(t('saved'), { exact: true }).last()).toBeVisible();
  await dialog.getByLabel(t('variantSizes')).fill('٢٥ مم، ٣٢ مم');
  await dialog.getByLabel(t('variantMaterials')).fill('PPR, PVC');
  await dialog.getByLabel(t('variantCodePrefix')).fill('PIPE');
  await dialog.getByRole('button', { name: t('generateVariantGrid') }).click();
  await dialog.getByLabel(`${t('barcode')} 1`, { exact: true }).fill('6220000000039');
  await dialog.getByRole('button', { name: t('previewVariants') }).click();
  await expect(dialog.getByRole('button', { name: t('applyVariants') })).toBeEnabled();
  await page.screenshot({ path: 'output/plumbing-variants.png', fullPage: true });
  await dialog.getByRole('button', { name: t('applyVariants') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog.getByRole('button', { name: t('applyVariants') })).not.toBeVisible();
  const family = await backend.query<Row[]>({
    type: 'product_variants',
    template_id: ids.product,
    after_id: '0',
    limit: 50,
  });
  expect(family).toHaveLength(4);
  const detail = await backend.query<ProductDetail>({ type: 'product', id: family[0].product_id! });
  expect(detail.product[0].warranty_days).toBe('365');
  expect(detail.units).toHaveLength(2);
  const inventory = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(inventory.find((r) => r.product_id === ids.product)?.quantity_micros).toBe('100000000');
  expect(
    inventory.filter((r) => r.product_id !== ids.product).every((r) => r.quantity_micros === '0'),
  ).toBe(true);
});

test('bundle editor expands fractional kits into normal POS lines and frozen printed warranties', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  await backend.mutate({
    type: 'set_warranty',
    product_id: ids.product,
    revision: '0',
    days: 365,
    reason: 'ضمان المورد',
  });
  await page.goto('/');
  await nav(page, t('products'));
  await page.getByRole('button', { name: t('salesBundles'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('salesBundles'), exact: true });
  await dialog.getByRole('button', { name: t('newBundle') }).click();
  await dialog.getByLabel(t('code'), { exact: true }).fill('KIT-1');
  await dialog.getByLabel(t('name'), { exact: true }).fill('طقم تغذية مزدوج');
  const search = dialog.getByRole('combobox', { name: t('searchProducts') });
  await search.fill('6220000000015');
  await search.press('Enter');
  await dialog.getByLabel(`${t('componentQuantity')} 1`, { exact: true }).fill('2');
  await dialog.getByLabel(t('reason'), { exact: true }).fill('طقم تركيب');
  await dialog.getByRole('button', { name: t('save'), exact: true }).click();
  await expect(dialog.getByRole('cell', { name: 'KIT-1', exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: t('close'), exact: true }).click();
  await nav(page, t('pos'));
  await page.getByRole('button', { name: t('addBundle'), exact: true }).click();
  const picker = page.getByRole('dialog', { name: t('addBundle'), exact: true });
  await picker.getByRole('button', { name: t('select'), exact: true }).click();
  await picker.getByLabel(t('bundleQuantity')).fill('1.5');
  await picker.getByRole('button', { name: t('previewBundle') }).click();
  await expect(picker.getByRole('button', { name: t('addBundleComponents') })).toBeVisible();
  await page.screenshot({ path: 'output/plumbing-bundle.png', fullPage: true });
  await picker.getByRole('button', { name: t('addBundleComponents') }).click();
  await expect(picker).not.toBeVisible();
  await expect(page.getByRole('textbox', { name: t('quantity'), exact: true })).toHaveValue(
    '3.000000',
  );
  await page.getByRole('button', { name: 'سداد الإجمالي' }).click();
  await page.keyboard.press('F9');
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(page.getByText(t('postedSuccess'), { exact: true })).toBeVisible();
  const sales = await backend.query<Row[]>({
    type: 'documents',
    kind: 'sale',
    status: 'posted',
    after_id: '0',
    limit: 50,
  });
  expect(sales).toHaveLength(1);
  expect(sales[0].total_minor).toBe('4500');
  const document = await backend.query<DocumentDetail>({ type: 'document', id: sales[0].id! });
  expect(document.lines[0].quantity_micros).toBe('3000000');
  expect(document.lines[0].notes).toContain('طقم تغذية مزدوج');
  await backend.mutate({
    type: 'set_warranty',
    product_id: ids.product,
    revision: '1',
    days: 730,
    reason: 'تغيير لاحق للضمان',
  });
  await page.getByRole('button', { name: t('printDocument'), exact: true }).click();
  const print = page.getByRole('dialog', { name: t('printPreview'), exact: true });
  await expect(print.locator('[data-print-page]').first()).toContainText(
    `${t('warrantyDays')}: 365`,
  );
  await expect(print.getByRole('button', { name: t('savePdf'), exact: true })).toBeEnabled();
  await page.emulateMedia({ media: 'print' });
  await print
    .locator('[data-print-page]')
    .first()
    .screenshot({ path: 'output/plumbing-warranty-a4.png' });
  await page.emulateMedia({ media: 'screen' });
  await print.getByLabel(t('printLayout')).selectOption('thermal80');
  await expect(print.locator('[data-print-page]').first()).toContainText(
    `${t('warrantyDays')}: 365`,
  );
  await expect(print.getByRole('button', { name: t('savePdf'), exact: true })).toBeEnabled();
  await page.emulateMedia({ media: 'print' });
  const fits = await print.locator('[data-print-page]').evaluateAll((pages) =>
    pages.every((page) => {
      const footer = page.querySelector('.print-footer')!.getBoundingClientRect();
      const flow = page.querySelector('.print-flow')!.getBoundingClientRect();
      return flow.bottom <= footer.top + 1 && page.scrollWidth <= page.clientWidth + 1;
    }),
  );
  expect(fits).toBe(true);
  await print
    .locator('[data-print-page]')
    .first()
    .screenshot({ path: 'output/plumbing-warranty-thermal.png' });
  const inventory = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(inventory[0].quantity_micros).toBe('97000000');
  expect(inventory[0].value_minor).toBe('97000');
});

test('shelf locations and quarantine isolation/release preserve total stock and cost', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  const warehouse = await backend.mutate({
    type: 'save_master',
    master: {
      type: 'warehouse',
      id: null,
      code: 'Q',
      name: 'فحص التالف',
      address: '',
      active: true,
    },
  });
  await page.goto('/');
  await nav(page, t('inventory'));
  await page.getByRole('button', { name: t('warehouseProduct'), exact: true }).click();
  const rules = page.getByRole('dialog', { name: t('warehouseProduct'), exact: true });
  await rules.getByLabel(t('shelf'), { exact: true }).fill('رف ٣');
  await rules.getByLabel(t('bin'), { exact: true }).fill('خانة ب');
  await rules.getByRole('button', { name: t('save'), exact: true }).click();
  await expect(rules).not.toBeVisible();
  const p = await backend.query<ProductDetail>({ type: 'product', id: ids.product });
  expect(p.warehouse_settings[0].shelf_location).toBe('رف ٣');
  expect(p.warehouse_settings[0].bin_location).toBe('خانة ب');
  await page.getByRole('button', { name: t('quarantineWorkflow'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('quarantineWorkflow'), exact: true });
  await dialog.getByLabel(t('warehouse'), { exact: true }).selectOption(warehouse.id!);
  await dialog.getByRole('checkbox', { name: t('quarantineWarehouseTag') }).check();
  await dialog.getByLabel(t('reason'), { exact: true }).first().fill('مكان فحص البضاعة');
  await dialog.getByRole('button', { name: t('saveQuarantineWarehouse') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(
    dialog
      .getByLabel(t('destination'), { exact: true })
      .getByRole('option', { name: /فحص التالف/ }),
  ).toHaveCount(1);
  await dialog.getByLabel(t('sourceWarehouse'), { exact: true }).selectOption('1');
  await dialog.getByLabel(t('destination'), { exact: true }).selectOption(warehouse.id!);
  const search = dialog.getByRole('combobox', { name: t('searchProducts') });
  await search.fill('6220000000015');
  await search.press('Enter');
  await dialog.getByLabel(`${t('componentQuantity')} 1`, { exact: true }).fill('2');
  await dialog.getByLabel(t('reason'), { exact: true }).last().fill('فحص تلف العبوة');
  await page.screenshot({ path: 'output/plumbing-quarantine.png', fullPage: true });
  await dialog.getByRole('button', { name: t('postQuarantineTransfer') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  await page.getByRole('button', { name: t('printDocument'), exact: true }).click();
  const print = page.getByRole('dialog', { name: t('printPreview'), exact: true });
  await expect(print.locator('[data-print-page]').first()).toContainText(t('quarantineGoods'));
  await print.getByRole('button', { name: t('closePreview'), exact: true }).click();
  await nav(page, t('inventory'));
  await page.getByRole('button', { name: t('quarantineWorkflow'), exact: true }).click();
  await dialog.getByLabel(t('quarantineDirection')).selectOption('release');
  await dialog.getByLabel(t('sourceWarehouse'), { exact: true }).selectOption(warehouse.id!);
  await dialog.getByLabel(t('destination'), { exact: true }).selectOption('1');
  await search.fill('6220000000015');
  await search.press('Enter');
  await dialog.getByLabel(t('reason'), { exact: true }).last().fill('اجتاز الفحص');
  await dialog.getByRole('button', { name: t('postQuarantineTransfer') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  const stock = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: null,
    after_id: '0',
    limit: 50,
  });
  expect(stock[0]).toMatchObject({
    quantity_micros: '100000000',
    available_quantity_micros: '99000000',
    quarantine_quantity_micros: '1000000',
    value_minor: '100000',
  });
});
