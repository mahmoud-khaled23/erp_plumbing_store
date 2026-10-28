# Stage 3: Arabic desktop interface

The complete source is in `src/`, connected to the Tauri commands in `src-tauri/`.
Every data-changing screen calls the Rust transaction layer. There is no browser-only
success simulation, API server, CDN font, telemetry or production network service.
Native file selection, backups and restore use permission-checked Rust commands.

## Run on Fedora

Install the native prerequisites and pinned Rust toolchain described in
[DEVELOPMENT.md](DEVELOPMENT.md). With Node.js >=22.12 and npm 11.21.1:

```bash
npm ci
npm run desktop
```

`npm run dev` is a development-only Vite server. Opening it directly in a browser shows
the Arabic desktop-required screen, because there is no Tauri command transport there.
`npm run build` generates bundled frontend assets in `dist/`; it does not produce an
installer. The Tauri configuration enables the `desktop` Cargo feature automatically.
The installed Windows application uses WebView2 and local SQLite, without Node.js
or a web server. A Windows candidate has been built from Fedora; native Windows acceptance
remains pending. See [WINDOWS_RELEASE.md](WINDOWS_RELEASE.md).

## Screens and workflows

| Area | Implemented interface |
| --- | --- |
| First run / authentication | Store and administrator setup, optional 100-product seed, opening imports, login, forced password change, recovery from verified backup |
| Dashboard | Today/month sales, gross profit, cash, AR/AP, low stock, 30-day sales and cash charts, top products/categories, overdue balances, due cheques, recent invoices |
| Products | Paginated name/code/barcode search, full product fields, image, multiple units and barcodes, exact conversion factors, prices, category/unit masters, CSV/Excel preview/import/export, barcode preview |
| Inventory | Warehouse balances, warehouse-specific reorder level/bin, stock card, valuation, adjustments, stocktake count sheet and differences, transfers, opening stock |
| Sales / POS | Barcode queue, keyboard search, retail/wholesale units, discounts/VAT, split/partial cash-bank-cheque payments, credit, save/hold/resume, quotations and conversion, linked partial returns, cancellation/reversal |
| Purchasing / receiving | Approved orders, remaining quantities, partial or no-PO receipt, damaged/short fields, supplier invoice matching, purchase returns, allocated landed costs |
| Parties | Customer/supplier details, credit limits, terms, statements, opening-balance imports |
| Finance | Receipts/payments/advances, expenses, manual/opening journals, cash transfers, shifts, cheque clearance/bounce, settlement/unsettlement, journal inspection |
| Reports | Trial balance, P&L, stock valuation, product/category/customer profit, AR/AP, aging, statement, cash flow, VAT, low stock, cheques, PO progress, open items, cash shifts |
| Settings | Store/currency/digits/VAT, print preferences, master data, sequences, fiscal locks, backups, restore, opening imports |
| Users / audit | Users, roles, active status, password reset, paginated audit trail with expandable Arabic field labels |

Stage 4 adds document/report/stock-card print previews, native PDF export and barcode
label sheets. See [PRINTING.md](PRINTING.md) for templates and printer instructions.
The [Arabic user guide](../README_AR.md) and [Windows build commands](WINDOWS_RELEASE.md)
complete the operating and release documentation.

## Navigation and precision

The root document is `lang="ar" dir="rtl"`. Shared controls use logical margins/padding,
visible focus, accessible names, modal focus containment and keyboard table selection.
Cairo WOFF2 files are bundled by Vite; the font's OFL license is copied to
`public/licenses/Cairo-OFL.txt` and included in the build.

All labels live in `src/i18n/ar.json`. Dates display `dd/MM/yyyy`. Displayed digits follow
the store preference. Decimal inputs accept Western, Arabic-Indic and Persian digits;
editable money/quantity strings are normalized without floating-point arithmetic.
Minor units and quantities cross IPC as decimal strings. `BigInt` calculates invoice
previews, discounts, tax, return previews and allocation; Rust recalculates authoritative
values when posting. Only chart geometry converts to JavaScript `Number`; tooltips and
totals retain the original exact amounts.

The screen reads are bounded: catalogue and document queries use 50-row keyset pages,
reports/count sheets use 100-row pages, and product suggestions use at most 30 rows.
TanStack Table renders these pages. Feature modules, charts and JsBarcode load lazily.
No router, icon pack, client Excel engine or client database is bundled.

