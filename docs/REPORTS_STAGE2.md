# Reporting improvement Stage 2

Stage 2 is read-only. Existing stock/account postings and migrations 0001–0009 remain
unchanged. New report queries, drill-downs, print snapshots and exports share the same
validated dates and authorization in Rust. This document records the approved scope, report definitions and implementation boundaries.
Verification commands and results are recorded in DEVELOPMENT.md.

## Decision: discounts

Current postings credit sales with the amount after line/invoice discounts. The Stage 2
report can recover both discounts exactly from immutable document lines, including
returns and dated reversals, without changing journal entries.

**Approved by the user: retain net-sales postings for Stage 2.** A separate gross-sales change
could post a sale of 10,000 minor units with 1,000 discount as debit receivables/cash
9,000, debit existing `sales_discounts` 1,000, credit sales 10,000 (VAT and COGS retain
their existing rules). All amounts here are integer minor units, not decimal money.

That change would apply only to future documents. It needs an explicit posting-version
marker: returns of old net-posted sales must use the old presentation, and returns of
new gross-posted sales must reverse their original gross revenue and discount shares.
Posted history must never be rewritten. Required tests before shipping that change:

- Discount-free, line-only, invoice-only, combined and 100% discounts; exact largest
  remainder and balanced journal entries.
- Cash, credit, partial/split payments; unchanged VAT, receivables, inventory and COGS.
- Partial/full returns across both posting versions, including last-minor-unit rounding;
  full reversal and reversal of returns.
- Identical net profit before/after presentation change; discounts report agrees with
  frozen documents and, for the new version, the discounts account.
- Idempotency, rollback, permissions and fiscal locks.

No gross-sales posting change is authorized or implemented by this report stage alone.

## Decision: inventory reconciliation by warehouse

Existing `journal_lines` have no warehouse dimension; internal transfers deliberately
post no inventory-control journal because their total value is zero. Some late-cost
documents also revalue several warehouses together. An independent, fully attributed
warehouse general ledger cannot be reconstructed in every case without assumptions.

**Approved by the user: defer warehouse-specific accounting reconciliation until a
warehouse dimension is added to journal postings in a separate stage.** The earlier
proposal to infer allocations and bridge transfers was not adopted. Stage 2 implements
only an independent whole-store comparison of the stock-movement valuation with the
inventory control account, both as of the selected date. Its drill-down shows both
amounts for each source document and places discrepancies first. Internal transfers
net to zero; no invented warehouse allocations or account postings are created.

## Report definitions

| Report | Definition |
| --- | --- |
| Balance sheet | Assets = liabilities + equity. Equity includes posted equity accounts, the unclosed net result before the containing fiscal period, and income less expenses within that period through the as-of date. If no containing period exists, use the calendar-year start. Already closed income/expense balances contribute only their remaining balance, avoiding double counting. |
| Inventory reconciliation | Independent whole-store sums of posted stock values and inventory-control debit less credit through the as-of date. A nonzero difference is emphasized on screen, print and XLSX. CSV includes the difference column. |
| ABC | Rank period net sales from `v_product_profit`, including returns and dated reversals. Positive amounts alone form the denominator. A covers the first 80%, B the next 15%, C the rest. The item crossing a boundary remains in its starting class; zero/negative sales are C. Pagination never changes rankings or denominators. |
| Slow/dead stock | Nonzero product/warehouse pools as of the date; last non-reversed sale at that warehouse. Moving: under 90 days; slow: 90–179; dead: 180+; never sold and negative stock have explicit separate labels. A return does not erase the fact of a sale; a posted reversal excludes it only from that reversal date onward. |
| Stock aging | Analytical quantity residence age, consuming oldest entries first; buckets 0–30, 31–60, 61–90, 91+ days. Each inbound event, including transfer/return/reversal, starts a new age; value-only revaluations do not change ages. Incoming quantities cover negative deficits first. Not physical batch/lot tracing and not FIFO accounting; weighted-average costing stays unchanged. |
| Turnover | Period COGS divided by the average of opening (strictly before from-date) and closing valuations. Inventory days = that endpoint average × inclusive period days / period COGS. Ratios use exact integer intermediates and thousandths; undefined/negative valuation cases are blank, not fabricated zeroes. The displayed average rounds to a minor unit, while ratios use the unrounded endpoint sum. |
| Discounts granted | Frozen line and invoice discounts, pre-VAT, for discounted sales/returns and their dated reversals. Gross/net summary refers only to documents containing discounts. Net-sales journal postings stay unchanged. Optional customer filter. |
| Period VAT | Input/output taxable bases and tax amounts by frozen rate. Returns/reversals are signed events at their own dates. Manual tax adjustments without a known base/rate appear explicitly unclassified, with no invented taxable base. Totals reconcile to the two tax control accounts. |
| Collections | Historical unpaid customer debit items, bucketed by due date; 0–30 includes due today. Future settlements do not alter earlier snapshots. Unapplied credits are shown separately, not automatically allocated. Last receipt date is the latest non-reversed incoming receipt crediting customer receivables, including a recorded pending cheque, not a guarantee of clearance. Supplier refunds on dual-role parties do not count. Optional customer filter. |

For as-of reports, only the `to` date selects balances; period reports use inclusive
`from` and `to`. Both input dates are validated. Party filters are rejected on the new
reports that do not support them, rather than showing a misleading filter label.

## Read-only delivery contract

- Migration 0010 only grants `reports.export` and `reports.drilldown` to administrator
  and accountant. Existing permissions and posting rules are unchanged.
- Nine reports, their print snapshots and exports run the same backend computations
  in one SQLite read snapshot. Summary totals cover all matching results, not the
  current page. Screen/drill-down pages are bounded to 200 rows (UI uses 100).
- Drill-down opens source movements, journals or open items, then a read-only document
  with its journal. Financial and inventory read permissions are enforced in Rust.
- Printing/export includes all pages, with an explicit 10,000-row limit. Oversized
  results fail in Arabic without silent truncation or a partial export file.
- Export uses a native save dialog, refuses overwrite, writes UTF-8 BOM CSV or RTL
  XLSX, and audits the export filters. All numbers are exact decimal strings with
  Western digits in files; dates are dd/MM/yyyy. XLSX cells are strings intentionally
  to avoid Excel's precision limit; CSV neutralizes formula-like cell values.
- Export/print audit rows may be appended, but stock, documents, journals, settlements
  and mutation receipts are unchanged. The new details component is lazy-loaded,
  and no runtime dependency was added.

Regression tests cover historical results and reversal dates, VAT reconciliation,
independent inventory discrepancies, collection bucket boundaries, ABC pagination,
90/180-day activity thresholds, negative aging, exact turnover rounding, permissions,
malformed drill-down keys, export/print limits, large amounts and spreadsheet formulas.
Browser/core tests exercise all nine report screens, document/journal drill-down and
Arabic print layouts. These checks do not prove Windows installation, physical printing,
idle RAM or cold-start targets; those remain subject to WINDOWS_RELEASE.md.
