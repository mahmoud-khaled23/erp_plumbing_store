import {
  AreaChart,
  Area,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import type { Row } from '../../api/client';
import { s } from '../../api/client';
import { t } from '../../i18n';
import { allowed, arabicDigits, businessDate, money, moneyDigits, useStore } from '../../app/store';
import { displayDate, formatDecimal } from '../../lib/numbers';
import { useQuery } from '../../lib/hooks';
import { Button, Empty, ErrorState, Icon, Loading, PageHeader, Panel } from '../../components/ui';
import { DataTable } from '../../components/Table';
// Only chart geometry is converted to Number. All sums and displayed amounts stay exact.
const coordinate = (value: string) => Number(BigInt(value) / 10n ** BigInt(moneyDigits()));
function ChartTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: readonly { payload: Record<string, string | number>; dataKey?: string | number }[];
  label?: unknown;
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  return (
    <div className="chart-tooltip">
      <p>{String(label ?? row.name ?? '')}</p>
      {payload.map((p, i) => (
        <div key={i} className="mt-1">
          <span>
            {t(p.dataKey === 'out' ? 'cashOut' : p.dataKey === 'in' ? 'cashIn' : 'salesValue')}
            :{' '}
          </span>
          <bdi>
            {money(
              String(
                p.payload[
                  p.dataKey === 'out' ? 'out_raw' : p.dataKey === 'in' ? 'in_raw' : 'raw'
                ] ?? '0',
              ),
            )}
          </bdi>
        </div>
      ))}
    </div>
  );
}
function Trend({ rows }: { rows: Row[] }) {
  const day = businessDate();
  const end = new Date(day + 'T12:00:00Z');
  const data = Array.from({ length: 30 }, (_, i) => {
    const d = new Date(end);
    d.setUTCDate(d.getUTCDate() - 29 + i);
    const iso = d.toISOString().slice(0, 10),
      raw = s(
        rows.find((r) => r.document_date === iso),
        'sales_minor',
        '0',
      );
    return { name: displayDate(iso, arabicDigits()).slice(0, 5), value: coordinate(raw), raw };
  });
  return (
    <ResponsiveContainer width="100%" height={250}>
      <AreaChart data={data} margin={{ top: 15, right: 5, left: 10, bottom: 0 }}>
        <defs>
          <linearGradient id="salesFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#087c75" stopOpacity={0.22} />
            <stop offset="100%" stopColor="#087c75" stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid strokeDasharray="4 4" vertical={false} stroke="#e9eeee" />
        <XAxis
          dataKey="name"
          axisLine={false}
          tickLine={false}
          minTickGap={35}
          tick={{ fontSize: 11 }}
        />
        <YAxis
          axisLine={false}
          tickLine={false}
          width={62}
          tick={{ fontSize: 10 }}
          tickFormatter={(v) =>
            new Intl.NumberFormat(arabicDigits() ? 'ar-EG' : 'en', { notation: 'compact' }).format(
              v,
            )
          }
        />
        <Tooltip content={<ChartTooltip />} />
        <Area
          dataKey="value"
          type="monotone"
          stroke="#087c75"
          strokeWidth={2.5}
          fill="url(#salesFill)"
          isAnimationActive={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
function Ranking({ rows, group = false }: { rows: Row[]; group?: boolean }) {
  const sorted = [...rows].sort((a, b) => {
    const delta = BigInt(s(b, 'net_sales_minor', '0')) - BigInt(s(a, 'net_sales_minor', '0'));
    return delta > 0n ? 1 : delta < 0n ? -1 : 0;
  });
  const data = sorted.slice(0, group ? 8 : 10).map((r) => ({
    name: s(r, 'name_ar'),
    raw: s(r, 'net_sales_minor', '0'),
    value: coordinate(s(r, 'net_sales_minor', '0')),
  }));
  if (group && sorted.length > 8) {
    const raw = sorted
      .slice(8)
      .reduce((sum, row) => sum + BigInt(s(row, 'net_sales_minor', '0')), 0n)
      .toString();
    data.push({ name: t('otherCategories'), raw, value: coordinate(raw) });
  }
  return data.length ? (
    <ResponsiveContainer width="100%" height={250}>
      <BarChart data={data} layout="vertical" margin={{ right: 4, left: 8 }}>
        <XAxis type="number" hide />
        <YAxis
          type="category"
          dataKey="name"
          width={145}
          axisLine={false}
          tickLine={false}
          tick={{ fontSize: 11 }}
        />
        <Tooltip content={<ChartTooltip />} />
        <Bar
          dataKey="value"
          fill="#3aa79b"
          radius={[3, 3, 3, 3]}
          maxBarSize={20}
          isAnimationActive={false}
        />
      </BarChart>
    </ResponsiveContainer>
  ) : (
    <Empty title="chartEmpty" hint="period30" />
  );
}
function CashChart({ rows }: { rows: Row[] }) {
  const days = new Map<string, { in: bigint; out: bigint }>();
  for (const r of rows) {
    const key = s(r, 'document_date'),
      current = days.get(key) ?? { in: 0n, out: 0n };
    current.in += BigInt(s(r, 'cash_in_minor', '0'));
    current.out += BigInt(s(r, 'cash_out_minor', '0'));
    days.set(key, current);
  }
  const data = Array.from(days, ([date, v]) => ({
    name: displayDate(date, arabicDigits()).slice(0, 5),
    in: coordinate(v.in.toString()),
    out: coordinate(v.out.toString()),
    in_raw: v.in.toString(),
    out_raw: v.out.toString(),
  }));
  return data.length ? (
    <ResponsiveContainer width="100%" height={250}>
      <BarChart data={data}>
        <CartesianGrid strokeDasharray="4 4" vertical={false} stroke="#e9eeee" />
        <XAxis dataKey="name" tick={{ fontSize: 11 }} />
        <YAxis
          width={65}
          tick={{ fontSize: 10 }}
          tickFormatter={(v) =>
            new Intl.NumberFormat(arabicDigits() ? 'ar-EG' : 'en', { notation: 'compact' }).format(
              v,
            )
          }
        />
        <Tooltip content={<ChartTooltip />} />
        <Legend formatter={(value) => t(value === 'in' ? 'cashIn' : 'cashOut')} />
        <Bar dataKey="in" fill="#087c75" isAnimationActive={false} />
        <Bar dataKey="out" fill="#d6a45b" isAnimationActive={false} />
      </BarChart>
    </ResponsiveContainer>
  ) : (
    <Empty title="chartEmpty" hint="period30" />
  );
}
export default function DashboardPage() {
  const day = businessDate();
  const result = useQuery({ type: 'dashboard', date: day });
  if (result.error) return <ErrorState error={result.error} />;
  if (!result.data) return <Loading />;
  const d = result.data;
  const kpis = [
    { label: 'todaySales', value: money(s(d.sales[0], 'today_sales_minor')), icon: 'sales' },
    { label: 'monthSales', value: money(s(d.sales[0], 'month_sales_minor')), icon: 'reports' },
    { label: 'grossProfit', value: money(s(d.sales[0], 'gross_profit_minor')), icon: 'finance' },
    ...(allowed('finance.read')
      ? [
          {
            label: 'cashBalance',
            value: money(d.cash.reduce((a, r) => a + BigInt(s(r, 'balance_minor', '0')), 0n)),
            icon: 'finance',
          },
          {
            label: 'receivables',
            value: money(s(d.balances[0], 'receivable_minor')),
            icon: 'parties',
          },
          {
            label: 'payables',
            value: money(s(d.balances[0], 'payable_minor')),
            icon: 'purchasing',
          },
        ]
      : []),
    { label: 'lowStock', value: formatDecimal(d.low_stock_count, 0, arabicDigits()), icon: 'box' },
  ];
  return (
    <>
      <PageHeader title="dashboard" hint="dashboardHint">
        {allowed('sales.write') && (
          <Button tone="primary" onClick={() => useStore.getState().navigate('pos')}>
            <Icon name="plus" />
            {t('newInvoice')}
          </Button>
        )}
      </PageHeader>
      <div className="kpi-grid">
        {kpis.map((k, i) => (
          <section
            className={`kpi ${i === 0 ? 'kpi-primary' : ''} ${i >= 4 ? 'kpi-secondary' : ''}`}
            key={k.label}
          >
            <div className="flex items-center justify-between gap-2">
              <span>{t(k.label)}</span>
              <span className="kpi-icon">
                <Icon name={k.icon} size={18} />
              </span>
            </div>
            <strong>
              <bdi>{k.value}</bdi>
            </strong>
            <span className="kpi-foot">
              {i === 0 ? displayDate(day, arabicDigits()) : t(i === 6 ? 'live' : 'workspace')}
            </span>
          </section>
        ))}
      </div>
      <div className="mt-5 grid gap-5 xl:grid-cols-[1.65fr_1fr]">
        <Panel title="salesTrend">
          <Trend rows={d.sales_trend} />
        </Panel>
        <Panel title="topProducts">
          <Ranking rows={d.top_products} />
        </Panel>
      </div>
      <div className="mt-5 grid gap-5 xl:grid-cols-2">
        <Panel title="salesByCategory">
          <Ranking rows={d.categories} group />
        </Panel>
        {allowed('finance.read') && (
          <Panel title="cashFlow">
            <CashChart rows={d.cash_flow} />
          </Panel>
        )}
      </div>
      <div className="mt-5 grid gap-5 xl:grid-cols-2">
        <Panel title="recentInvoices">
          <DataTable
            compact
            rows={d.recent_invoices}
            columns={['number', 'party_name_snapshot', 'total_minor', 'status']}
            onRow={(r) => void useStore.getState().navigate('pos', s(r, 'id'))}
          />
        </Panel>
        <Panel title="lowStock">
          <DataTable
            compact
            rows={d.low_stock}
            columns={['name_ar', 'warehouse_name', 'quantity_micros', 'min_stock_micros']}
          />
        </Panel>
      </div>
      {allowed('finance.read') && (
        <div className="mt-5 grid gap-5 xl:grid-cols-2">
          <Panel title="overdue">
            <DataTable compact rows={d.overdue} columns={['name_ar', 'overdue_minor']} />
          </Panel>
          <Panel title="dueSoon">
            <DataTable
              compact
              rows={d.cheques_due}
              columns={['cheque_number', 'due_date', 'amount_minor', 'direction']}
            />
          </Panel>
        </div>
      )}
    </>
  );
}
