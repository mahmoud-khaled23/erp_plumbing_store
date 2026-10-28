# Arabic Plumbing Store ERP

[دليل المستخدم بالعربية](README_AR.md) · [Architecture and schema](docs/ARCHITECTURE.md) ·
[Windows release guide](docs/WINDOWS_RELEASE.md)

An offline, Arabic RTL desktop ERP for one plumbing store. Development runs on Fedora;
deployment targets Windows 10/11 x64. Tauri 2 hosts a React 18/TypeScript interface and
Rust services backed by bundled SQLite. There is no production HTTP server, Node.js
runtime, CDN font, cloud account or internet requirement. WebView2 must already be
installed on the Windows PC; an offline runtime installer can be supplied separately.

## Delivery and verified status

All six source/documentation stages are delivered. **Version 0.1.0 is a release
candidate, not a production-accepted release.** The actual unsigned NSIS installer built
on Fedora on 28 October 2026 is **3,713,278 bytes (3.71 MB)**:

```text
output/releases/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
SHA256 76522bc5254de1361787f19601f537884f35a1c1a6ff7d9a62fce969c0468ba4
```

The adjacent `release-manifest.json` and `SHA256SUMS` record the actual payload and
installer hashes, PE architectures and DLL imports. Both the verifier and LLVM found
an x64 GUI application without a separate VC++ runtime dependency. `output/` is ignored
by Git; cloning the source does not download a prebuilt installer.

| Check | Recorded result |
| --- | --- |
| Rust transaction/arithmetic/workflow tests | 29 passed |
| Python schema, installer and PE checks | 32 passed |
| Frontend unit tests | 12 passed |
| Browser integration with real Rust/SQLite | 10 passed |
| TypeScript, Arabic labels, generated contracts, formatting and Rust Clippy | Passed |
| Native PDF rendering on Fedora | Five PDFs checked; both barcode samples decoded at 203 dpi |
| Installer size, PE imports and SHA-256 | Passed; below 20,000,000 bytes |
| Windows installation, native interaction and store printer checks | Not executed |
| Windows idle RAM below 150 MB; cold start below 2 seconds | Not measured |

These results describe the recorded candidate, not an assertion that remote CI ran.
The supplied workflows must be pushed/dispatched to execute on GitHub. See
[release acceptance and measurement commands](docs/WINDOWS_RELEASE.md) before deployment.

## Implemented workflows

- Catalogue: Arabic product search, categories, images, units with rational conversions,
  per-unit prices/barcodes, CSV/Excel import/export and barcode sheets.
- Inventory: immutable movement ledger, weighted-average valuation, warehouses,
  transfers, adjustments, stocktake, low-stock rules and stock cards.
- Sales: keyboard POS, retail/wholesale pricing, discounts/VAT, split/partial payment,
  customer credit, drafts/hold/resume, quotations, linked returns and audited reversal.
- Purchasing: approved orders, partial/no-PO receiving, damaged/short quantities,
  supplier invoice matching, returns and landed costs without duplicate receipt stock.
- Accounts: customers/suppliers, opening balances, receipts/payments/advances, settlements,
  statements, historical aging, cheques, expenses, journals, cash transfers and shifts.
- Reporting/system: dashboard, financial/stock reports, fiscal locks, four seeded roles,
  Argon2id login, audit log, first-run setup, optional 100-product demo, backup/recovery.
- Printing: Arabic A4/80 mm documents, goods-received notes, complete reports, stock
  cards and barcode labels, with native print dialogs and PDF export.

The store is local to one OS user. Staff sign in through separate ERP users; this is
not a shared network database or multi-workstation system. Demo mode creates product
records only, without opening stock, transactions or default credentials.

## Start development on Fedora

The tested tool versions are Rust **1.95.0**, Node.js **22.22.0** and npm **11.21.1**.
Node.js must be at least 22.12. Python **3.11+** is used for build checks; schema tests
need SQLite 3.37+ with FTS5. The shipped Rust application bundles SQLite 3.51.3 through
rusqlite 0.39 and checks its version at startup. Docker is optional for development,
but required by the supplied browser-test driver and Fedora Windows-build scripts.

Install the Fedora desktop dependencies, Rust/rustup, Node and Python as described in
[DEVELOPMENT.md](docs/DEVELOPMENT.md). From the repository root:

```bash
npx --yes --package npm@11.21.1 npm ci
npm run desktop
```

