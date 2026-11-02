import { test, expect, seed, nav, day, makeDraft } from './fixtures';
import type { Row } from '../../src/api/client';
import { t } from '../../src/i18n';

test('product planning creates reviewed purchase orders without changing stock', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  await backend.mutate({
    type: 'save_product_planning',
    planning: {
      product_id: ids.product,
      revision: '0',
      supplier_id: ids.party,
      reorder_point: '0',
      max_level: '0',
      lead_time_days: 0,
      weight_grams: '0',
    },
    reason: 'المورد الافتراضي',
  });
  await page.goto('/');
  await nav(page, t('products'));
  await page.getByRole('button', { name: t('productPlanning'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('productPlanning'), exact: true });
  await dialog.getByLabel(t('reorderPoint')).fill('100');
  await dialog.getByLabel(t('maximumStock')).fill('120');
  await dialog.getByLabel(t('supplierLeadDays')).fill('7');
  await dialog.getByLabel(t('baseWeightGrams')).fill('250');
  await dialog.getByLabel(t('reason')).fill('حدود تخطيط الصنف');
  await dialog.getByRole('button', { name: t('save'), exact: true }).click();
  await expect(page.getByText(t('saved'), { exact: true }).last()).toBeVisible();
  await dialog.getByRole('button', { name: t('close'), exact: true }).click();
  await nav(page, t('purchasing'));
  await page.getByRole('button', { name: t('suggestedPurchases'), exact: true }).click();
  await page.getByRole('checkbox', { name: t('selectForOrder'), exact: true }).check();
  await page.getByRole('button', { name: t('previewSuggestedOrders') }).click();
  await page.getByLabel(t('reason')).fill('طلب حد المخزون');
  await page.screenshot({ path: 'output/reorder-preview.png', fullPage: true });
  await page.getByRole('button', { name: t('createSuggestedOrders') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(page.getByRole('button', { name: new RegExp(t('openCreatedOrder')) })).toBeVisible();
  const docs = await backend.query<Row[]>({
    type: 'documents',
    kind: 'purchase_order',
    status: 'draft',
    after_id: '0',
    limit: 50,
  });
  expect(docs).toHaveLength(1);
  expect(docs[0].party_id).toBe(ids.party);
  const inventory = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(inventory[0].quantity_micros).toBe('100000000');
});

test('bulk prices and margin policy are previewed and POS requires an override reason', async ({
  page,
  backend,
}) => {
  await seed(backend);
  await page.goto('/');
  await nav(page, t('products'));
  await page.getByRole('button', { name: t('bulkPrices') }).click();
  const dialog = page.getByRole('dialog', { name: t('bulkPrices') });
  await dialog.getByLabel(t('minimumMargin')).fill('40');
  await dialog.getByLabel(t('reason'), { exact: true }).first().fill('حد الهامش');
  await dialog.getByRole('button', { name: t('saveMarginPolicy') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await dialog
    .getByRole('checkbox', { name: t('selectUnitForPrice') })
    .first()
    .check();
  await dialog.getByLabel(t('priceChangePercent')).fill('10');
  await dialog.getByLabel(t('reason'), { exact: true }).last().fill('زيادة الأسعار');
  await dialog.getByRole('button', { name: t('previewPrices') }).click();
  await expect(
    dialog.getByRole('columnheader', { name: t('columns.new_retail_minor') }),
  ).toBeVisible();
  await page.screenshot({ path: 'output/bulk-prices-preview.png', fullPage: true });
  await dialog.getByRole('button', { name: t('applyPrices') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog.getByRole('button', { name: t('applyPrices') })).not.toBeVisible();
  await dialog.getByRole('button', { name: t('close'), exact: true }).click();
  await nav(page, t('pos'));
  const search = page.getByRole('combobox', { name: t('searchProducts') });
  await search.fill('6220000000015');
  await search.press('Enter');
  await expect(page.getByRole('textbox', { name: t('price'), exact: true })).toHaveValue('16.50');
  await expect(page.getByText(t('marginOverrideHint'), { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'سداد الإجمالي' }).click();
  await page.keyboard.press('F9');
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(page.getByRole('alert').filter({ hasText: 'تجاوز السعر' })).toBeVisible();
  await page.getByLabel(t('reason'), { exact: true }).fill('موافقة المدير على هامش العميل');
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
  expect(sales[0].total_minor).toBe('1650');
  expect(sales[0].reason).toContain('المدير');
});

test('weight-based landed cost is previewed by Rust and printed with its allocation method', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  await backend.mutate({
    type: 'save_product_planning',
    planning: {
      product_id: ids.product,
      revision: '0',
      supplier_id: ids.party,
      reorder_point: '0',
      max_level: '0',
      lead_time_days: 0,
      weight_grams: '250',
    },
    reason: 'وزن الوحدة',
  });
  const receipt = await backend.mutate({
    type: 'save_draft',
    draft: makeDraft('goods_receipt', ids.unit, { party_id: ids.party }),
  });
  await backend.mutate({
    type: 'document_action',
    id: receipt.id!,
    revision: receipt.revision!,
    action: 'post',
    reason: 'استلام',
    acknowledge_credit: false,
  });
  await page.goto('/');
  await nav(page, t('purchasing'));
  await page.getByRole('button', { name: t('landedCost'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('landedCost'), exact: true });
  await dialog.getByRole('button', { name: t('linkedReceipt'), exact: true }).click();
  await page
    .getByRole('dialog', { name: t('selectSource') })
    .getByRole('button', { name: t('useSource'), exact: true })
    .click();
  await dialog.getByRole('button', { name: t('select'), exact: true }).click();
  await page
    .getByRole('dialog', { name: t('parties') })
    .getByRole('row', { name: /C001/ })
    .click();
  await dialog.getByLabel(t('amount'), { exact: true }).fill('1.01');
  await dialog.getByLabel(t('description'), { exact: true }).fill('شحن حسب الوزن');
  await dialog.getByLabel(t('allocateBy')).selectOption('weight');
  await dialog.getByRole('button', { name: t('previewAllocation') }).click();
  await expect(dialog.getByRole('button', { name: t('post'), exact: true })).toBeEnabled();
  await dialog.getByRole('button', { name: t('post'), exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('cell', { name: t('enums.weight'), exact: true })).toBeVisible();
  const inventory = await backend.query<Row[]>({
    type: 'inventory',
    warehouse_id: '1',
    after_id: '0',
    limit: 50,
  });
  expect(inventory[0].quantity_micros).toBe('101000000');
  expect(inventory[0].value_minor).toBe('101101');
  await page.getByRole('button', { name: t('printDocument') }).click();
  await expect(
    page
      .getByRole('dialog', { name: 'معاينة الطباعة' })
      .getByText(t('enums.weight'), { exact: true }),
  ).toBeVisible();
});
