import { useState } from 'react';
import { allowed } from '../../app/store';
import { t } from '../../i18n';
import { Button } from '../../components/ui';
import { ImportDialog } from '../../components/ImportDialog';
export function OpeningImports() {
  const [kind, setKind] = useState<'products' | 'stock' | 'balances' | null>(null);
  return (
    <>
      <p className="mb-4 text-sm leading-7 text-slate-500">{t('openingImportHint')}</p>
      <div className="flex flex-wrap gap-3">
        {allowed('products.import') && (
          <Button onClick={() => setKind('products')}>{t('importProducts')}</Button>
        )}
        {allowed('inventory.adjust') && (
          <Button onClick={() => setKind('stock')}>{t('openingStock')}</Button>
        )}
        {allowed('finance.write') && (
          <Button onClick={() => setKind('balances')}>{t('openingBalance')}</Button>
        )}
      </div>
      {kind && <ImportDialog kind={kind} onClose={() => setKind(null)} />}
    </>
  );
}
