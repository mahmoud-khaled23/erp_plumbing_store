import { useState } from 'react';
import { api, s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { allowed, moneyDigits, toast, useStore } from '../../app/store';
import { decimal, editDecimal } from '../../lib/numbers';
import { useDebounced, useQuery } from '../../lib/hooks';
import { Button, ErrorState, Icon, Loading, Modal, PageHeader, Panel } from '../../components/ui';
import { SmartForm, textValue, type FormField } from '../../components/Form';
import { DataTable, Pager } from '../../components/Table';
import { ReportView } from '../reports/ReportsPage';
const fields: FormField[] = [
  { name: 'code', label: 'code', required: true },
  { name: 'name', label: 'name', required: true },
  { name: 'customer', label: 'customer', type: 'check' },
  { name: 'supplier', label: 'supplier', type: 'check' },
  { name: 'phone', label: 'phone' },
  { name: 'address', label: 'address' },
  { name: 'tax_number', label: 'taxNumber' },
  { name: 'credit_limit', label: 'creditLimit', type: 'number', digits: 3, hint: 'unlimited' },
  { name: 'terms_days', label: 'paymentTerms', type: 'number', required: true },
  { name: 'active', label: 'active', type: 'check' },
  { name: 'notes', label: 'notes', type: 'textarea', span: true },
];
export default function PartiesPage() {
  const [search, setSearch] = useState(''),
    [cursors, setCursors] = useState(['0']),
    [edit, setEdit] = useState<Row | 'new' | null>(null),
    [statement, setStatement] = useState<Row | null>(null);
  const q = useDebounced(search);
  const result = useQuery({ type: 'parties', search: q, after_id: cursors.at(-1)!, limit: 50 });
  const p = edit === 'new' ? undefined : (edit ?? undefined);
  return (
    <>
      <PageHeader title="parties" hint="partyHint">
        {allowed('parties.write') && (
          <Button tone="primary" onClick={() => setEdit('new')}>
            <Icon name="plus" />
            {t('newParty')}
          </Button>
        )}
      </PageHeader>
      <Panel>
        <div className="search-field mb-4">
          <Icon name="search" />
          <input
            aria-label={t('searchParties')}
            placeholder={t('searchParties')}
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setCursors(['0']);
            }}
          />
        </div>
        {result.error ? (
          <ErrorState error={result.error} />
        ) : result.loading ? (
          <Loading />
        ) : (
          <DataTable
            rows={result.data ?? []}
            columns={[
              'code',
              'name_ar',
              'phone',
              'is_customer',
              'is_supplier',
              'credit_limit_minor',
              'active',
            ]}
            actions={(r) => (
              <>
                {allowed('parties.write') && (
                  <Button onClick={() => setEdit(r)}>{t('edit')}</Button>
                )}
                {allowed('reports.read') && (
                  <Button onClick={() => setStatement(r)}>{t('statement')}</Button>
                )}
              </>
            )}
          />
        )}
        <Pager
          page={cursors.length - 1}
          canNext={result.data?.length === 50}
          onPrevious={() => setCursors((c) => c.slice(0, -1))}
          onNext={() => setCursors((c) => [...c, s(result.data?.at(-1), 'id')])}
        />
      </Panel>
      {edit && (
        <Modal title={p ? 'editParty' : 'newParty'} onClose={() => setEdit(null)} wide>
          <SmartForm
            fields={fields}
            initial={{
              code: s(p, 'code'),
              name: s(p, 'name_ar'),
              customer: p ? s(p, 'is_customer') === '1' : true,
              supplier: s(p, 'is_supplier') === '1',
              phone: s(p, 'phone'),
              address: s(p, 'address'),
              tax_number: s(p, 'tax_number'),
              credit_limit:
                p?.credit_limit_minor == null
                  ? ''
                  : editDecimal(p.credit_limit_minor, moneyDigits()),
              terms_days: s(p, 'payment_terms_days', '0'),
              active: p ? s(p, 'active') === '1' : true,
              notes: s(p, 'notes'),
            }}
            onSubmit={async (v) => {
              await api.mutate({
                type: 'save_party',
                party: {
                  id: p ? s(p, 'id') : null,
                  code: textValue(v, 'code'),
                  name: textValue(v, 'name'),
                  customer: !!v.customer,
                  supplier: !!v.supplier,
                  phone: textValue(v, 'phone'),
                  address: textValue(v, 'address'),
                  tax_number: textValue(v, 'tax_number'),
                  credit_limit: textValue(v, 'credit_limit').trim()
                    ? decimal(textValue(v, 'credit_limit'), moneyDigits())
                    : null,
                  terms_days: Number(decimal(textValue(v, 'terms_days'), 0)),
                  active: !!v.active,
                  notes: textValue(v, 'notes'),
                },
              });
              toast(t('saved'));
              useStore.getState().refresh();
              setEdit(null);
            }}
          />
        </Modal>
      )}
      {statement && (
        <Modal title="statement" onClose={() => setStatement(null)} wide>
          <h3 className="mb-4 font-bold">{s(statement, 'name_ar')}</h3>
          <ReportView
            initial="party_statement"
            partyId={s(statement, 'id')}
            partyName={s(statement, 'name_ar')}
          />
        </Modal>
      )}
    </>
  );
}
