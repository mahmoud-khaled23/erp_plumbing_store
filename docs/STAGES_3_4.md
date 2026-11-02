# Accounting and purchasing stages

Implementation order: complete and verify stage 3 before stage 4. Existing
stage 1/2 migrations and posted history remain unchanged.

Stage 3 files: `0011_period_close_reconciliation.sql`, `db/mod.rs`,
`models/requests.rs`, services `closing.rs`, `reconciliation.rs`,
`receivable_adjustments.rs`, dispatcher/reporting/returns, typed API,
lazy finance screens, Arabic labels, backend/UI tests and documentation.

Stage 4 files: `0012_purchasing_pricing.sql`, `services/purchasing.rs`,
`services/pricing.rs`, document posting and report/print integration, typed API,
`ProductPlanningDialog`, `ReorderPage`, `BulkPricesDialog`, `LandedCost`, POS margin
display, Arabic labels, backend/schema/UI tests and documentation. Final integration
also aligns balance-sheet result/drill-down classification with prior-period closing
reversals; this changes only read queries.

Year-end reversal is dated in the immediately following open period; the
original period and entries remain locked. Reclosing after a reversal is
also dated in that next period and includes the original close/reversal
effects. Profit and loss excludes closing entries and their reversals.
Customer adjustments are financial documents with no stock or automatic
VAT correction. They allocate a credit against one existing receivable.
Bank matching and unmatching are immutable events, never journal edits.

Both stages are implemented. Stage 3 passed the full verification suite before Stage 4
began; the combined Stage 4 source then passed 65 Rust, 38 Python, 13 frontend and 17
browser/core tests. Exact commands and results are recorded in `DEVELOPMENT.md`.
Windows deployment acceptance remains separate from Fedora cross-compilation.
