import { Component, Suspense, lazy, useEffect, useState, type ReactNode } from 'react';
import { api, s } from '../api/client';
import { t } from '../i18n';
import {
  allowed,
  arabicDigits,
  businessDate,
  errorMessage,
  handleError,
  refreshBootstrap,
  useStore,
  type Route,
} from './store';
import { confirmAction } from './dialogs';
import { displayDate } from '../lib/numbers';
import { useHotkeys, useTask } from '../lib/hooks';
import { Button, ConfirmHost, ErrorState, Icon, Loading, Modal, ToastHost } from '../components/ui';
import AuthPage, { OpeningWizard, PasswordDialog, ReauthDialog } from '../features/auth/AuthPage';
import { usePrint, closePrint } from '../features/printing/store';
const PrintHost = lazy(() => import('../features/printing/PrintHost'));
const Dashboard = lazy(() => import('../features/dashboard/DashboardPage'));
const Products = lazy(() => import('../features/products/ProductsPage'));
const Parties = lazy(() => import('../features/parties/PartiesPage'));
const Inventory = lazy(() => import('../features/inventory/InventoryPage'));
const Documents = lazy(() =>
  import('../features/documents/DocumentsPage').then((m) => ({ default: m.DocumentsPage })),
);
const Pos = lazy(() =>
  import('../features/documents/DocumentsPage').then((m) => ({ default: m.PosPage })),
);
const Finance = lazy(() => import('../features/finance/FinancePage'));
const Reports = lazy(() => import('../features/reports/ReportsPage'));
const Settings = lazy(() => import('../features/settings/SettingsPage'));
const Users = lazy(() =>
  import('../features/settings/UsersPage').then((m) => ({ default: m.UsersPage })),
);
const Audit = lazy(() =>
  import('../features/settings/UsersPage').then((m) => ({ default: m.AuditPage })),
);
const navigation: { route: Route; permission: string[]; icon: string; group: string }[] = [
  { route: 'dashboard', permission: ['dashboard.read'], icon: 'dashboard', group: 'workspace' },
  { route: 'pos', permission: ['sales.write'], icon: 'pos', group: 'operations' },
  { route: 'products', permission: ['products.read'], icon: 'box', group: 'operations' },
  { route: 'inventory', permission: ['inventory.read'], icon: 'inventory', group: 'operations' },
  { route: 'sales', permission: ['sales.read'], icon: 'sales', group: 'operations' },
  { route: 'purchasing', permission: ['purchases.read'], icon: 'purchasing', group: 'operations' },
  { route: 'receiving', permission: ['inventory.read'], icon: 'receiving', group: 'operations' },
  { route: 'parties', permission: ['parties.read'], icon: 'parties', group: 'operations' },
  {
    route: 'finance',
    permission: ['finance.read', 'payments.write', 'cash.shift'],
    icon: 'finance',
    group: 'accounting',
  },
  { route: 'reports', permission: ['reports.read'], icon: 'reports', group: 'accounting' },
  {
    route: 'settings',
    permission: ['settings.write', 'periods.lock', 'backup.create'],
    icon: 'settings',
    group: 'system',
  },
  { route: 'users', permission: ['users.manage'], icon: 'users', group: 'system' },
  { route: 'audit', permission: ['audit.read'], icon: 'audit', group: 'system' },
];
class Boundary extends Component<{ children: ReactNode }, { error: unknown }> {
  state = { error: null };
  static getDerivedStateFromError(error: unknown) {
    return { error };
  }
  render() {
    return this.state.error ? (
      <div className="p-8">
        <ErrorState error={this.state.error} />
        <Button
          onClick={() => {
            this.setState({ error: null });
            useStore.setState({ route: 'dashboard', dirty: false });
          }}
        >
          {t('back')}
        </Button>
      </div>
    ) : (
      this.props.children
    );
  }
}
function Screen({ route }: { route: Route }) {
  switch (route) {
    case 'dashboard':
      return <Dashboard />;
    case 'pos':
      return <Pos />;
    case 'products':
      return <Products />;
    case 'inventory':
      return <Inventory />;
    case 'sales':
    case 'purchasing':
    case 'receiving':
      return <Documents key={route} module={route} />;
    case 'parties':
      return <Parties />;
    case 'finance':
      return <Finance />;
    case 'reports':
      return <Reports />;
    case 'settings':
      return <Settings />;
    case 'users':
      return <Users />;
    case 'audit':
      return <Audit />;
  }
}
export default function App() {
  const printJob = usePrint((s) => s.job);
  const b = useStore((s) => s.bootstrap),
    route = useStore((s) => s.route);
  const [error, setError] = useState<unknown>(),
    [password, setPassword] = useState(false),
    [help, setHelp] = useState(false),
    [collapsed, setCollapsed] = useState(false),
    [expired, setExpired] = useState(false);
  const task = useTask();
  useEffect(() => {
    if (!b) return;
    let live = true;
    void document.fonts.ready.then(() => {
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (live) void api.frontendReady().catch(() => {});
        }),
      );
    });
    return () => {
      live = false;
    };
  }, [b]);
  useEffect(() => {
    if (!b?.authenticated) closePrint();
  }, [b?.authenticated]);
  useEffect(() => {
    const expired = () => setExpired(true);
    window.addEventListener('erp-session-expired', expired);
    return () => window.removeEventListener('erp-session-expired', expired);
  }, []);
  useEffect(() => {
    document.title = t('app');
    refreshBootstrap().catch(setError);
  }, []);
  useEffect(() => {
    const before = (e: BeforeUnloadEvent) => {
      if (useStore.getState().dirty) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', before);
    return () => window.removeEventListener('beforeunload', before);
  }, []);
  useHotkeys((e) => {
    if (!b?.authenticated || document.querySelector('dialog[open]')) return;
    if (e.key === 'F2' && allowed('sales.write')) {
      e.preventDefault();
      if (route !== 'pos') void useStore.getState().navigate('pos');
    }
    if ((e.ctrlKey && e.key.toLowerCase() === 'k') || (e.key === 'F4' && route !== 'pos')) {
      e.preventDefault();
      document.querySelector<HTMLInputElement>('[data-page-search],[data-product-search]')?.focus();
    }
  });
  const nav = navigation.filter((n) => n.permission.some(allowed));
  const permitted = nav.some((n) => n.route === route);
  return (
    <Boundary>
      <ConfirmHost />
      <ToastHost />
      {printJob && b?.authenticated && (
        <Suspense fallback={null}>
          <PrintHost />
        </Suspense>
      )}
      {!b ? (
        error ? (
          <div className="flex min-h-screen items-center justify-center bg-canvas p-8">
            <div className="max-w-lg">
              <h1 className="mb-5 text-2xl font-bold">{t('app')}</h1>
              <p className="mb-5 text-slate-500">{errorMessage(error)}</p>
              <Button
                tone="primary"
                onClick={() => {
                  setError(undefined);
                  void refreshBootstrap().catch(setError);
                }}
              >
                {t('retry')}
              </Button>
            </div>
          </div>
        ) : (
          <Loading />
        )
      ) : !b.authenticated ? (
        <AuthPage />
      ) : (
        <div className={`app-shell ${collapsed ? 'sidebar-collapsed' : ''}`}>
          <aside className="sidebar">
            <div className="sidebar-brand">
              <span className="brand-mark">
                <Icon name="inventory" size={24} />
              </span>
              <div className="brand-copy">
                <strong>{s(b.settings[0], 'store_name', t('app'))}</strong>
                <span>{t('workspace')}</span>
              </div>
            </div>
            <nav aria-label={t('workspace')}>
              {['workspace', 'operations', 'accounting', 'system'].map((group) => (
                <div className="nav-group" key={group}>
                  <p>{t(group)}</p>
                  {nav
                    .filter((n) => n.group === group)
                    .map((n) => (
                      <button
                        aria-label={t(n.route)}
                        title={t(n.route)}
                        key={n.route}
                        className={`nav-item ${route === n.route ? 'active' : ''}`}
                        aria-current={route === n.route ? 'page' : undefined}
                        onClick={() => n.route !== route && useStore.getState().navigate(n.route)}
                      >
                        <Icon name={n.icon} />
                        <span>{t(n.route)}</span>
                        {n.route === 'pos' && <kbd>F2</kbd>}
                      </button>
                    ))}
                </div>
              ))}
            </nav>
            <div className="sidebar-footer">
              <span className="status-dot" />
              <span>{t('offline')}</span>
            </div>
          </aside>
          <div className="workspace-main">
            <header className="topbar">
              <div className="flex items-center gap-3">
                <Button
                  tone="ghost"
                  aria-label={t('workspace')}
                  onClick={() => setCollapsed((v) => !v)}
                >
                  <Icon name="menu" />
                </Button>
                <span className="text-sm text-slate-500">
                  {t('workspace')}
                  <span className="mx-3 text-slate-300">/</span>
                  <strong className="font-semibold text-ink">{t(route)}</strong>
                </span>
              </div>
              <div className="flex items-center gap-3">
                <span className="hidden text-xs text-slate-500 md:block">
                  <bdi>{displayDate(businessDate(), arabicDigits())}</bdi>
                </span>
                <Button tone="ghost" onClick={() => setHelp(true)} aria-label={t('help')}>
                  ?
                </Button>
                <button
                  className="user-chip"
                  onClick={() => setPassword(true)}
                  title={t('changePassword')}
                >
                  <span>{b.display_name?.slice(0, 1) || t('app').slice(0, 1)}</span>
                  <strong>{b.display_name}</strong>
                </button>
                <Button
                  tone="ghost"
                  busy={task.busy}
                  aria-label={t('logout')}
                  onClick={() =>
                    task.run(async () => {
                      if (
                        (await confirmAction(
                          t(useStore.getState().dirty ? 'unsaved' : 'closeSession'),
                        )) === null
                      )
                        return;
                      await api.logout();
                      useStore.setState({
                        bootstrap: null,
                        dirty: false,
                        targetDocument: null,
                        route: 'dashboard',
                      });
                      await refreshBootstrap();
                    })
                  }
                >
                  <Icon name="logout" />
                </Button>
              </div>
            </header>
            <main id="main" className="page-content">
              <Suspense fallback={<Loading />}>
                {permitted ? <Screen route={route} /> : <p role="alert">{t('noPermission')}</p>}
              </Suspense>
            </main>
          </div>
          <OpeningWizard />
          {expired && <ReauthDialog onDone={() => setExpired(false)} />}
          {(password || b.must_change_password) && (
            <PasswordDialog forced={b.must_change_password} onClose={() => setPassword(false)} />
          )}{' '}
          {help && (
            <Modal title="help" onClose={() => setHelp(false)}>
              <dl className="space-y-4">
                {[
                  ['F2', 'shortcutPos'],
                  ['F4 / Ctrl+K', 'shortcutSearch'],
                  ['F8', 'shortcutSave'],
                  ['F9', 'shortcutPost'],
                ].map(([key, label]) => (
                  <div className="flex justify-between" key={key}>
                    <dt>{t(label)}</dt>
                    <dd>
                      <kbd>{key}</kbd>
                    </dd>
                  </div>
                ))}
              </dl>
            </Modal>
          )}
        </div>
      )}
    </Boundary>
  );
}
