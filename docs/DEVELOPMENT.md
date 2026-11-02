# Developing on Fedora, deploying on Windows

The source includes the schema, Rust backend, Tauri adapter, Arabic React application,
native printing/PDF and Windows NSIS build/acceptance tools. Start with the
[developer README](../README.md) or [Arabic user guide](../README_AR.md). The portable
backend can also be compiled and tested without a webview or JavaScript dependencies.

## Portable backend

Rust is pinned to 1.95.0 in `rust-toolchain.toml`. `src-tauri/Cargo.lock` pins dependencies,
including rusqlite 0.39.0 with bundled SQLite 3.51.3. The application asserts the SQLite
version at startup, enables WAL and foreign keys, validates migration checksums and runs
database integrity checks. There is no SQLite installation requirement on the user's PC.

On a machine with Rust installed:

```bash
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo check --locked --manifest-path src-tauri/Cargo.toml --no-default-features
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
```

On Fedora without host Rust, use the supplied Docker runner:

```bash
bash scripts/test-backend.sh
```

The script uses host Rust if available; otherwise it runs a pinned Rust image as your
UID/GID. Cargo downloads and build output stay under `output/`, which is gitignored.
The `:Z` mount label supports Fedora SELinux. This is a development environment only;
Docker, Rust, Python and their caches are not included in the Windows installer.
The first dependency fetch needs internet. The installed ERP will not.

Format and lint through Docker:

```bash
docker build -f docker/Dockerfile.tools -t plumbing-erp-tools:stage2 .
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target \
  -v "$PWD:/work:Z" -w /work/src-tauri plumbing-erp-tools:stage2 cargo fmt --check
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target \
  -v "$PWD:/work:Z" -w /work/src-tauri plumbing-erp-tools:stage2 \
  cargo clippy --locked --no-default-features --all-targets -- -D warnings
```

Avoid running two Cargo builds against the same target directory simultaneously;
Cargo serializes them using its artifact lock. For independent builds, give each one
a separate `CARGO_TARGET_DIR`.

## Fedora desktop prerequisites

The portable core does not link GTK or WebKit. Enabling the `desktop` feature compiles
Tauri's Linux adapter and therefore requires the native development libraries:

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel \
  librsvg2-devel libxdo-devel
```

Install Rust with rustup using the official Rust instructions and Node.js >=22.12.
The frontend lockfile was generated with npm 11.21.1. A Fedora npm 10.9.4 peer-resolver
crash (`Cannot read properties of null (reading 'edgesOut')`) was encountered during
dependency installation. To use the tested npm without replacing Fedora packages:

```bash
npx --yes --package npm@11.21.1 npm ci
npm run desktop
```

Alternatively, with npm 11 installed, run `npm ci` then `npm run desktop`.
Tauri starts Vite on `127.0.0.1:5173`, compiles the desktop feature and opens the native
window. On first run, enter your own administrator credentials. The demo checkbox
creates 100 Arabic plumbing products; opening stock and balances can then be imported.
Do not enable demo data for the real store.

Validate the desktop adapter independently:

```bash
cargo check --locked --manifest-path src-tauri/Cargo.toml --features desktop
```

These packages follow [Tauri's Fedora prerequisites](https://v2.tauri.app/start/prerequisites/).
No host system packages were installed as part of this stage. The initial development
machine was Fedora 42; use a Fedora release supported by your system administrator.

A container alternative is supplied for compilation checks:

```bash
docker build -f docker/Dockerfile.check -t plumbing-erp-check:stage2 .
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target \
  -e CARGO_BUILD_JOBS=2 -v "$PWD:/work:Z" -w /work/src-tauri \
  plumbing-erp-check:stage2 cargo check --locked --features desktop
