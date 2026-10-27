# Arabic Plumbing Store ERP

A staged implementation of an offline Arabic RTL desktop ERP using Tauri 2, Rust,
rusqlite, React 18, TypeScript, Vite and Tailwind CSS.

**Current delivery: stage 3 — Arabic React desktop screens connected to Rust/SQLite.**
The source includes all module screens, first-run setup, permissions and keyboard POS.
Development supports Fedora; release packaging targets Windows MSVC/NSIS. Print/PDF
templates are stage 4; a tested Windows installer and performance measurements are stage 5.
This is a staged implementation, not a finished production release.

- [Fedora development, Docker checks and Windows build path](docs/DEVELOPMENT.md)
- [Frontend implementation, screens, keyboard shortcuts and test instructions](docs/FRONTEND.md)
- [Backend commands, workflows and import templates](docs/BACKEND_API.md)
- [Typed request models](src-tauri/src/models/requests.rs)
- [Transaction/session boundary](src-tauri/src/state.rs)
- [Domain service dispatch](src-tauri/src/services/mod.rs)
- [Rust workflow tests](src-tauri/tests/backend.rs)

After installing the Fedora desktop prerequisites, Rust and Node.js >=22.12:

```bash
npm ci
npm run desktop
```

The lockfile was created with npm 11.21.1. If Fedora's npm 10 reports an `edgesOut`
resolver error, use npm 11 as described in the development guide. `npm run dev` starts
only Vite; database commands require the Tauri desktop. There is no development server
or Node.js runtime in the installed application.

Run the backend tests with host Rust or the Docker fallback:

```bash
bash scripts/test-backend.sh
```

## Delivered files

- [Architecture, workflows and final folder structure](docs/ARCHITECTURE.md)
- [Complete executable SQLite schema](docs/schema.sql)
- [Connection configuration](src-tauri/db/connection.sql)
- [Foundation migration](src-tauri/migrations/0001_foundation.sql)
- [Documents migration](src-tauri/migrations/0002_documents.sql)
- [Ledgers migration](src-tauri/migrations/0003_ledgers.sql)
- [Posting and integrity guards](src-tauri/migrations/0004_guards.sql)
- [Detail protection and immutable-ledger guards](src-tauri/migrations/0005_detail_guards.sql)
- [Reporting views](src-tauri/migrations/0006_views.sql)
- [Arabic reference data and role permissions](src-tauri/migrations/0007_reference_data.sql)
- [Late-cost attribution migration](src-tauri/migrations/0008_cost_attribution.sql)
- [Historical aging query](src-tauri/queries/aging.sql)
- [Schema contract tests](tests/test_schema.py)
- [Schema reference generator](scripts/export_schema.py)

## Verify stage 1

Python 3 with SQLite >=3.37 and FTS5 is sufficient for the schema tests. Application
production builds will bundle a patched SQLite >=3.51.3 through rusqlite.

```bash
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
```

After editing an unreleased migration, regenerate the full schema reference:

```bash
python3 scripts/export_schema.py
```

Optional: create a disposable database using the SQLite CLI, if installed:

```bash
sqlite3 /tmp/plumbing-schema-review.db < docs/schema.sql
```

Use a new, empty file. This reference script creates reference data but no administrator,
fiscal period, demo transactions or plaintext credentials. The stage 2 setup command
creates the administrator and fiscal period; `demo: true` adds 100 Arabic products.
The first-run screen offers the same optional demo flag. Do not edit an applied production migration.

## Delivery order

1. Architecture and complete versioned SQLite schema — delivered.
2. Rust models, services, Tauri commands and optional demo data — delivered.
3. React layout, shared components and all module screens — delivered.
4. Arabic A4, thermal, goods-received, labels and report print templates.
5. Application tests, locked build configuration and Windows NSIS installer commands.
6. Arabic user guide (`README_AR.md`) and final developer README.

## Installer assumption

The <20 MB installer target assumes a preinstalled WebView2 runtime. A self-contained
offline installer embedding that runtime is substantially larger. RAM and cold-start
targets require measurement on Windows; no performance result is claimed yet.
See the architecture document for the deployment decision and official references.
