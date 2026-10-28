import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { api, assetUrl, s, type PrintSnapshot } from '../../api/client';
import type { PrintPaper, PrintSource } from '../../api/contracts';
import { errorMessage } from '../../app/store';
import { t } from '../../i18n';
import { Button, Check, Field, Input, Loading, Select } from '../../components/ui';
import { buildPrintModel, printFormat, type PrintLayout } from './model';
import { DocumentPages, LabelPages } from './Pages';
import { closePrint, usePrint } from './store';
import './print.css';

function Preview({ source }: { source: PrintSource }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [snapshot, setSnapshot] = useState<PrintSnapshot>();
  const [loadError, setLoadError] = useState(''),
    [renderError, setRenderError] = useState(''),
    [actionError, setActionError] = useState('');
  const [layout, setLayout] = useState<PrintLayout>(source.type === 'labels' ? 'labels' : 'a4');
  const [paper, setPaper] = useState<PrintPaper | null>(null),
    [busy, setBusy] = useState(false),
    [saved, setSaved] = useState('');
  const [copies, setCopies] = useState(1),
    [quantity, setQuantity] = useState(1),
    [skip, setSkip] = useState(0);
  const [price, setPrice] = useState(true),
    [wholesale, setWholesale] = useState(false),
    [showLogo, setShowLogo] = useState(true),
    [logo, setLogo] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const locked = useRef(false);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.showModal();
    return () => previous?.focus();
  }, []);
  useEffect(() => {
    let live = true;
    setLoadError('');
    setSnapshot(undefined);
    setPaper(null);
    setRenderError('');
    setSaved('');
    void api
      .query({ type: 'print_snapshot', source })
      .then((data) => {
        if (!live) return;
        setSnapshot(data);
        if (source.type !== 'labels')
          setCopies(Math.min(10, Math.max(1, Number(s(data.preferences, 'print_copies', '1')))));
        const thermal =
          source.type === 'document' &&
          ['sale', 'sale_return', 'purchase', 'purchase_return', 'quotation'].includes(
            s(data.document?.header[0], 'kind'),
          );
        if (thermal && data.preferences.default_print_layout === 'thermal80')
          setLayout('thermal80');
        if (
          source.type === 'stock_card' ||
          (source.type === 'report' &&
            ['aging', 'party_statement', 'cash_shifts'].includes(source.report))
        )
          setLayout('landscape');
      })
      .catch((e) => {
        if (live) setLoadError(errorMessage(e));
      });
    return () => {
      live = false;
    };
  }, [source, attempt]);
  useEffect(() => {
    let live = true;
    setLogo(null);
    setRenderError('');
    setPaper(null);
    const path = s(snapshot?.store, 'logo_path');
    if (!showLogo || !path) {
      setLogo('');
      return;
    }
    void assetUrl(path)
      .then(async (url) => {
        if (!url) throw new Error(t('printLogoError'));
        const img = new Image();
        img.src = url;
        await img.decode();
        if (live) setLogo(url);
      })
      .catch(() => {
        if (live) setRenderError(t('printLogoError'));
      });
    return () => {
      live = false;
    };
  }, [snapshot, showLogo]);
  const model = useMemo(
    () => (snapshot ? buildPrintModel(snapshot, source, layout) : null),
    [snapshot, source, layout],
  );
  const failed = useCallback((error: unknown) => setRenderError(errorMessage(error)), []);
  async function execute(pdf: boolean) {
    if (!paper || locked.current || renderError) return;
    locked.current = true;
    setBusy(true);
    setActionError('');
    setSaved('');
    const title = document.title;
    try {
      document.title = model?.number || model?.title || t('barcodeLabels');
      await document.fonts.ready;
      if (pdf) {
        const result = await api.exportPdf(source, paper);
        if (!result.cancelled) setSaved(result.path ?? '');
      } else await api.print(source, paper);
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      document.title = title;
      locked.current = false;
      setBusy(false);
    }
  }
  const reset = () => {
    setPaper(null);
    setRenderError('');
    setActionError('');
    setSaved('');
  };
  const maxQuantity = Math.max(1, Math.floor(500 / (snapshot?.rows.length || 1)));
  return (
    <div id="erp-print-root">
      <dialog
        ref={dialog}
        className="print-dialog"
        aria-label={t('printPreview')}
        onCancel={(e) => {
          e.preventDefault();
          if (!busy) closePrint();
        }}
      >
        <div className="print-toolbar">
          <div className="print-toolbar-controls">
            <h2>{t('printPreview')}</h2>
            {source.type !== 'labels' && (
              <>
                <Field label="printLayout">
                  <Select
                    value={layout}
                    disabled={busy}
                    onChange={(e) => {
                      reset();
                      setLayout(e.target.value as PrintLayout);
                    }}
                  >
                    <option value="a4">{t('a4Portrait')}</option>
                    <option value="landscape">{t('a4Landscape')}</option>
                    {source.type === 'document' && (
                      <option value="thermal80">{t('thermal80')}</option>
                    )}
                  </Select>
                </Field>
                <Field label="printCopies">
                  <Select
                    value={copies}
                    disabled={busy}
                    onChange={(e) => {
                      reset();
                      setCopies(Number(e.target.value));
                    }}
                  >
                    {Array.from({ length: 10 }, (_, i) => (
                      <option key={i} value={i + 1}>
                        {snapshot ? printFormat(snapshot).number(i + 1) : i + 1}
                      </option>
                    ))}
                  </Select>
                </Field>
                {snapshot?.store.logo_path && (
                  <Check
                    label="printLogo"
                    checked={showLogo}
                    disabled={busy}
                    onChange={(e) => setShowLogo(e.target.checked)}
                  />
                )}
              </>
            )}
            {source.type === 'labels' && (
              <>
                <Field label="labelsPerUnit">
                  <Input
                    type="number"
                    min={1}
                    max={maxQuantity}
                    value={quantity}
                    disabled={busy}
                    onChange={(e) => {
                      reset();
                      setQuantity(
                        Math.min(maxQuantity, Math.max(1, Math.floor(Number(e.target.value) || 1))),
                      );
                    }}
                  />
                </Field>
                <Field label="labelSkip">
                  <Input
                    type="number"
                    min={0}
                    max={23}
                    value={skip}
                    disabled={busy}
                    onChange={(e) => {
                      reset();
                      setSkip(Math.min(23, Math.max(0, Math.floor(Number(e.target.value) || 0))));
                    }}
                  />
                </Field>
                <Check
                  label="labelShowPrice"
                  checked={price}
                  disabled={busy}
                  onChange={(e) => {
                    reset();
                    setPrice(e.target.checked);
                  }}
                />
                <Check
                  label="wholesale"
                  checked={wholesale}
                  disabled={busy}
                  onChange={(e) => {
                    reset();
                    setWholesale(e.target.checked);
                  }}
                />
              </>
            )}
            <Button
              tone="primary"
              disabled={!paper || !!renderError}
              busy={busy}
              onClick={() => void execute(false)}
            >
              {t('printNow')}
            </Button>
            <Button
              disabled={!paper || !!renderError}
              busy={busy}
              onClick={() => void execute(true)}
            >
              {t('savePdf')}
            </Button>
            <Button disabled={busy} onClick={closePrint}>
              {t('closePreview')}
            </Button>
          </div>
          <p className="print-hint">
            {t(
              source.type === 'labels'
                ? 'labelPrintHint'
                : layout === 'thermal80'
                  ? 'thermalPrintHint'
                  : 'paperPrintHint',
            )}
          </p>
          {(loadError || renderError || actionError) && (
            <div className="print-inline-error" role="alert">
              {loadError || renderError || actionError}
              {loadError && <Button onClick={() => setAttempt((v) => v + 1)}>{t('retry')}</Button>}
            </div>
          )}
          {saved && (
            <p className="print-hint" role="status">
              {t('pdfSaved')} <bdi>{saved}</bdi>
            </p>
          )}
          {!paper && !loadError && !renderError && (
            <p className="print-hint" role="status">
              {t('preparingPrint')}
            </p>
          )}
        </div>
        {paper && (
          <style>{`@page { size: ${paper.width_mm}mm ${paper.height_mm}mm; margin: 0; }`}</style>
        )}
        {snapshot &&
          model &&
          (source.type === 'labels' ? (
            <LabelPages
              snapshot={snapshot}
              quantity={quantity}
              skip={skip}
              price={price}
              wholesale={wholesale}
              onReady={setPaper}
              onError={failed}
            />
          ) : logo !== null ? (
            <DocumentPages
              model={model}
              snapshot={snapshot}
              layout={layout as Exclude<PrintLayout, 'labels'>}
              logo={logo}
              copies={copies}
              onReady={setPaper}
              onError={failed}
            />
          ) : null)}
        {!snapshot && !loadError && <Loading />}
      </dialog>
    </div>
  );
}
export default function PrintHost() {
  const job = usePrint((s) => s.job);
  return job ? createPortal(<Preview key={job.id} source={job.source} />, document.body) : null;
}
