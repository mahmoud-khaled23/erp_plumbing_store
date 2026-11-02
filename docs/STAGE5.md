# Plumbing inventory improvement Stage 5

Read first: README, architecture arithmetic/stock/document/schema contracts, backend
API/development notes, costing, inventory replay, document/return/finance posting,
reporting and existing migration guards. Stages 1–4 remain intact.

Implementation files:

1. `0013_plumbing_inventory.sql`, `db/mod.rs`, `models/requests.rs`, services
   `plumbing.rs`, `quarantine.rs`, dispatcher/reporting/administration and sale guards.
2. Lazy product variant/bundle/warranty screens, POS bundle picker, inventory
   quarantine workflow and shelf/bin fields, typed API and `ar.json`.
3. Historical warranty snapshots in document detail and A4/thermal print model;
   backend/schema/frontend/browser regression tests.
4. `ARCHITECTURE.md`, `BACKEND_API.md`, `README_AR.md`, developer/release records;
   full README checks before a separate Windows candidate.

Variants are ordinary products created atomically from one existing product template,
with independent codes/barcodes and frozen family links. A bundle is a non-stock sales
template: Rust expands quantities and current selected-unit prices, and the POS adds
ordinary component lines. Users review those components before saving; the bundle price
is their sum, and returns use the sold component lines. There is no synthetic stock
pool or second posting engine. Saved/posted lines do not depend on later template edits.

Shelf/bin information extends existing per-warehouse product settings. Quarantine is
an explicit warehouse classification: direct sales are blocked, transfers involving it
require permission and reason, and release uses normal cost-preserving stock transfers.
Damaged/short receipt quantities retain their existing meaning (not accepted stock).
Warranty is optional days per product, snapshotted with invoice lines, preserved for
linked returns and quotation conversion, and printed offline in Arabic.

Implemented in migration 0013, services `plumbing`/`quarantine`, lazy Arabic screens
and the existing print model. The inventory cache, net-sales discount posting and
whole-store reconciliation decisions remain intact. No dependency was added.
See the detailed [architecture contract](ARCHITECTURE.md#plumbing-inventory-stage-5-migration-0013),
[API payloads](BACKEND_API.md#plumbing-inventory-stage-5),
[Arabic walkthrough](../README_AR.md#المقاسات-والأطقم-والضمان-والعزل) and
[exact verification commands/results](DEVELOPMENT.md#plumbing-inventory-stage-5-verification).

The focused browser tests exposed a stale shared warehouse picker after classification;
the mutation now reloads bootstrap before subsequent transfer selection. They also
verify recipe expansion with fractional kit quantities, frozen warranty printing after
a catalogue change, and conservation of total inventory after isolate/release.

Final verification passed 73 Rust, 39 Python, 14 frontend and 20 browser/core tests,
both Clippy configurations and every README gate. The separate Fedora-built Windows
candidate is `output/releases/plumbing-stage5/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe`
(3,933,560 bytes); its manifest and SHA-256 are recorded in `DEVELOPMENT.md`.
Windows installation, native printing and runtime performance still require actual
Windows acceptance. Prior candidate installers remain preserved.
