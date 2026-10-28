import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import JsBarcode from 'jsbarcode';
import type { PrintPaper } from '../../api/contracts';
import { s, type PrintSnapshot } from '../../api/client';
import { t } from '../../i18n';
import { printFormat, type PrintModel, type PrintSection, type PrintLayout } from './model';
import { paginate, type MeasuredRow } from './paginate';

const PX_PER_MM = 96 / 25.4;
function TableSection({ section, indices }: { section: PrintSection; indices?: number[] }) {
  const rows = indices ?? section.rows.map((_, i) => i);
  return (
    <table className="print-table" data-group={section.id}>
      {section.widths && (
        <colgroup>
          {section.widths.map((width, i) => (
            <col key={i} style={{ width: `${width}%` }} />
          ))}
        </colgroup>
      )}
      <thead>
        {section.title && (
          <tr>
            <th colSpan={section.rows[0]?.length ?? 1} className="print-section-title">
              {section.title}
            </th>
          </tr>
        )}
        {!!section.headers.length && (
          <tr>
            {section.headers.map((h, i) => (
              <th key={i}>{h}</th>
            ))}
          </tr>
        )}
      </thead>
      <tbody>
        {rows.map((i) => (
          <tr
            key={i}
            data-flow-id={`${section.id}:${i}`}
            className={
              section.id === 'totals' && section.rows[i][0] === t('total')
                ? 'print-grand-total'
                : ''
            }
          >
            {section.rows[i].map((cell, k) => (
              <td key={k}>
                <span className="print-cell-text">{cell || '\u00a0'}</span>
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
function Header({
  model,
  snapshot,
  logo,
}: {
  model: PrintModel;
  snapshot: PrintSnapshot;
  logo: string;
}) {
  return (
    <header className="print-header">
      <div className="print-brand-row">
        <div className="print-brand">
          {logo && <img src={logo} alt={t('storeLogo')} />}
          <div>
            <h1>{s(snapshot.store, 'store_name')}</h1>
            <p>{s(snapshot.store, 'address')}</p>
            <p dir="auto">{s(snapshot.store, 'phones')}</p>
            {snapshot.store.tax_number && (
              <p>
                {t('taxNumber')}: <bdi>{snapshot.store.tax_number}</bdi>
              </p>
            )}
          </div>
        </div>
        <div className="print-title">
          <h2>{model.title}</h2>
          <p>
            <bdi>{model.number}</bdi>
          </p>
          <p>{model.status}</p>
        </div>
      </div>
      {model.warning && <div className="print-warning">{model.warning}</div>}
      <dl className="print-metadata">
        {model.metadata.map(([label, value], i) => (
          <div key={i}>
            <dt>{label}</dt>
            <dd>
              <bdi>{value}</bdi>
            </dd>
          </div>
        ))}
      </dl>
    </header>
  );
}
function Footer({
  model,
  snapshot,
  page,
  pages,
  copy,
  copies,
}: {
  model: PrintModel;
  snapshot: PrintSnapshot;
  page: number;
  pages: number;
  copy: number;
  copies: number;
}) {
  const f = printFormat(snapshot);
  return (
    <footer className="print-footer">
      <span>{model.footer}</span>
      <span>
        {t('printPage')} <bdi>{f.number(page)}</bdi> {t('printOf')} <bdi>{f.number(pages)}</bdi>
        {copies > 1
          ? ` · ${t('printCopy')} ${f.number(copy)} ${t('printOf')} ${f.number(copies)}`
          : ''}
      </span>
    </footer>
  );
}
export function DocumentPages({
  model,
  snapshot,
  layout,
  logo,
  copies,
  onReady,
  onError,
}: {
  model: PrintModel;
  snapshot: PrintSnapshot;
  layout: Exclude<PrintLayout, 'labels'>;
  logo: string;
  copies: number;
  onReady: (paper: PrintPaper | null) => void;
  onError: (error: unknown) => void;
}) {
  const measure = useRef<HTMLDivElement>(null);
  const [pagination, setPagination] = useState<{ pages: string[][]; height: number } | null>(null);
  const width = layout === 'thermal80' ? 80 : layout === 'landscape' ? 297 : 210;
  const height = layout === 'thermal80' ? 800 : layout === 'landscape' ? 210 : 297;
  const padding = layout === 'thermal80' ? 4 : 12;
  const sections = useMemo(
    () =>
      model.sections.map((s) =>
        s.rows.length ? s : { ...s, rows: [[t('noData')]], headers: [], widths: undefined },
      ),
    [model],
  );
  useLayoutEffect(() => {
    let live = true;
    setPagination(null);
    onReady(null);
    void (async () => {
      await document.fonts.ready;
      const root = measure.current;
      if (!root || !live) return;
      await Promise.all(Array.from(root.querySelectorAll('img')).map((img) => img.decode()));
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );
      if (!live) return;
      const head = root.querySelector('.print-header')!.getBoundingClientRect().height;
      const foot = root.querySelector('.print-footer')!.getBoundingClientRect().height;
      const rows: MeasuredRow[] = [];
      for (const table of root.querySelectorAll<HTMLTableElement>('.print-table')) {
        const heading = table.tHead?.getBoundingClientRect().height ?? 0;
        for (const row of table.querySelectorAll<HTMLTableRowElement>('tbody tr'))
          rows.push({
            id: row.dataset.flowId!,
            height: row.getBoundingClientRect().height,
            group: table.dataset.group!,
            heading,
            keepTogether: table.dataset.group === 'totals',
          });
      }
      const pages = paginate(
        rows,
        (height - 0.5) * PX_PER_MM - 2 * padding * PX_PER_MM - head - foot - 8,
      );
      let actualHeight = height;
      if (layout === 'thermal80' && pages.length === 1) {
        let previous = '';
        const flow = rows.reduce((total, r) => {
          const h = r.height + (previous === r.group ? 0 : r.heading);
          previous = r.group;
          return total + h;
        }, 0);
        actualHeight = Math.min(
          800,
          Math.max(60, Math.ceil((head + foot + flow + 10) / PX_PER_MM + 2 * padding)),
        );
      }
      setPagination({ pages, height: actualHeight });
      onReady({ width_mm: width, height_mm: actualHeight });
    })().catch((e) => {
      if (live)
        onError(
          e instanceof Error && ['page-capacity', 'row-too-tall'].includes(e.message)
            ? new Error(t('printOverflow'))
            : e,
        );
    });
    return () => {
      live = false;
    };
  }, [sections, layout, logo, copies, width, height, padding, onReady, onError]);
  const style = {
    '--paper-width': `${width}mm`,
    '--paper-height': `${pagination?.height ?? height}mm`,
    '--paper-padding': `${padding}mm`,
  } as CSSProperties;
  const header = <Header model={model} snapshot={snapshot} logo={logo} />;
  return (
    <div className={`print-pages print-${layout}`} style={style}>
      {!pagination && (
        <div className="print-measure print-page" ref={measure} aria-hidden="true">
          {header}
          <div className="print-flow">
            {sections.map((section) => (
              <TableSection key={section.id} section={section} />
            ))}
          </div>
          <Footer model={model} snapshot={snapshot} page={1} pages={1} copy={1} copies={copies} />
        </div>
      )}
      {pagination &&
        Array.from({ length: copies }, (_, copy) =>
          pagination.pages.map((ids, i) => (
            <article className="print-page" key={`${copy}:${i}`} data-print-page>
              {header}
              <div className="print-flow">
                {sections.map((section) => {
                  const indices = ids
                    .filter((id) => id.startsWith(`${section.id}:`))
                    .map((id) => Number(id.slice(section.id.length + 1)));
                  return indices.length ? (
                    <TableSection key={section.id} section={section} indices={indices} />
                  ) : null;
                })}
              </div>
              <Footer
                model={model}
                snapshot={snapshot}
                page={i + 1}
                pages={pagination.pages.length}
                copy={copy + 1}
                copies={copies}
              />
            </article>
          )),
        )}
    </div>
  );
}
function LabelBarcode({ value, onError }: { value: string; onError: (error: unknown) => void }) {
  const ref = useRef<SVGSVGElement>(null);
  useLayoutEffect(() => {
    try {
      JsBarcode(ref.current, value, {
        format: 'CODE128',
        height: 36,
        width: 1,
        margin: 10,
        marginTop: 0,
        marginBottom: 0,
        displayValue: false,
      });
      const modules = Number(ref.current!.getAttribute('width'));
      // Keep every narrow bar at least 0.25 mm (two dots on a 203 dpi printer).
      if (modules * 0.25 > 54) throw new Error(t('barcodeTooWide'));
      ref.current!.setAttribute('preserveAspectRatio', 'none');
      ref.current!.style.width = `${Math.min(54, modules * 0.33)}mm`;
      ref.current!.style.height = '10mm';
    } catch {
      onError(new Error(t('barcodeTooWide')));
    }
  }, [value, onError]);
  return <svg ref={ref} role="img" aria-label={t('barcodePreview')} />;
}
export function LabelPages({
  snapshot,
  quantity,
  skip,
  price,
  wholesale,
  onReady,
  onError,
}: {
  snapshot: PrintSnapshot;
  quantity: number;
  skip: number;
  price: boolean;
  wholesale: boolean;
  onReady: (paper: PrintPaper | null) => void;
  onError: (error: unknown) => void;
}) {
  const f = printFormat(snapshot);
  const labels = useMemo(
    () => [
      ...Array.from({ length: skip }, () => null),
      ...snapshot.rows.flatMap((row) => Array.from({ length: quantity }, () => row)),
    ],
    [snapshot, quantity, skip],
  );
  const pages = Array.from({ length: Math.ceil(labels.length / 24) }, (_, i) =>
    labels.slice(i * 24, i * 24 + 24),
  );
  useEffect(() => {
    let live = true;
    onReady(null);
    void document.fonts.ready.then(() => {
      if (live) onReady({ width_mm: 210, height_mm: 297 });
    });
    return () => {
      live = false;
    };
  }, [quantity, skip, price, wholesale, snapshot, onReady]);
  return (
    <div className="print-pages print-labels">
      {pages.map((page, i) => (
        <article key={i} className="print-page label-sheet" data-print-page>
          {page.map((row, k) => (
            <section className="barcode-label" key={k}>
              {row && (
                <>
                  <div className="label-store">{s(snapshot.store, 'store_name')}</div>
                  <strong className="label-name">{s(row, 'name_ar')}</strong>
                  <div className="label-detail">
                    <bdi>{s(row, 'code')}</bdi>
                    <span>{s(row, 'unit_name')}</span>
                    {price && (
                      <bdi>
                        {f.money(row[wholesale ? 'wholesale_price_minor' : 'retail_price_minor'])}
                      </bdi>
                    )}
                  </div>
                  <LabelBarcode value={s(row, 'barcode')} onError={onError} />
                  <bdi className="label-digits" dir="ltr">
                    {s(row, 'barcode')}
                  </bdi>
                </>
              )}
            </section>
          ))}
        </article>
      ))}
    </div>
  );
}