| Key | Action |
| --- | --- |
| F2 | Open POS, with an unsaved-change confirmation where necessary |
| F4 | Focus product/barcode search |
| Arrow up/down + Enter | Choose a product; barcode Enter resolves the matching unit |
| F8 | Save the current invoice draft |
| F9 | Confirm posting the current invoice/receipt/transfer |
| Ctrl+K | Focus the current screen's catalogue/product search |
| Escape | Close a dismissible dialog or product suggestions |

Consecutive scans are queued instead of dropped while queries run. Saving/posting waits
until the scan queue is empty. Drafts stay in component memory; F8 saves them to SQLite.
On session expiry, the same user can authenticate again without losing the current
in-memory invoice. Closing the process before saving still loses unsaved work. Saved
drafts and held invoices are available from their document lists after restarting.

Frontend permissions control navigation and buttons; Rust independently authorizes
every query/mutation. Sessions/passwords are never stored in localStorage. Mutations
reuse a UUID when retrying an uncertain transport result, and optimistic revisions
protect saved drafts/products/settings from stale writes. Posted documents are immutable.

## Implementation map

| Path | Responsibility |
| --- | --- |
| `src/app/App.tsx`, `store.ts`, `dialogs.ts` | RTL shell, feature loading, permission navigation, session state, confirmations, toasts |
| `src/api/contracts.ts` | Generated request unions from Rust; do not edit manually |
| `src/api/client.ts` | Typed invoke wrapper, runtime response validation, idempotency, file commands |
| `src/lib/numbers.ts`, `hooks.ts` | Exact arithmetic, date/digit formatting, bounded queries, task guards |
| `src/components/` | Table, forms, modal controls, pickers, payments, import preview, image loading, audit values |
| `src/features/auth/` | First run, login, password and session renewal |
| `src/features/dashboard/` | KPI cards, exact-value tooltips and charts |
| `src/features/products/`, `parties/` | Catalogue, units, barcodes, party maintenance and statements |
| `src/features/documents/` | Shared invoice/receipt/order editor, document history, returns, source selection |
| `src/features/inventory/`, `purchasing/` | Stock, counts, warehouse settings and landed costs |
| `src/features/finance/`, `reports/` | Financial operations and paginated accounting/inventory reports |
| `src/features/printing/` | Lazy print preview, measured Arabic templates, native actions and barcode sheets |
| `src/features/settings/` | Store/master settings, periods, backup/import, users and audit |
| `src/i18n/`, `src/styles.css` | Arabic dictionary and shared RTL styling |
| `scripts/generate_types.py`, `check_i18n.py` | Rust/TypeScript contract drift and Arabic-label checks |
| `tests/frontend/` | Exact arithmetic, invoice totals and IPC retry tests |
| `tests/ui/`, `src-tauri/examples/ui_driver.rs` | Browser integration with the real Rust application and disposable SQLite store |

After changing a Rust input DTO, regenerate and check:

```bash
python3 scripts/generate_types.py
npm run check
```

## Validation

```bash
npm run check
npm run format:check
npm test
npm run build
bash scripts/test-backend.sh
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
```

Browser integration requires Docker and Google Chrome on the Linux development host.
With Chrome already installed:

```bash
npm run test:ui
```

The `pretest:ui` script builds a test-only Rust driver in the pinned Docker image. Each
test starts a separate driver and temporary database. A dedicated Vite server on
`127.0.0.1:5179` keeps the tests separate from a running Tauri session on port 5173. Playwright replaces only the IPC
transport: React invokes actual Rust setup, authentication, query and transaction
services. Neither the driver nor Playwright is included in the desktop installer.
These browser tests do not open native file dialogs or exercise WebKitGTK/WebView2.
The separate stage 4 [native PDF check](PRINTING.md) exercises WebKitGTK export.

The supplied GitHub workflow installs its browser using
`npx playwright install --with-deps chrome` on Ubuntu. On Fedora use an installed Chrome
and its system dependencies. See the official [Playwright browser documentation](https://playwright.dev/docs/browsers).

Recorded locally through stage 5: 12 frontend unit tests, 10 Chrome integration workflows,
29 Rust tests and 32 Python schema/release tests passed. Integration covers first run/offline font, fractional sale
and return, unit barcode/editing and draft guard, partial receiving/invoice matching,
stocktake, every module route, queued scans/session renewal, and balanced advance/expense
journals. Dashboard/POS screenshots are generated in `output/` for visual review.
Frontend type checking, build, formatting and Rust Clippy also pass. CI configuration is
supplied, but no remote CI run or Windows execution is claimed.

The actual NSIS installer measured 3,713,278 bytes. Idle RAM below 150 MB and startup
below 2 seconds remain acceptance targets requiring a documented Windows machine;
they are not inferred from frontend bundle size.