```

This compiles the adapter without a display server. Stage 3 browser integration tests
exercise React with the real Rust core in a temporary database. Neither check replaces
native WebKitGTK/WebView2 acceptance tests or printer tests. See [FRONTEND.md](FRONTEND.md).

## Windows build and validation

Use the Microsoft MSVC toolchain on Windows. Install Visual Studio Build Tools with
the **Desktop development with C++** workload, Rust/rustup, Node.js LTS and WebView2.
Backend validation in PowerShell:

```powershell
rustup toolchain install 1.95.0 --profile minimal
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo check --locked --manifest-path src-tauri/Cargo.toml --features desktop
```

`.github/workflows/backend.yml` runs these checks on both Linux and Windows when the
repository is pushed or the workflow is dispatched. This workflow has been written
locally; no remote workflow run or Windows execution is claimed by this delivery.

The packaging target is `x86_64-pc-windows-msvc` and NSIS `.exe`, configured in
`src-tauri/tauri.conf.json`. [WINDOWS_RELEASE.md](WINDOWS_RELEASE.md) gives exact commands
for the Fedora cargo-xwin container and native Windows builds, artifact verification,
installation checks and measurements. The MSVC SDK/CRT license must be accepted before
using the cross-build. Installing a Rust Windows target alone does not provide the SDK
or verify Windows behavior. Cross-compilation is not the Windows acceptance test.
[Tauri Windows distribution documentation](https://v2.tauri.app/distribute/windows-installer/).

The compact installer assumes WebView2 is installed already. An offline runtime package
can travel separately on USB. This configuration never downloads it during installation.
The verifier enforces an installer size below 20,000,000 bytes. Idle RAM and cold-start
targets still require the documented Windows reference-machine measurements.

## Validation recorded through 28 October 2026

On the Fedora development host, using the pinned Linux Rust containers:

- 29 Rust tests passed, covering arithmetic, postings/rollback, linked returns,
  receipt/invoice matching, late costs, negative-stock reconciliation, permissions,
  historical aging, file imports, backup recovery and pre-migration preservation.
- 32 Python tests passed: 28 schema contracts and four installer/PE/integrity checks; the
  generated schema matches all eight migrations. Explicit UTF-8 and LF keep the
  Arabic source and migration checksums consistent between Fedora and Windows.
- Rust formatting and Clippy with warnings denied passed.
- `cargo check --locked --features desktop` compiled the Linux Tauri adapter.
- React/TypeScript checks, production Vite build and frontend formatting passed.
- 12 frontend unit tests and 10 Chrome/Rust/SQLite integration workflows passed.
- Windows GNU desktop Clippy checked the WebView2 PDF bindings with warnings denied.
- NSIS hooks compiled with warnings treated as errors. PowerShell scripts passed parsing
  and process-tree memory accounting checks inside PowerShell 7.5 on Linux.
- npm reported zero known vulnerabilities in runtime dependencies at this check.
- The Fedora MSVC build produced a 3,713,278-byte NSIS installer. PE import checks and
  independent LLVM inspection confirmed the x64 GUI payload with static VC++ linkage;
  the copied installer's SHA-256 check passed. See the release guide for the exact hash.
- Five native WebKitGTK PDFs matched browser page counts/sizes; both barcode codes decoded at 203 dpi.
- Arabic RTL dashboard and POS screenshots were inspected; Cairo loads locally.

The native rendering check covers the PDF adapter and exported print markup, not the
complete native desktop or save/printer dialogs. These checks do not constitute Windows
execution, physical printer tests or measurements of idle memory or startup time. Actual
installer size and hashes are recorded by the separate release artifact verifier.

## Application files and portability

The Tauri application-local-data directory contains `store/store.db` and `store/assets`.
On Windows it is under the current user's local application-data folder; on Linux it
uses Tauri's platform path resolver. Nothing is written beside the installed executable.
All paths use `Path`/`PathBuf`. No shell commands, drive-letter assumptions or Linux paths
are embedded in application services.

SQLite access is serialized through one owned connection. Tauri commands use blocking
workers so database operations and Argon2 do not block the webview's main thread.
The desktop has a single-instance guard. Sessions and import permissions live only in
memory. No network or unrestricted filesystem API is exposed to the frontend.

Backup files include the database, image assets, schema version and SHA-256 manifest.
They are verified before restore. Restore closes SQLite before directory replacement,
which is necessary on Windows. A durable intent file lets the next startup roll back
an interrupted directory replacement before it creates any new database. The prior
store is retained in a `previous-UUID` directory and a pre-restore archive.

If startup checks fail, `open_with_recovery` exposes recovery status without creating
replacement tables in the damaged database. Recovery authenticates against the selected
backup using a user with restore permission, retains the damaged directory and requires
a fresh login. The recovery screen is included in stage 3.

Automatic backups run once per store-local day while the desktop process is open,
including startup after a missed day. Only verified automatic backup records participate
in retention of the latest 30; manual backups are retained. The backup destination can
be selected through a native folder dialog, including USB storage.

## Stage 2 implementation map

| Files | Responsibility |
| --- | --- |
| `src-tauri/src/models/` | Strict request enums, integer wire type, arithmetic and allocation |
| `src-tauri/src/db/mod.rs` | Connection setup, versioned migrations, checksums, integrity, query serialization |
| `src-tauri/src/state.rs` | Session ownership, permissions, transactions, request deduplication and recovery state |
| `src-tauri/src/services/auth.rs` | Argon2id, throttled login, first run, users and password changes |
| `src-tauri/src/services/catalog.rs` | Products, multiple units, parties, search normalization and demo seed |
| `src-tauri/src/services/documents.rs` | Drafts, numbering, hold/resume, approval, quotation conversion and posting |
| `src-tauri/src/services/costing.rs` | Moving-average costing and negative-stock reconciliation |
| `src-tauri/src/services/inventory.rs` | Stock movements, transfers, adjustments, counts, landed costs and cost replay |
| `src-tauri/src/services/returns.rs` | Linked partial returns, cumulative limits, reversal and unmatching |
| `src-tauri/src/services/finance.rs` | Journals, split payments, vouchers, settlements, cheques, expenses and cash shifts |
| `src-tauri/src/services/reporting.rs` | Paginated queries, dashboard and all accounting/stock reports |
| `src-tauri/src/services/administration.rs` | Master data, settings, fiscal locks and user access |
| `src-tauri/src/services/files.rs` | CSV/Excel, opening imports, images, backups, restore and crash recovery |
| `src-tauri/src/commands/mod.rs` | Tauri IPC commands and native file dialogs |
| `src-tauri/tests/backend.rs` | Arithmetic, transaction and workflow integration tests |

Migration 0008 adds cost attribution from late purchase/landed charges back to the
original stock issue. Subsequent sales returns use that corrected original cost and
preserve the final minor-unit remainder. Existing migrations 0001–0007 are unchanged.

All current code is implemented in these files; there are no success-returning mock
services. The module screens and keyboard navigation are described in [FRONTEND.md](FRONTEND.md).
Stage 4 adds print templates and the native PDF adapter; see [PRINTING.md](PRINTING.md)
for the implementation and Fedora native-rendering checks. A Windows installer candidate
has been built locally; performance profiling and Windows/printer acceptance remain
required before a production release.


## Inventory improvement Stage 1

The accounting/inventory improvement work is a new staged change after the original
release candidate. Only Stage 1 is implemented. Migration 0009 adds a derived stock
balance cache and `inventory.verify`; migrations 0001–0008 remain byte-for-byte unchanged.
Startup, backup/restore and authorized maintenance independently verify quantity, value,
and last movement against the ledger. Maintenance failure ends the session and enables
recovery. No posting/accounting rules or historical report definitions are changed.

Validation on Fedora, 28 October 2026:

| Command (repository root unless stated) | Result |
| --- | --- |
| `npm run check` | Passed TypeScript, generated request types and Arabic label checks |
| `npm run format:check` | Passed |
| `npm test` | 12 passed |
| `npm run build` | Passed; Vite emitted only existing dependency annotation warnings |
| `python3 -m unittest discover -s tests -v` | 37 passed |
| `python3 scripts/export_schema.py --check` | Passed; nine migrations |
| `python3 scripts/generate_installer.py --check` | Passed |
| `bash scripts/test-backend.sh -- --nocapture` (tools container below) | 36 passed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` (tools container) | Passed |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings` (tools container) | Passed |
| Desktop Clippy (exact invocation below) | Passed with warnings denied |
| `npm run test:ui` | 10 passed against the real Rust/SQLite driver, including the new maintenance dialog |

The following generators were run, rather than hand-editing their output:

```bash
python3 scripts/export_schema.py
python3 scripts/generate_types.py
python3 scripts/generate_installer.py
```

The host's pinned minimal Rust toolchain lacks rustfmt/Clippy. These exact container
commands used existing dependency caches (UID/GID 1000 on this workstation):

```bash
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh -- --nocapture
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
```

For the desktop Clippy run, a local container combined the already installed GTK/WebKit
libraries and the same pinned Clippy binaries, without downloads or host installation:

```bash
docker build --network=none -t plumbing-erp-desktop-tools:inventory -f - . <<'EOF'
FROM plumbing-erp-check:stage2
COPY --from=plumbing-erp-tools:stage2 /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/clippy-driver /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/clippy-driver
COPY --from=plumbing-erp-tools:stage2 /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy
EOF
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -e CARGO_BUILD_JOBS=2 -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-desktop-tools:inventory /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
```

The new tests compare cached pools with independent SQL ledger sums after receipts,
sales, transfers, sales/purchase returns, reversals, stocktakes, adjustments,
negative-stock corrections and late purchase/landed-cost revaluations. They also cover
version-8 backfill, zero pools, date guards, integer overflow, bulk-insert/savepoint
rollback, audit/idempotency rollback after the second issue fails, missing/extra/corrupt
cache rows, maintenance permissions, startup recovery and refusal of inconsistent backups.
An early deliberate-corruption fixture was corrected to use valid foreign keys; the
final full backend suite above passes with FK protection enabled.

The non-ignored benchmark creates **200,000 guarded movements** in a file-backed WAL
database without disabling triggers or indexes, checks cache/ledger/finance equality,
and measures actual `inventory::receive` / `inventory::issue` calls. It compares warmed
median per-operation timings from nine batches of 50 pairs at 1,000 and 200,000 prior
movements. Each measurement transaction is rolled back. The regression threshold is
5x baseline + 100 microseconds, allowing CI scheduling noise while catching a linear
history scan. Historical valuation and late-cost replay are intentionally not constant
work; startup verification deliberately scans history too.

The isolated benchmark command was:

```bash
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh issue_receive_latency_does_not_scale_with_200000_movements -- --nocapture --test-threads=1
```

| Prior movements | Receipt median (ns/op) | Issue median (ns/op) |
| --- | --- | --- |
| 1,000 | 58,933 | 80,911 |
| 200,000 | 66,953 | 88,442 |

These are debug Linux container service-call measurements inside a transaction, not
end-to-end invoice latency, commit/fsync latency, Windows startup or idle-memory results.
Windows installation, native printing and all performance targets still require the
real-device checks in [WINDOWS_RELEASE.md](WINDOWS_RELEASE.md). No new runtime dependency
was added; the maintenance UI stays in the existing lazy inventory page.

The Stage 1 source was also cross-compiled into a separate unsigned NSIS installer.
The successful build and artifact-verification commands were:

```bash
docker run --rm --user 1000:1000 -e CARGO_NET_OFFLINE=true -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z plumbing-erp-windows:stage5 npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross --output-dir output/releases/inventory-stage1
```

An initial build with Docker networking disabled compiled the application but could not
retrieve the missing official NSIS utility DLL. The retry above downloaded and
hash-validated that build-time component; Cargo dependencies remained offline. This does
not introduce a network requirement in the installed application. Existing Clang compiler
detection and non-Windows NSIS charset warnings did not prevent the successful build.

```text
output/releases/inventory-stage1/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
Installer bytes: 3717279 (below 20000000)
Installer SHA256: 10dda845302c81c832170051da645c5294c0c549aba32597c215b8e185d85ae9
Application bytes: 12940288
Application SHA256: 91dde2b8eec19fdf26110a0795eef854cee4269b81d5cb796526f91d783bfa1a
```

Adjacent `release-manifest.json` and `SHA256SUMS` record the artifact evidence. PE imports
and the x64 payload passed the verifier; the x86 NSIS bootstrapper packages that x64
application. The original candidate under `output/releases/0.1.0/` was retained.
The Stage 1 file remains a test candidate; no native Windows execution is claimed.

## Reporting improvement Stage 2

Verified locally on Fedora on 28–29 October 2026. The approved scope keeps net-sales
postings and defers warehouse-specific inventory/account reconciliation; the implemented
comparison is whole-store. Nine reports share read-only backend queries, full-result
print/export and lazy source-document/journal drill-down. Migration 0010 adds only
read/export permissions. No production runtime dependency or posting rule was changed.
Definitions and the accepted decisions are in [REPORTS_STAGE2.md](REPORTS_STAGE2.md).

The final core suite passes **48 tests**, including 12 new reporting tests and the
non-ignored 200,000-movement Stage 1 regression. Python passes **37 tests**; Vitest
passes **13 tests**. Browser/real-Rust integration passes **11 tests**, including all nine
new report screens, journal drill-down and Arabic print/PDF layouts. TypeScript, generated
contracts, Arabic labels, schema/installer generation checks, formatting and Clippy pass.

Exact host verification commands (all exited 0):

```bash
npm run check
npm run format:check
npm test
npm run build
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
npm run test:ui
git diff --check
```

Generators used for their outputs, without hand-editing generated files:

```bash
python3 scripts/export_schema.py
python3 scripts/generate_types.py
python3 scripts/generate_installer.py
```

The host's minimal toolchain lacks rustfmt/Clippy. Exact successful Rust commands with
the existing local containers and dependency cache:

```bash
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh -- --nocapture
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -e CARGO_BUILD_JOBS=2 -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-desktop-tools:inventory /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
```

An intermediate collection test caught a named/positional SQL date-parameter collision;
it was fixed before the final run. The turnover fixture was corrected to expect exact
half-up rounding (1.104), and the large-value export fixture now aggregates several
valid posted documents instead of bypassing the existing per-document limit. Clippy's
single needless-borrow warning was fixed. The final suites above pass without warnings
from the application; Vite still reports the existing upstream Zod comment annotations.

Reporting tests cover historical balance-sheet equity, dated returns/reversals, unchanged
net-sales posting, independent stock/account differences, rate-classified VAT plus manual
adjustments, historical collections and future settlements, ABC pagination/thresholds,
90/180-day activity, negative stock aging, exact turnover, read-only monetary snapshots,
permissions, unsupported filters, malformed drill keys, whole-result summaries, 10,000-row
limits, overwrite protection and CSV/XLSX precision/formula safety.

The browser suite writes nine new one-page fixtures to `output/pdf/report-*.pdf` and
self-contained HTML to `output/print-fixtures/`. It checks row coverage, page overflow,
footer clearance and the actual Rust print snapshot. All nine PDFs were rendered with
Poppler and visually reviewed for Arabic shaping, headers, columns and pagination.
For example, the exact balance-sheet rendering command was:

```bash
pdftoppm -scale-to 1400 -singlefile -png output/pdf/report-balance_sheet.pdf output/pdf/verified-balance-sheet
```

The remaining eight report PDFs used the same arguments and a `verified-report-` prefix;
`pdfinfo` also opened each file successfully. These are Chromium/Fedora layout checks,
not Windows native/physical-printer acceptance. The browser screenshot of a portrait
preview can be obscured by its sticky toolbar; inspection used the actual PDF rendering.
Windows installation, native printing, idle RAM and cold-start targets remain unverified
and require [WINDOWS_RELEASE.md](WINDOWS_RELEASE.md). No Stage 3 posting is implemented.


The final Stage 2 source also produced a separate unsigned Windows NSIS candidate with
migration 0010. Exact successful commands:

```bash
docker run --rm --user 1000:1000 -e CARGO_NET_OFFLINE=true -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z plumbing-erp-windows:stage5 npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross --output-dir output/releases/reports-stage2
```

```text
output/releases/reports-stage2/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
Installer bytes: 3785016 (3.79 MB; below 20000000)
Installer SHA256: fbbe5a417646b15ab099b84d3e4d6dc4a08396a2efe3881c8e9bbcf94fcd3689
Application bytes: 13161472
Application SHA256: c10ece76020947e5fe5726cfdf9cd7ee6a6bcd08069e16d44e133fecb573625c
```

The verifier passed the x64 application payload, PE import checks, bundled-font check
and installer size. Adjacent `release-manifest.json` and `SHA256SUMS` contain the evidence.
The original and Stage 1 release directories were retained. Cargo used cached locked
dependencies; the bundler retrieved and hash-verified its official NSIS utility DLL.
Existing Clang compiler-detection/NSIS host warnings did not prevent the build. The
application still has no runtime network requirement. Cross-building this file does not
verify Windows installation, native interaction, physical printing or performance targets.
## Accounting improvement Stage 3 verification

The stage adds migration 0011, closing/reconciliation/receivable-adjustment services,
lazy finance pages and document-reference print sections. The existing 0001–0010
migrations were not edited. Generated schema, DTOs and installer labels were rebuilt
with their scripts. No dependencies were added. The Stage 2 installer remains intact
and does not include Stage 3 source changes.

Commands run from the repository root on Fedora (29 October 2026):

```bash
python3 scripts/export_schema.py
python3 scripts/generate_types.py
python3 scripts/generate_installer.py
npm run check
npm run format:check
npm test
npm run build
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
python3 scripts/generate_types.py --check
git diff --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh -- --nocapture
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -e CARGO_BUILD_JOBS=2 -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-desktop-tools:inventory /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
npm run test:ui
```

Backend: **55 passed**, including seven new tests for closing, loss/empty close,
explicit reopening and reclose, stale previews, fiscal locks, exact partial customer
adjustment/reversal, CSV precision and duplicate rejection, partial matching/unmatching,
immutability, source reversal protection, permissions and atomic rejection with no
document/audit/receipt residue. Python: **37 passed**. Frontend: **13 passed**.
TypeScript/i18n/generated-file checks, formatting, production web build and both core
and desktop Clippy (`-D warnings`) passed. An initial integration pass caught shared
translation-key regressions; existing report wording was restored before final reruns.

The Linux debug stock regression measured median receive/issue 158,225/211,351 ns at
1,000 movements and 141,800/192,994 ns at 200,000 movements. These are regression
measurements inside a transaction, not Windows latency/startup/RAM guarantees.
Windows installation, WebView2, native dialogs, physical printing and release
performance targets remain unverified and require `docs/WINDOWS_RELEASE.md` acceptance.

Final `npm run test:ui`: **14 passed** (2.2 minutes), including all existing invoice,
inventory and Stage 2 report workflows plus three new finance workflows. Screenshots
`output/finance-closing.png` and `output/finance-reconciliation.png` were visually
inspected for RTL layout. Customer-adjustment print preview is covered by the browser
test; this is not a physical-printer or Windows-native-print verification. Stage 3
verification completed before starting Stage 4 implementation.

## Purchasing and pricing Stage 4 verification

Migration 0012 adds planning, policy and append-only supplier-price/batch/margin/
landed-allocation evidence. Migrations 0001–0011 remain unchanged. New services and
lazy screens use the existing transaction/audit/receipt and exact arithmetic paths;
no dependencies were added. The final integration also corrects balance-sheet
prior/current result classification after reversing/reclosing a previous year, with
matching drill-down and historical as-of assertions. Net-sales postings and whole-store
inventory reconciliation remain as approved in Stage 2.

Commands run from the repository root on Fedora (29 October 2026):

```bash
python3 scripts/export_schema.py
python3 scripts/generate_types.py
npm run format
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all
npm run check
npm run format:check
npm test
npm run build
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
python3 scripts/generate_types.py --check
git diff --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh -- --nocapture
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -e CARGO_BUILD_JOBS=2 -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-desktop-tools:inventory /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
npm run test:ui
```

Backend: **65 passed**. The ten new Stage 4 tests cover immutable/idempotent supplier
history and reversal exclusion, net-sales velocity and micro ceilings, outstanding
orders and supplier grouping, stale previews, exact percentage/markup rounding, atomic
batch rollback, actual-cost/policy changes after draft, override reason/permission,
duplicate-product sequential issues, and value/quantity/weight landed allocation with
late COGS revaluation. Python: **38 passed**, including upgrade from version 11 with
posted purchase history backfill while documents, journals, stock ledger and balance
cache remain identical. Frontend: **13 passed**. TypeScript/i18n/generated-file checks,
formatting, web production build and core/desktop Clippy (`-D warnings`) passed.
The Vite build only reports harmless annotation warnings in existing Zod source.

The Linux debug stock regression measured median receive/issue 436,639/537,930 ns at
1,000 movements and 60,128/80,086 ns at 200,000 movements in this full-suite run. This
is a transaction-local regression check under host load, not a Windows performance
measurement. Windows installation, WebView2/native dialogs, physical printing, idle RAM
and cold-start targets still require real Windows acceptance in `WINDOWS_RELEASE.md`.

Final browser/core run: **17 passed** (1.8 minutes), including three new Stage 4
workflows for planning→draft POs, bulk prices→margin override, and weight allocation→
print preview. All earlier finance, reporting, inventory and sales workflows passed.
`output/reorder-preview.png` and `output/bulk-prices-preview.png` were visually inspected
for Arabic RTL layout. These browser tests use real Rust/SQLite but do not test native
Windows printing or a physical printer.

### Combined Stage 3–4 Windows candidate

After all final checks passed, the authorized Fedora/cargo-xwin build produced an
unsigned Arabic NSIS installer containing migrations 0001–0012 and both stages.
Commands executed:

```bash
docker run --rm --user 1000:1000 -e CARGO_NET_OFFLINE=true -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z plumbing-erp-windows:stage5 npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross --output-dir output/releases/accounting-purchasing-stages3-4
```

Both commands passed. Rust release compilation took 2m 49s. Cargo used locked cached
dependencies; Tauri retrieved and hash-validated its NSIS utility DLL during packaging.
The build reports the known clang-cl compiler-detection and cross-platform/signing
warnings; compilation and NSIS packaging completed. This is not a signed release.

```text
output/releases/accounting-purchasing-stages3-4/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
Installer bytes: 3898254 (3.90 MB / 3.72 MiB)
Installer SHA256: 7345c6c296b50d211e1a931bf2ad274f2c51e49eb6d428167624742cd41ce30b
Application bytes: 13654016
Application SHA256: 7173962461c25069e180857c13c477a1ed670d169d57dd1d13f23fe774e66df0
```

The verifier confirms an x64 application, x86 NSIS launcher, bundled offline Cairo,
no shipped source maps, no separate VC++/GCC runtime imports, Arabic-only installer
settings and size below 20,000,000 bytes. `release-manifest.json` and `SHA256SUMS`
are stored beside the installer. Original, Stage 1 and Stage 2 release directories
were preserved. Windows installation, native printing and runtime performance have
not been executed or measured; complete `WINDOWS_RELEASE.md` before store deployment.

## Plumbing inventory stage 5 verification

Verified locally on Fedora, 2 November 2026, using migration 0013 and the existing
cached toolchain images. Migration 0001–0012 content was preserved. No dependencies
were added. New variant, bundle and quarantine dialogs/pickers have separate lazy
chunks. `docs/STAGE5.md` records the implementation scope and design decisions.

Exact frontend/schema commands executed, all passing:

```bash
npm run check
npm run format:check
npm test
npm run build
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
python3 scripts/generate_types.py --check
git diff --check
```

Results: **14 frontend tests**, **39 Python tests**, successful TypeScript/Arabic-label/
generated-contract checks and production build. Python upgrade coverage compares all
old columns of posted documents, lines, stock, journals, products, warehouses and users
before/after migration 0013; historical warranties remain null. Existing upgrade
fixtures copy only columns present in the source schema version. Vite reports two
dependency comment-annotation warnings in Zod; the build completes without chunk-size
warnings. No dependency source was modified to suppress those warnings.

Rust commands executed sequentially (Docker supplies the pinned compiler/components):

```bash
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 bash scripts/test-backend.sh -- --nocapture
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-tools:stage2 cargo clippy --locked --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
docker run --rm --user 1000:1000 -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target -e CARGO_BUILD_JOBS=2 -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z -w /work plumbing-erp-desktop-tools:inventory /usr/local/rustup/toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/cargo-clippy clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
```

All passed: **73 Rust tests**, formatting and both Clippy configurations with warnings
denied. The eight new backend tests cover atomic variant creation/stale previews,
permissions and exact fractional bundle expansion, rollback after a second component
fails, idempotent posting/reversal, component returns after landed-cost revaluation,
frozen warranty terms including null quotation warranties, quarantine sale guards,
cost-preserving isolate/release, immutable metadata, shelf/bin values, classification
limits and prevention of reversal-based permission bypass. Deactivating a kit remains
possible after a component becomes inactive.

The full run's existing stock benchmark measured median transaction-local service
latency in debug mode: at 1,000 movements, receive **158,713 ns** / issue **229,828 ns**;
at 200,000 movements, receive **67,195 ns** / issue **99,270 ns**. Both remain within
the regression bounds; these host-load-sensitive figures are not Windows performance
or end-to-end document-posting measurements.

Browser commands executed:

```bash
npm run test:ui -- tests/ui/plumbing.spec.ts
npm run test:ui
```

The final full run passed **20 tests in 2.1 minutes**, including the three new Stage 5
workflows with the actual Rust core/SQLite. The focused run found and fixed a stale
warehouse-picker bootstrap after quarantine classification. Two test expectations
were corrected to use the existing six-decimal quantity editor and preview-close label.
Full rerun passed the original and new sales, finance, reports, purchasing, inventory
and printing cases. The warranty test changes the catalogue warranty after posting
and confirms the original term on both A4 and thermal; thermal print geometry is
checked for overflow. Print-media screenshots avoid the preview toolbar obscuring
the document. Arabic RTL screenshots were visually reviewed:

- `output/plumbing-variants.png`
- `output/plumbing-bundle.png`
- `output/plumbing-quarantine.png`
- `output/plumbing-warranty-a4.png`
- `output/plumbing-warranty-thermal.png`

These checks do not verify Windows installation, WebView2, native print/PDF dialogs,
physical store printers, idle RAM or cold-start targets. Run the acceptance procedures
in `WINDOWS_RELEASE.md` on a real Windows PC before store deployment.

### Plumbing Stage 5 Windows candidate

After the full verification passed, the authorized Fedora/cargo-xwin build produced
the unsigned Arabic NSIS candidate with migrations 0001–0013. Exact commands:

```bash
docker run --rm --user 1000:1000 -e CARGO_NET_OFFLINE=true -v /home/ma7moud-5aled/Projects/erp_plumbing_store:/work:Z plumbing-erp-windows:stage5 npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross --output-dir output/releases/plumbing-stage5
```

Both passed. Optimized Rust compilation took **2m 53s**. Cargo used the cached locked
dependency graph; Tauri downloaded and hash-validated its NSIS utility DLL during
packaging. The known clang-cl detection, cross-platform compilation, signing and
non-Win32 NSIS output-charset warnings did not prevent producing the installer.
This records a successful cross-build, not Windows execution or an offline build.

```text
output/releases/plumbing-stage5/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
Installer bytes: 3933560 (3.93 MB / 3.75 MiB)
Installer SHA256: 1e3f2e7fa4d051c63ab70ede1810cada7df0305b5a4579ecc319cb6f27c4884d
Application bytes: 13847040
Application SHA256: 0501e5f934f296730daa40fccb853d78a32e5b392f9582f6540ead2dfc22f9cb
```

The verifier passed: x64 application, x86 NSIS launcher, Arabic installer settings,
offline Cairo assets, no shipped source maps, no separate VC++/GCC runtime imports,
and installer size below 20,000,000 bytes. `release-manifest.json` and `SHA256SUMS`
are beside the installer. The original, inventory Stage 1, reports Stage 2 and combined
Stages 3–4 candidates remain untouched in their own release directories. No Windows
installation, physical printing or runtime-performance claim is made.
