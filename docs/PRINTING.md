# Stage 4: Arabic printing and PDF

This stage delivers the templates, preview, native print/PDF commands and integration
tests. It adds no PDF service, browser executable or JavaScript PDF engine to the app.
Cairo, React, CSS and JsBarcode are bundled and work offline. Windows uses the existing
WebView2 runtime; Fedora development uses WebKitGTK.

## الاستخدام

1. افتح مستنداً محفوظاً واختر **طباعة المستند**. اختر A4 بالطول أو بالعرض، أو
   إيصال ٨٠ مم، وحدد عدد النسخ. المسودة والمستند الملغى يحملان تنبيهاً واضحاً.
2. راجع الصفحات ثم اختر **طباعة** لفتح حوار الطابعة، أو **حفظ PDF** لاختيار ملف
   محلي أو مجلد على USB. عند وجود ملف بالاسم نفسه اختر اسماً جديداً؛ لا يستبدل
   التطبيق النسخة السابقة. إلغاء حوار الحفظ لا ينشئ ملفاً نهائياً.
3. من **التقارير** حدد التقرير والتواريخ والطرف عند الحاجة، ثم **طباعة التقرير**.
   الطباعة تشمل كل صفوف النطاق، حتى لو كانت الشاشة تعرض صفحة واحدة. إذا تجاوز
   التقرير عشرة آلاف صف فسيطلب التطبيق تضييق النطاق، ولن يحذف صفوفاً بصمت.
4. من تفاصيل الصنف اطبع ملصق الوحدة المطلوبة، أو **طباعة ملصقات كل الوحدات**.
   اختر عدد الملصقات وإظهار السعر والتجزئة/الجملة. يمكن تجاوز أول ٠–٢٣ خانة
   لاستخدام ورقة ملصقات بدأت طباعتها سابقاً.
5. يمكن طباعة إذن الاستلام من مستنداته، وكارت الصنف من المخزون. إذن الاستلام
   يميز الكمية المقبولة عن التالف والناقص؛ الأخيرة لا تدخل المخزون.

للطباعة الورقية استخدم المقاس المعروض، ومقياس ١٠٠٪، وهوامش صفر، وأوقف رأس
المتصفح وتذييله. النسخ مكررة داخل المعاينة؛ اجعل نسخ تعريف الطابعة **نسخة واحدة**.
الملصقات: A4، ثلاثة أعمدة وثمانية صفوف، ٦٢ × ٣٢ مم، فاصل ٢ مم وهامش ١٠ مم.
شريط الباركود يظل بأرقام غربية ليطابق الماسح. اسم الصنف الطويل يقتصر على سطرين
في الملصق؛ يظهر كاملاً في شاشة الأصناف والفواتير.

استخدم ورق اختبار للتحقق من محاذاة الملصقات ومنطقة الطباعة الفعلية للطابعة.
التطبيق لا يعتبر إغلاق حوار الطابعة دليلاً على خروج ورقة. قيمة «المتبقي عند
الإصدار» تخص دفعات الفاتورة الأصلية؛ الرصيد الحالي موجود في كشف حساب الطرف.

## Templates and data

| Template | Content and pagination |
| --- | --- |
| A4 portrait / landscape | Historical store/logo/customer details, document status, units, fractional quantities, exact prices, discounts, VAT, totals, split payments, notes and signatures |
| 80 mm receipt | Compact four-column lines, store details, VAT/discounts, payment summary and footer; height fits the receipt from 60 to 800 mm, with longer receipts paginated |
| Goods-received note | Accepted lines, warehouse/source, separate damaged/short quantities, notes and signatures |
| Stock documents | Transfers, adjustments, opening stock and stocktake count/difference sheets |
| Reports | All report screens and complete stock cards; repeated headings, scope/date, generation time in the store timezone, currency, opening balances and appropriate totals |
| Barcode labels | CODE128 SVG for each selected product unit, unit barcode/price, optional price, first-sheet offset, maximum 500 labels per job |

The print snapshot runs in the same read transaction as the application's other queries.
It reuses the source's Rust permission checks; the native commands authorize again.
Each native print/PDF request also appends an audit event with user, time and selected
source, including reprints. The event records a request, including one later cancelled
in the dialog; it never claims physical delivery. Opening a preview does not write an event.
Document store, currency, customer and line descriptions come from saved snapshots.
Changing the catalogue or store name does not rewrite an old invoice. Current digit,
copy and layout preferences apply to the preview. New documents also snapshot the
receipt footer; earlier documents without that field use the current footer. No schema
migration is needed for this compatible JSON-field addition.

Money remains integer strings/BigInt. Printing never posts a document, changes stock,
creates journal lines or mutates ledger rows. Text is rendered as React text nodes,
including customer-provided names and notes. Logo files come from the authorized local
asset command. A missing logo blocks output with an Arabic message; the user can
explicitly disable the logo in the preview.