Using npm 11 avoids the npm 10 peer-resolver failure observed on the Fedora host. Tauri
starts Vite on `127.0.0.1:5173` and opens the desktop application. The first-run screen
creates your administrator; there is no built-in password. Use demo data only in a
disposable store. Development also writes real local data through Rust.

| Command | Result |
| --- | --- |
| `npm run desktop` | Full Tauri desktop with local SQLite |
| `npm run dev` | Vite only; a normal browser has no production database transport |
| `npm run build` | Production frontend in `dist/`; not an installer |
| `npm run build:windows` | Native Windows MSVC/NSIS build |
| `bash scripts/build-windows-fedora.sh --accept-sdk-license` | Fedora Docker/MSVC/NSIS build after SDK license acceptance |

## Repository map

| Path | Responsibility |
| --- | --- |
| `src/app/` | RTL shell, permission navigation, session state, confirmations |
| `src/api/` | Typed invoke wrapper, generated request unions, response validation |
| `src/features/` | Auth, dashboard, products, inventory, documents, purchasing, parties, finance, reports, settings, printing |
| `src/components/`, `src/lib/` | Shared Arabic controls, imports, payments, exact number/date helpers |
| `src/i18n/ar.json` | Frontend and custom installer labels; Cairo is bundled from Fontsource |
| `src-tauri/src/commands/` | Permission-checked Tauri IPC, native files/printing, opt-in startup diagnostics |
| `src-tauri/src/services/` | Transactional domain operations, reporting, authentication, imports, backups |
| `src-tauri/src/models/`, `src-tauri/src/state.rs`, `src-tauri/src/db/` | Strict DTOs/arithmetic, serialized session/transaction boundary, SQLite/migrations |
| `src-tauri/migrations/` | Eight versioned SQL migrations; authoritative production DDL |
| `src-tauri/queries/`, `src-tauri/seeds/` | Historical aging SQL and optional Arabic demo catalogue |
| `src-tauri/windows/`, `.cargo/config.toml` | Arabic NSIS hooks/strings and target-specific static CRT linkage |
| `tests/`, `src-tauri/tests/` | Frontend, integration, schema, release and backend tests |
| `scripts/`, `docker/`, `.github/workflows/` | Generators, checks, containers, native and cross-platform build workflows |
| `docs/schema.sql` | Generated full schema reference; not a second migration source |

The [architecture document](docs/ARCHITECTURE.md) lists the actual source files and
explains accounting/stock invariants. The [backend API](docs/BACKEND_API.md) documents
commands, response encoding, import columns and workflows. Screen implementation and
keyboard handling are in [FRONTEND.md](docs/FRONTEND.md); printing is in
[PRINTING.md](docs/PRINTING.md).

## Boundaries to preserve when changing code

React uses React Hook Form/Zod, Zustand, TanStack Table and lazily loaded Recharts and
JsBarcode. Rust owns authorization, validation, final arithmetic and postings. Every
stock/account mutation runs in one SQLite transaction, including its audit and request
receipt. A failed post leaves no partial ledger entry. Queries share a read transaction
on the single mutex-owned connection; database work runs off the webview thread.

Money is integer minor units; quantity is integer millionths. Checked Rust `i128`
intermediates protect multiplication/allocation, with persisted `i64` bounds. IDs,
amounts, quantities and revisions cross IPC as decimal strings. Frontend previews use
`BigInt`; only chart geometry uses floating point. See the arithmetic contract before
changing prices, tax, discounts, conversion, weighted-average cost or returns.

Persisted ledgers and posted documents are immutable. Corrections use returns,
reversal or current-date adjustment documents. Supplier invoices match receipt lines;
only goods receipts add their quantity. Never update stock or party balances directly.
Financial dates must belong to open periods; stock backdating is also constrained.

Mutations use a UUID idempotency key. Retry an uncertain transport result with the same
key and payload, not a new document. Draft/product/settings revisions reject stale
writes. Permissions are checked in Rust even when an action is hidden in the interface.
Passwords and sessions are not persisted in browser storage or audit payloads.

### Schema and generated files

Never edit a migration already applied to a distributed database. Add the next numbered
SQL file and register it in `src-tauri/src/db/mod.rs`. The runner stores checksums and
refuses changed history. `.gitattributes` preserves LF across Fedora/Windows checkouts.
After changing an unreleased migration or adding one:

