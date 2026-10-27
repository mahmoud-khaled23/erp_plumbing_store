import { useState } from 'react';
import { AuditJson } from '../../components/AuditValue';
import { api, s, type Row } from '../../api/client';
import { t } from '../../i18n';
import { refreshBootstrap, toast, useStore } from '../../app/store';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, Check, ErrorState, Loading, Modal, PageHeader, Panel } from '../../components/ui';
import { SmartForm, textValue } from '../../components/Form';
import { DataTable, Pager } from '../../components/Table';
export function UsersPage() {
  const result = useQuery({ type: 'users' });
  const [edit, setEdit] = useState<Row | 'new' | null>(null),
    [reset, setReset] = useState<Row | null>(null),
    [roles, setRoles] = useState<string[]>([]),
    [active, setActive] = useState(true);
  const task = useTask();
  const row = edit === 'new' ? undefined : (edit ?? undefined);
  const choose = (row: Row | 'new') => {
    setEdit(row);
    setRoles(
      row === 'new'
        ? ['2']
        : (result.data?.user_roles ?? [])
            .filter((r) => r.user_id === row.id)
            .map((r) => s(r, 'role_id')),
    );
    setActive(row === 'new' || row.active === '1');
  };
  return (
    <>
      <PageHeader title="users" hint="usersHint">
        <Button tone="primary" onClick={() => choose('new')}>
          {t('createUser')}
        </Button>
      </PageHeader>
      <Panel>
        {result.error ? (
          <ErrorState error={result.error} />
        ) : result.loading ? (
          <Loading />
        ) : (
          <DataTable
            rows={result.data?.users ?? []}
            columns={['username', 'display_name', 'active', 'must_change_password', 'locked_until']}
            actions={(r) => (
              <>
                <Button onClick={() => choose(r)}>{t('edit')}</Button>
                {r.id !== useStore.getState().bootstrap?.user_id && (
                  <Button onClick={() => setReset(r)}>{t('resetPassword')}</Button>
                )}
              </>
            )}
          />
        )}
      </Panel>
      {edit && (
        <Modal title={row ? 'edit' : 'createUser'} onClose={() => setEdit(null)} busy={task.busy}>
          {row ? (
            <>
              <h3 className="mb-4 font-bold">{s(row, 'display_name')}</h3>
              <Check
                label="active"
                checked={active}
                onChange={(e) => setActive(e.target.checked)}
              />
            </>
          ) : null}
          {row ? (
            <>
              <div className="my-4">
                {result.data?.roles.map((r) => (
                  <label className="mb-3 flex gap-2" key={s(r, 'id')}>
                    <input
                      type="checkbox"
                      checked={roles.includes(s(r, 'id'))}
                      onChange={(e) =>
                        setRoles((v) =>
                          e.target.checked ? [...v, s(r, 'id')] : v.filter((id) => id !== r.id),
                        )
                      }
                    />
                    {s(r, 'name_ar')}
                  </label>
                ))}
              </div>
              <Button
                tone="primary"
                busy={task.busy}
                disabled={!roles.length}
                onClick={() =>
                  task.run(async () => {
                    await api.mutate({
                      type: 'set_user_access',
                      user_id: s(row, 'id'),
                      active,
                      roles,
                    });
                    await refreshBootstrap();
                    useStore.getState().refresh();
                    toast(t('saved'));
                    setEdit(null);
                  })
                }
              >
                {t('save')}
              </Button>
            </>
          ) : (
            <SmartForm
              fields={[
                { name: 'username', label: 'username', required: true },
                { name: 'name', label: 'displayName', required: true },
                {
                  name: 'password',
                  label: 'password',
                  type: 'password',
                  required: true,
                  min: 12,
                  max: 256,
                },
                {
                  name: 'role',
                  label: 'roles',
                  type: 'select',
                  required: true,
                  options: result.data?.roles.map((r) => ({
                    value: s(r, 'id'),
                    label: s(r, 'name_ar'),
                  })),
                },
              ]}
              initial={{ username: '', name: '', password: '', role: '2' }}
              onSubmit={async (v) => {
                await api.createUser(
                  textValue(v, 'username'),
                  textValue(v, 'name'),
                  textValue(v, 'password'),
                  [textValue(v, 'role')],
                );
                toast(t('saved'));
                useStore.getState().refresh();
                setEdit(null);
              }}
            />
          )}
        </Modal>
      )}
      {reset && (
        <Modal title="resetPassword" onClose={() => setReset(null)}>
          <h3 className="mb-4 font-bold">{s(reset, 'display_name')}</h3>
          <SmartForm
            fields={[
              {
                name: 'password',
                label: 'newPassword',
                type: 'password',
                required: true,
                min: 12,
                max: 256,
              },
              { name: 'confirm', label: 'confirmPassword', type: 'password', required: true },
            ]}
            initial={{ password: '', confirm: '' }}
            validate={(v) => (v.password !== v.confirm ? t('passwordMismatch') : undefined)}
            onSubmit={async (v) => {
              await api.resetPassword(s(reset, 'id'), textValue(v, 'password'));
              toast(t('saved'));
              useStore.getState().refresh();
              setReset(null);
            }}
          />
        </Modal>
      )}
    </>
  );
}
export function AuditPage() {
  const [cursors, setCursors] = useState(['0']),
    [selected, setSelected] = useState<Row | null>(null);
  const result = useQuery({ type: 'audit', after_id: cursors.at(-1)!, limit: 50 });
  return (
    <>
      <PageHeader title="audit" hint="auditHint" />
      <Panel>
        {result.error ? (
          <ErrorState error={result.error} />
        ) : result.loading ? (
          <Loading />
        ) : (
          <DataTable
            rows={result.data ?? []}
            columns={['created_at', 'user_id', 'action', 'entity_type', 'entity_id', 'reason']}
            onRow={setSelected}
          />
        )}
        <Pager
          page={cursors.length - 1}
          canNext={result.data?.length === 50}
          onPrevious={() => setCursors((c) => c.slice(0, -1))}
          onNext={() => setCursors((c) => [...c, s(result.data?.at(-1), 'id')])}
        />
      </Panel>
      {selected && (
        <Modal title="details" onClose={() => setSelected(null)} wide>
          <DataTable
            rows={[selected]}
            columns={[
              'action',
              'reason',
              { key: 'before_json', render: (r) => <AuditJson text={s(r, 'before_json')} /> },
              { key: 'after_json', render: (r) => <AuditJson text={s(r, 'after_json')} /> },
            ]}
          />
        </Modal>
      )}
    </>
  );
}
