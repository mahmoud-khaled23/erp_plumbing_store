import { useState } from 'react';
import { api, s } from '../../api/client';
import { t } from '../../i18n';
import { refreshBootstrap, toast, useStore } from '../../app/store';
import { decimal, today } from '../../lib/numbers';
import { Button, Icon, Modal } from '../../components/ui';
import { SmartForm, textValue, type FormField } from '../../components/Form';
import { settingsInput } from '../settings/settingsModel';
import { OpeningImports } from '../settings/OpeningImports';
import { useTask } from '../../lib/hooks';
import { confirmAction } from '../../app/dialogs';
export function PasswordDialog({
  forced = false,
  onClose,
}: {
  forced?: boolean;
  onClose: () => void;
}) {
  return (
    <Modal title="changePassword" onClose={forced ? () => {} : onClose}>
      <p className="mb-4 text-sm text-slate-500">{t(forced ? 'forceChange' : 'passwordHint')}</p>
      <SmartForm
        fields={[
          { name: 'old', label: 'oldPassword', type: 'password', required: true },
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
        initial={{ old: '', password: '', confirm: '' }}
        submit="changePassword"
        validate={(v) => (v.password !== v.confirm ? t('passwordMismatch') : undefined)}
        onSubmit={async (v) => {
          await api.password(textValue(v, 'old'), textValue(v, 'password'));
          useStore.setState({ bootstrap: null, dirty: false });
          await refreshBootstrap();
          toast(t('passwordChanged'));
          onClose();
        }}
      />
    </Modal>
  );
}
export function OpeningWizard() {
  const open = useStore((s) => s.wizard);
  return open ? (
    <Modal title="setupOpening" wide onClose={() => useStore.getState().setWizard(false)}>
      <OpeningImports />
      <div className="mt-8 flex justify-end">
        <Button tone="primary" onClick={() => useStore.getState().setWizard(false)}>
          {t('setupFinish')}
        </Button>
      </div>
    </Modal>
  ) : null;
}
export function ReauthDialog({ onDone }: { onDone: () => void }) {
  const userId = useStore.getState().bootstrap?.user_id;
  const task = useTask();
  return (
    <Modal title="reauth" onClose={() => {}}>
      <p className="mb-4 text-sm text-slate-500">{t('reauthHint')}</p>
      <SmartForm
        fields={[
          { name: 'username', label: 'username', required: true },
          { name: 'password', label: 'password', type: 'password', required: true },
        ]}
        initial={{ username: '', password: '' }}
        submit="login"
        onSubmit={async (v) => {
          const user = await api.login(textValue(v, 'username'), textValue(v, 'password'));
          if (user.id !== userId) {
            await api.logout();
            throw new Error(t('differentUser'));
          }
          await refreshBootstrap();
          onDone();
        }}
      />
      <Button
        className="mt-4 w-full"
        busy={task.busy}
        onClick={() =>
          task.run(async () => {
            if (useStore.getState().dirty && (await confirmAction(t('unsaved'))) === null) return;
            await api.logout();
            useStore.setState({
              bootstrap: null,
              dirty: false,
              targetDocument: null,
              route: 'dashboard',
            });
            onDone();
            await refreshBootstrap();
          })
        }
      >
        {t('logout')}
      </Button>
    </Modal>
  );
}
export default function AuthPage() {
  const b = useStore((s) => s.bootstrap);
  const setup = !!b?.setup_required,
    recovery = !!b?.recovery_required;
  const [setupStep, setSetupStep] = useState(1);
  const fields: FormField[] = [
    ...(setup
      ? [
          { name: 'store_name', label: 'storeName', required: true },
          { name: 'address', label: 'address' },
          { name: 'phones', label: 'phones' },
          { name: 'tax_number', label: 'taxNumber' },
          { name: 'display_name', label: 'displayName', required: true },
          { name: 'year', label: 'fiscalYear', type: 'number' as const, required: true },
        ]
      : []),
    { name: 'username', label: 'username', required: true },
    {
      name: 'password',
      label: 'password',
      type: 'password',
      required: true,
      ...(setup ? { min: 12, max: 256 } : {}),
    },
    ...(setup
      ? [
          { name: 'confirm', label: 'confirmPassword', type: 'password' as const, required: true },
          { name: 'demo', label: 'demo', type: 'check' as const, hint: 'demoHint' },
        ]
      : []),
  ];
  return (
    <div className="auth-shell">
      <aside className="auth-brand">
        <div className="brand-mark">
          <Icon name="inventory" size={32} />
        </div>
        <h1>{t('app')}</h1>
        <p>{t('tagline')}</p>
        <div className="auth-pipe" aria-hidden="true">
          <div />
          <div />
          <div />
        </div>
        <span className="offline-pill">
          <span />
          {t('offline')}
        </span>
      </aside>
      <main className="auth-main">
        <div className={`auth-card ${setup ? 'auth-card-wide' : ''}`}>
          <span className="eyebrow">{t('workspace')}</span>
          <h2>{t(recovery ? 'recoverTitle' : setup ? 'setup' : 'welcome')}</h2>
          <p className="mb-7 text-sm leading-7 text-slate-500">
            {t(recovery ? 'recoverHint' : setup ? 'setupHint' : 'loginHint')}
          </p>
          {recovery && (
            <p role="alert" className="mb-5 rounded-lg bg-amber-50 p-4 text-sm text-amber-900">
              {b?.message}
            </p>
          )}
          <SmartForm
            key={setup ? 'setup' : recovery ? 'recovery' : 'login'}
            fields={fields}
            initial={{
              store_name: '',
              address: '',
              phones: '',
              tax_number: '',
              display_name: '',
              year: today().slice(0, 4),
              username: '',
              password: '',
              confirm: '',
              demo: false,
            }}
            submit={recovery ? 'restore' : setup ? 'setup' : 'login'}
            validate={(v) =>
              setup && v.password !== v.confirm ? t('passwordMismatch') : undefined
            }
            onSubmit={async (v) => {
              const username = textValue(v, 'username'),
                password = textValue(v, 'password');
              if (recovery) {
                const r = await api.recover(username, password);
                if (!r.cancelled) await refreshBootstrap();
                return;
              }
              if (setup)
                await api.setup({
                  store_name: textValue(v, 'store_name'),
                  username,
                  display_name: textValue(v, 'display_name'),
                  password,
                  fiscal_year: Number(decimal(textValue(v, 'year'), 0)),
                  demo: !!v.demo,
                });
              await api.login(username, password);
              const loaded = await refreshBootstrap();
              if (setup) {
                await api.mutate({
                  type: 'save_settings',
                  settings: {
                    ...settingsInput(loaded.settings[0]),
                    address: textValue(v, 'address'),
                    phones: textValue(v, 'phones'),
                    tax_number: textValue(v, 'tax_number'),
                  },
                });
                await refreshBootstrap();
                useStore.getState().setWizard(true);
              }
            }}
          />
          <div className="mt-7 border-t border-slate-100 pt-5 text-xs text-slate-400">
            {t('offline')}
          </div>
        </div>
      </main>
    </div>
  );
}