```bash
python3 scripts/export_schema.py
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
```

After changing Rust request DTOs or custom installer labels:

```bash
python3 scripts/generate_types.py
python3 scripts/generate_installer.py
npm run check
```

Do not manually edit `src/api/contracts.ts`, `src-tauri/windows/Arabic.nsh` or
`docs/schema.sql`. Static frontend labels belong in `src/i18n/ar.json`; backend
validation errors are Arabic in the Rust services/SQL guards. Names and role/account
reference data belong in migrations. Python tools explicitly use UTF-8. Keep lockfiles
and package/Cargo/Tauri versions consistent for release builds.

## Verify changes

With the desktop prerequisites installed, run the checks appropriate to the change:

```bash
npm run check
npm run format:check
npm test
npm run build
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
bash scripts/test-backend.sh
```

For Rust formatting/linting, install the pinned components if absent:

```bash
rustup component add rustfmt clippy --toolchain 1.95.0
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
```

Docker alternatives for Rust/GTK checks are in [DEVELOPMENT.md](docs/DEVELOPMENT.md).
With Docker and Chrome installed on Fedora:

```bash
npm run test:ui
```

Playwright uses its own Vite server on port 5179 and calls the actual Rust core through
a test-only JSON-lines driver in a disposable database. It leaves the desktop's port
5173 independent. This proves browser/core workflows; it does not prove WebView2 or
native dialog behavior. Native PDF verification is documented in [PRINTING.md](docs/PRINTING.md).

The workflows are `backend.yml` (Linux/Windows core and desktop compilation),
`frontend.yml` (UI/core integration and native PDF checks on Linux), and
`windows-release.yml` (native MSVC build and disposable-profile installer smoke test).
Do not run install/uninstall tests in a Windows profile containing a real store.

## Build the Windows installer

The Microsoft SDK/CRT license was accepted for this workstation's local build. Other
operators must review the terms linked in [WINDOWS_RELEASE.md](docs/WINDOWS_RELEASE.md)
before using cargo-xwin. The explicit flag is required by the script:

```bash
bash scripts/build-windows-fedora.sh --accept-sdk-license
```

The container uses Cargo's locked graph, the MSVC target and NSIS. The verifier checks
size, embedded font, version consistency and PE imports, then copies the real installer
and manifest into `output/releases/<version>/fedora-cross/`. Initial downloads need
internet; caches stay under `output/` and no build tools enter the installer.

For a native Windows build, install the Visual Studio C++ workload/SDK, Rust, Node,
Python and WebView2, then follow the complete PowerShell sequence in the release guide.
The final build commands are:

```powershell
npm run build:windows
npm run verify:release -- --origin windows-native
```

Signing requires a distributor-owned certificate and configuration; none is included.
The current installer is per user, Arabic, refuses downgrade/in-use updates and does
not download WebView2. Default uninstall retains store data; explicit data deletion
removes it. Windows smoke/performance scripts and their evidence formats are described
in the release guide. Cross-compilation alone does not satisfy Windows acceptance.

## Data, backups and recovery

Windows store data is under `%LOCALAPPDATA%\com.plumbingstore.erp\store`, with SQLite
in `store.db` and images in `assets/`. Fedora uses Tauri's application-local-data path
under the user's XDG data directory. No data is written beside the installed executable.
SQLite uses WAL, foreign keys, `synchronous=FULL` and startup integrity/migration checks.
Use local storage; do not place the live store on a network share or synchronize a live
database between PCs.

Backups use SQLite's online backup API and include images plus a SHA-256 manifest in
`.erpbackup` archives. Daily rotation keeps the latest 30 successful automatic backups
while manual/pre-restore backups are retained. The task runs while the app is open,
including a missed day's backup on the next start. Verify its recorded outcome; a missing
USB device is not a successful backup. Copying only a running `store.db` omits its WAL.

Restore validates and stages the archive, backs up the existing store, closes SQLite,
then replaces its directory with crash-recovery bookkeeping. It clears the session and
requires login against the restored users. A damaged store opens recovery mode instead
of silently creating a new database. Recovery authenticates a privileged user from the
selected backup. Use the application's backup/restore workflow when moving to another
PC; never remove recovery evidence before verifying the recovered store.

This README is the developer entry point. Staff operating the installed application
should use [README_AR.md](README_AR.md).
