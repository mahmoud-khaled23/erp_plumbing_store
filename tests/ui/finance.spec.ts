import { test, expect, seed, nav, day, makeDraft } from './fixtures';
import type { Row } from '../../src/api/client';
import { t } from '../../src/i18n';

test('year-end preview posts, locks and displays the closing journal', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend),
    year = Number(day.slice(0, 4));
  await backend.mutate({
    type: 'fiscal_period',
    id: null,
    name: String(year + 1),
    starts_on: `${year + 1}-01-01`,
    ends_on: `${year + 1}-12-31`,
    locked: false,
    reason: 'الفترة التالية',
  });
  const sale = await backend.mutate({
    type: 'save_draft',
    draft: makeDraft('sale', ids.unit, { party_id: ids.party }),
  });
  await backend.mutate({
    type: 'document_action',
    id: sale.id!,
    revision: sale.revision!,
    action: 'post',
    reason: 'بيع',
    acknowledge_credit: false,
  });
  await page.goto('/');
  await nav(page, t('finance'));
  await page.getByRole('tab', { name: t('yearClosing'), exact: true }).click();
  await page.getByRole('button', { name: t('previewClosing') }).click();
  await expect(page.getByText(t('closingResult'), { exact: false })).toBeVisible();
  await page.getByLabel(t('reason'), { exact: true }).fill('مراجعة وإقفال السنة');
  await page.getByRole('button', { name: t('postYearClosing') }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(page.getByRole('heading', { name: t('financeContext'), exact: true })).toBeVisible();
  const docs = await backend.query<Row[]>({
    type: 'documents',
    kind: 'year_end_close',
    status: 'posted',
    after_id: '0',
    limit: 50,
  });
  expect(docs).toHaveLength(1);
  await page.screenshot({ path: 'output/finance-closing.png', fullPage: true });
});

test('customer debt adjustment uses the chosen receivable and prints its reference', async ({
  page,
  backend,
}) => {
  const ids = await seed(backend);
  const sale = await backend.mutate({
    type: 'save_draft',
    draft: makeDraft('sale', ids.unit, { party_id: ids.party }),
  });
  await backend.mutate({
    type: 'document_action',
    id: sale.id!,
    revision: sale.revision!,
    action: 'post',
    reason: 'بيع',
    acknowledge_credit: false,
  });
  await page.goto('/');
  await nav(page, t('finance'));
  await page.getByRole('tab', { name: t('customerAdjustments'), exact: true }).click();
  await page.getByRole('button', { name: t('customerAdjustment'), exact: true }).click();
  const dialog = page.getByRole('dialog', { name: t('customerAdjustment'), exact: true });
  await dialog.getByLabel(t('amount'), { exact: true }).fill('5.00');
  await dialog.getByLabel(t('reason'), { exact: true }).fill('تعذر تحصيل جزء من الدين');
  await dialog.getByRole('button', { name: t('post'), exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(page.getByRole('heading', { name: t('financeContext'), exact: true })).toBeVisible();
  const open = await backend.query<Row[]>({
    type: 'customer_open_items',
    search: '',
    after_id: '0',
    limit: 50,
  });
  expect(open[0].outstanding_signed_minor).toBe('1000');
  await page.getByRole('button', { name: t('printDocument'), exact: true }).click();
  await expect(
    page
      .getByRole('dialog', { name: 'معاينة الطباعة' })
      .getByText(t('financeContext'), { exact: true }),
  ).toBeVisible();
});

test('CSV bank statement matches and unmatches a real cash journal line', async ({
  page,
  backend,
}) => {
  await seed(backend);
  await backend.mutate({
    type: 'journal',
    date: day,
    opening: false,
    notes: 'إيداع اختبار',
    lines: [
      { account_id: '101', party_id: null, due_date: null, amount: '1500', memo: 'إيداع' },
      { account_id: '303', party_id: null, due_date: null, amount: '-1500', memo: 'مقابل' },
    ],
  });
  await page.goto('/');
  await nav(page, t('finance'));
  await page.getByRole('tab', { name: t('bankReconciliation'), exact: true }).click();
  await page.getByRole('button', { name: t('newBankStatement') }).click();
  const dialog = page.getByRole('dialog', { name: t('newBankStatement') });
  await dialog.getByLabel(t('cashAccount'), { exact: true }).selectOption('1');
  await dialog.getByLabel(t('reference'), { exact: true }).first().fill('كشف اختبار');
  await dialog.getByLabel(t('statementClosing')).fill('15.00');
  await dialog.getByLabel(t('importStatementCsv')).setInputFiles({
    name: 'statement.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from(`date,amount,reference,description\n${day},15.00,A,إيداع اختبار\n`),
  });
  await expect(dialog.getByRole('cell', { name: 'إيداع اختبار', exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: t('save'), exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  await page.getByRole('cell', { name: 'كشف اختبار', exact: true }).click();
  await page.getByRole('button', { name: t('selectForMatch') }).click();
  await page.getByRole('button', { name: t('matchStatement'), exact: true }).click();
  const match = page.getByRole('dialog', { name: t('matchStatement'), exact: true });
  await match.getByLabel(t('reason')).fill('مطابقة الإيداع');
  await match.getByRole('button', { name: t('confirm'), exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(match).not.toBeVisible();
  await page.getByRole('tab', { name: t('reconciliationTabs.matches') }).click();
  await page.getByRole('button', { name: t('unmatchStatement') }).click();
  const undo = page.getByRole('dialog', { name: t('unmatchStatement') });
  await undo.getByLabel(t('reason')).fill('مراجعة المطابقة');
  await undo.getByRole('button', { name: t('confirm'), exact: true }).click();
  await page
    .getByRole('dialog', { name: 'تأكيد العملية' })
    .getByRole('button', { name: t('confirm'), exact: true })
    .click();
  await expect(undo).not.toBeVisible();
  const statements = await backend.query<Row[]>({
    type: 'bank_statements',
    after_id: '0',
    limit: 50,
  });
  const result = await backend.query<{ rows: Row[] }>({
    type: 'reconciliation',
    statement_id: statements[0].id!,
    view: 'statement',
    limit: 50,
    offset: 0,
  });
  expect(result.rows[0].remaining_minor).toBe('1500');
  await page.screenshot({ path: 'output/finance-reconciliation.png', fullPage: true });
});
