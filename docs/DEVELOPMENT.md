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