Rows are measured after local fonts/images load. Each page reserves the header, footer,
padding and repeated table headings. Rows cannot split, and an unprintably tall row
produces an error instead of being clipped. A 0.5 mm allowance avoids WebKit's rounded
page-height blank sheet. Labels reject barcodes that cannot retain a 0.25 mm minimum
narrow bar within the 54 mm available width, including quiet zones.

## Native implementation

- `src/features/printing/model.ts`: exact display model for documents and reports.
- `Pages.tsx`, `paginate.ts`, `print.css`: measured RTL pages, repeated headers/footers,
  label sheets and dedicated print-media rules that hide application controls.
- `PrintHost.tsx`, `store.ts`: lazy modal preview, layout/copies, focus restoration,
  snapshot loading and native actions.
- `src-tauri/src/services/printing.rs`: bounded, permission-checked snapshots.
- `src-tauri/src/commands/printing.rs`: main-window restriction, valid paper sizes,
  native dialogs and atomic PDF finalization.

Windows uses `ICoreWebView2_7::PrintToPdf` with explicit paper dimensions, zero margins,
backgrounds enabled and browser headers disabled. The physical-print action opens
WebView2's preview. Linux uses `WebKitPrintOperation` with a custom GTK page setup,
either its printer dialog or the print-to-file backend. These APIs reuse the engine
already used by Tauri. The native bindings match the lockfile; Tauri's minor version is
pinned because its native-webview interface can change across minor releases.
See [Tauri native webviews](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.with_webview),
[WebView2 printing](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/print)
and [WebKit print operations](https://webkitgtk.org/reference/webkit2gtk/2.36.6/WebKitPrintOperation.html).

PDF export accepts no caller-provided output path. A native save dialog selects a local
`.pdf` outside the application's data directory. The engine writes a temporary file in
the destination directory; Rust verifies its PDF signature, syncs it and promotes it
without replacing an existing file. One PDF job runs at a time. If a job takes over
120 seconds, the UI receives an Arabic error; the native job retains its temporary file
and lock until its callback completes, then cleans up. The app does not open or upload
the saved PDF automatically.

## Repeat the checks on Fedora

Run the regular tests, then generate the review PDFs with the real Rust/SQLite driver:

```bash
npm run check
npm test
npm run test:ui -- tests/ui/printing.spec.ts
bash scripts/test-backend.sh
```

The browser workflow writes five PDFs and offline HTML fixtures under `output/`.
Build the optional test-only native renderer and its isolated display environment:

```bash
docker build -f docker/Dockerfile.check -t plumbing-erp-check:stage2 .
docker build -f docker/Dockerfile.print-check -t plumbing-erp-print:stage4 .
bash scripts/check-native-print.sh
```

After dependencies have been cached, `bash scripts/check-native-print.sh --offline`
also works. Its runtime container has no network. It renders all five HTML fixtures
through the same WebKit PDF adapter, compares page counts and dimensions, rejects
blank pages, and rasterizes both sets of barcode PDFs at 203 dpi to decode both unit
codes with ZBar. The container-only WebKit sandbox override is necessary for this
isolated harness; it is never set by the application. Docker/Xvfb/Poppler/ZBar/Python
and the Rust example are not distributed in the installer.

An optional Windows GNU compile check on Fedora validates Windows-only Rust bindings:

```bash
docker build -f docker/Dockerfile.tools -t plumbing-erp-tools:stage2 .
docker build -f docker/Dockerfile.windows-check -t plumbing-erp-windows-check:stage4 .
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target-windows \
  -e CARGO_BUILD_JOBS=2 -v "$PWD:/work:Z" \
  plumbing-erp-windows-check:stage4 \
  cargo check --locked --target x86_64-pc-windows-gnu --features desktop
```

This is compilation only. The release target remains Windows MSVC/NSIS. Windows runtime,
save/printer dialogs, physical A4/thermal printers, barcode scanners and installer size,
RAM and startup measurements still need Windows acceptance in stage 5.

## Validation recorded on 28 October 2026

- 29 Rust workflow tests, 28 schema tests, 12 frontend unit tests and 10 browser/Rust
  integration workflows pass. Print tests cover historical snapshots, permissions,
  audit requests, complete reports, fractional quantities and exact large amounts.
- TypeScript checks, generated contracts, Arabic dictionary checks, formatting and the
  production frontend build pass. Windows GNU desktop Clippy passes with warnings denied.
- All five templates export through the shared WebKitGTK adapter with matching page
  counts and physical dimensions; no blank native pages remain. Both unit barcodes
  decode from Chromium and WebKit PDFs rasterized at 203 dpi.
- The PDFs were rendered for visual review of Arabic shaping, RTL order, repeated
  headers/footers, totals, receiving fields and label spacing. The sample 38-line invoice
  spans six pages; the 105-entry statement spans eleven landscape pages.

These samples contain disposable test-store data. No native Windows execution, physical
printer test, remote CI run or installer/performance result is claimed by this stage.
