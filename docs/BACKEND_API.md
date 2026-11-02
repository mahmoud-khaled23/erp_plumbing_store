# Backend interface

The authoritative input types are `src-tauri/src/models/requests.rs`. Requests reject
unknown fields. Every `Int` is a signed decimal **string**, never a JSON number. IDs,
minor-unit prices, quantities, revisions and totals use this representation. Dates are
ISO calendar dates. Boolean options, bounded page sizes and basis-point rates use their
normal JSON types. Database query result integers also serialize as strings, including
SQLite 0/1 flag columns; the frontend wrapper must decode each field explicitly.

`src/api/contracts.ts` is generated from these request models using
`python3 scripts/generate_types.py`. `npm run check` rejects contract drift and validates
Arabic labels; `src/api/client.ts` also validates response shapes at runtime.

Authenticated bootstrap includes permitted reference data, the display name, fiscal
periods, document sequences and backup history. The `cash_desk` query requires
`cash.shift` and returns current cash balances and open shifts. Document reads include
remaining receipt/invoice/return quantities; journal and settlement rows require
`finance.read`. Product reads include per-warehouse reorder/bin settings.

## Commands

| Tauri command | Arguments | Effect |
| --- | --- | --- |
| `erp_query` | `query: Query` | Permission-checked read; unauthenticated bootstrap returns setup/recovery state only |
| `erp_setup` | `input: Setup` | Create store, first administrator and fiscal year in one transaction |
| `erp_login` | `username`, `password` | Create in-memory session; return user and permissions |
| `erp_logout` | none | Clear session and outstanding import grants |
| `erp_change_password` | `old`, `new` | Verify current password, replace hash, end session |
| `erp_create_user` | `username`, `name`, `password`, `roles` | Administrator creates a role-bound user |
| `erp_reset_password` | `userId`, `password` | Administrator resets another user; next login must change password |
| `erp_mutate` | `request: Request` | Atomic domain mutation with UUID idempotency key |
| `erp_file` | `action: FileAction` | Native file/folder selection; no caller-provided filesystem path |
| `erp_import_apply` | `token`, `replace` | Apply the exact previously previewed product file |
| `erp_import_opening_apply` | `token`, `balances`, `date`, `warehouseId` | Post opening imports through normal services |
| `erp_print` | `source: PrintSource`, `paper: PrintPaper` | Reauthorize source, open the native print dialog; no claim of physical print completion |
| `erp_export_pdf` | `source: PrintSource`, `paper: PrintPaper` | Native save dialog and atomic PDF export; returns `{path}` or `{cancelled:true}` |
| `erp_recover` | `username`, `password` | Select a backup and recover a damaged store using backup administrator credentials |
| `erp_frontend_ready` | none | Main-window readiness signal; writes a local measurement only when the launcher sets `PLUMBING_ERP_STARTUP_REPORT` |

Tauri's top-level Rust argument names use camelCase in JavaScript (`warehouseId`,
`userId`). Fields inside the serde request objects remain snake_case exactly as defined
in the Rust models. Enum discriminators are `type`; enum values use snake_case.

Example setup invocation arguments:

```json
{
  "input": {
    "store_name": "متجر السباكة",
    "username": "admin",
    "display_name": "مدير المتجر",
    "password": "ChooseYourOwnStrongPassword!",
    "fiscal_year": 2026,
    "demo": false
  }
}
```

This is an example payload, not a seeded credential. Passwords must contain 12–256
characters. Do not save credentials in Zustand persistence, browser storage or logs.

Example query arguments:

```json
{
  "query": {
    "type": "products",
    "search": "ماسورة",
    "after_id": "0",
    "limit": 50
  }
}
```

Example posting arguments after saving draft 42 at revision 1:

```json
{
  "request": {
    "request_id": "fa9d4cb3-5dc8-47eb-b87d-b8af98e8e90f",
    "mutation": {
      "type": "document_action",
      "id": "42",
      "revision": "1",
      "action": "post",
      "reason": "",
      "acknowledge_credit": false
    }
  }
}
```

Generate one UUID per intended operation. Retry an uncertain result with the same UUID
and identical payload. Reusing it with different contents or a different user is rejected.
Do not calculate a new UUID merely because the transport timed out. Posting rollback
leaves the saved draft intact. Successful posting returns the immutable document number.

A credit-limit exception returns `credit_warning` when explicit acknowledgement is
allowed; retry with a reason and `acknowledge_credit: true` after the user's confirmation.
Policy/permission blocks still return an error. Expired, reset or disabled sessions return
`unauthenticated`; the frontend prompts for the same user's credentials while keeping
the current in-memory invoice. Authentication data is not persisted in the webview.

## Workflows

- Products and parties: `save_product`, `save_party`, `save_master`. Updating a product
  requires its revision. Send all intended units; omitted old units become inactive.
- Sales: `save_draft` with `kind: sale`, then `document_action`. Hold/resume uses the
  current revision. Retail/wholesale price changes and discounts require permission.
- Quotations: save and approve, then `convert_quotation`; conversion creates an editable
  sales draft and closes the quote. It does not post a sale automatically.
- Purchasing: save/approve a purchase order, then goods-receipt drafts. A receipt line
  can reference an order line or be received without an order. Damaged and short amounts
  are retained separately; only accepted quantity enters stock.
- Supplier invoices: purchase lines match previously accepted receipt quantities.
  Invoice posting clears GRNI and updates AP/VAT/cost differences without receiving
  quantity a second time. Direct purchasing can save/post a no-PO receipt first.
- Payments: cash/bank/cheque legs support split and partial payment. Credit is the unpaid
  residual. A walk-in sale requires full payment; on-account sales require a customer.
- Returns: pass source line IDs and selected-unit quantities. The backend derives
  original prices, discounts, tax and adjusted COGS, and caps all cumulative returns.
  `warehouse_id` selects a purchase-return warehouse or overrides the original warehouse.
- Inventory: `adjustment`, `start_stocktake`, `count_stocktake`, then post the stocktake
  document. A changed movement snapshot aborts posting and requires a fresh count sheet.
- Finance: `journal`, `voucher`, `expense`, `cash_transfer`, `settle`, `unsettle`,
  `cheque_event`, `open_shift`, `close_shift`. A signed journal amount is debit when
  positive and credit when negative. Inventory-control balances require stock documents.
- Landed cost: `landed_cost` accepts exact minor-unit allocations to receipt lines.
  The arithmetic module provides largest-remainder allocation for value/quantity bases.
  Posting replays affected weighted-average pools and allocates current inventory versus
  expense changes without changing historical rows.
- Cancellation: cancel only drafts; posted documents use `reverse`. Dependencies and
  subsequent stock activity can require returns or current adjustments instead of direct
  reversal. Exact opposite postings and settlement unmatching preserve the audit trail.
- Administration: settings, sequences, account/category/unit/warehouse masters, fiscal
  period locking and user access are typed mutations with role checks.

All mutation and file methods recheck authorization in Rust. Password-reset sessions
can read bootstrap state and change their password but cannot write or export data.
Financial reports require report permission; dashboard financial sections are also
restricted by finance permission.

## CSV/Excel and opening templates

Product export produces one row per product unit, including its barcode and prices.
Import accepts `.csv`, `.xlsx` and `.xls`. Export accepts CSV and `.xlsx` and writes
spreadsheet text cells safely. The exported header order is the import contract:

```csv
code,name_ar,category_id,brand,material,size,base_unit_id,standard_cost_minor,min_stock_micros,active,unit_id,factor_numerator,factor_denominator,barcode,retail_price_minor,wholesale_price_minor,unit_active
PPR20,ماسورة مياه,1,محلي,PPR,20 مم,1,1000,5000000,1,1,1,1,6220000000015,1500,1400,1
```

Product prices in that template are integer minor units; stock thresholds are quantity
micros. Identifiers refer to the store's existing categories and units. Preview returns
the first 100 products and a total count; apply validates the unchanged file, checks
every row and writes the entire batch in one transaction. The input limit is 20 MB and
100,000 rows. Re-importing an identical successful file is blocked by its checksum.

Opening stock uses human decimal quantities and prices, parsed as exact integers:

```csv
product_code,unit_id,quantity,unit_cost
PPR20,1,2.5,10.00
```

Opening balances use account codes and optional party codes. Positive amounts are
debits; negative amounts are credits. The system adds opening equity to balance the
journal. A receivable/payable account requires a compatible party.

```csv
account_code,party_code,amount,due_date
1200,CUSTOMER-01,100.25,2026-11-01
2100,SUPPLIER-01,-250.00,2026-11-15
1101,,500.00,2026-10-27
```

Opening files are previewed before posting. Large files become multiple bounded
documents inside one transaction, so any invalid row rolls back the entire import.
Dates must belong to an open fiscal period. The template date examples are illustrative.

## Reporting and errors

Inventory maintenance accepts `{"query":{"type":"stock_integrity"}}` via `erp_query`.
It requires `inventory.verify` (admin, storekeeper or accountant), checks **all**
product/warehouse pools, and returns `{"pools":"12","movements":"200000"}` with
decimal-string counts. It is read-only, uses one consistent read transaction, and does
not append audit/idempotency records or repair data. The inventory screen exposes this
as **فحص أرصدة المخزون**, with progress and a persistent result/error dialog.

Migration 0009 backfills `stock_balances` from the immutable ledger; insertion triggers
maintain quantity, value and last movement for every stock path, within the existing
posting transaction. No new stock-writing command is exposed. Costing pool lookups and
availability/date guards use the cache. Historical reports remain ledger-based.

Startup and backup/restore checks also verify the cache. Any missing/extra pool,
quantity/value difference, missing cache table or wrong last movement fails with
`stock_balance_mismatch` and an Arabic message. A maintenance mismatch clears the session
and import grants and sets `recovery_required:true` in bootstrap; login/writes remain
blocked until recovery. Failed validation never rebuilds balances automatically.

Queries provide keyset pagination for catalogue/documents/stock and bounded report
pagination. Stock without a selected warehouse is aggregated per product, so its cursor
cannot skip a warehouse row. A statement includes an opening balance and running balances;
historical aging excludes later payments and settlements. Credits/advances are reported
separately from overdue debt. The `over_90_minor` bucket starts at day 91.

All command failures return `code`, Arabic `message` and `retryable`. Constraint messages
are Arabic; raw SQLite queries, internal paths and credentials are not returned. A `busy`
result is retryable. Validation, revision conflicts, credit warnings and insufficient
stock require user correction or the explicit authorized acknowledgement.

### Stage 2 report queries, drill-down and exports

`Query::Report` adds `balance_sheet`, `inventory_reconciliation`, `stock_abc`,
`stock_activity`, `stock_aging`, `stock_turnover`, `discounts_granted`, `vat_return`,
and `collections`. These nine return `{rows, opening: [], summary}`; `summary` is an
array of `{metric, amount_minor}` with whole-report totals. Old array reports and
party-statement opening rows retain their contracts. All row values are decimal strings,
text or null; an undefined turnover or unknown VAT base/rate is null, never zero.

Each new row contains `drill_key`. Send the unchanged report filter and this opaque key:

```json
{"query":{"type":"report_drilldown","filter":{"report":"balance_sheet","from":"2026-01-01","to":"2026-10-27","party_id":null},"key":"account:120","limit":100,"offset":0}}
```

The result is `{rows, columns}`; rows carry `document_id` for the existing `document`
query. The detail view shows movements, journal entries or unpaid items, then the source
document and its journal. Keys are validated per report and SQL parameters are bound.
Require `reports.read` and `reports.drilldown`, plus `finance.read` for financial/ABC
entries or `inventory.read` for stock movements; document reads retain their existing
kind-specific authorization. Limits clamp to 1–200; UI uses pages of 100.

`ReportFilter` contains `{report, from, to, party_id}`. Dates must be valid ISO dates
with `from <= to`. Balance sheet, reconciliation, activity, aging and collections are
as-of `to`; other new reports use inclusive date ranges. Only collections and discounts
accept `party_id` among the new reports; unsupported filters fail in Arabic. The user
approved net-sales postings unchanged and deferred warehouse accounting reconciliation;
`inventory_reconciliation` is **whole-store only**. See [report definitions](REPORTS_STAGE2.md)
for ABC thresholds, residence-age assumptions, VAT residuals and last-receipt meaning.

Export with `erp_file` and action
`{type:"export_report", filter:ReportFilter, excel:boolean}`. This requires `reports.read`
and `reports.export`; migration 0010 grants the two new actions to admin/accountant.
A native save dialog selects the destination; IPC accepts no arbitrary output path.
Cancellation returns `{cancelled:true}`. Success returns `{rows:"count", path:"selected"}`.
The same backend query runs in one snapshot and exports **all** matching rows and summary,
including Arabic headers, store, currency, dates and methodological notes. More than
10,000 rows fails before creating a file; existing files cannot be overwritten. Write
failure removes only the newly created incomplete destination. Success appends an
`export_report` audit event with filters without changing any ledger or posted document.

CSV is UTF-8 BOM and guards formula-like cells. XLSX is RTL with frozen headings, filters
and highlighted discrepancies. Every numeric cell is an exact decimal **string** (currency
minor digits, quantity six places, ratios three places, percentage two places); files use
Western digits and dd/MM/yyyy dates. This intentionally avoids Excel precision loss.
Do not silently coerce exported large values to floating-point numbers on re-import.

## Accounting improvement stage 3

All identifiers and monetary values below are integer strings. New mutations use
the existing `erp_mutate` request UUID, immediate transaction, audit and receipt.

| Operation | Payload / result | Permission |
|---|---|---|
| `closing_preview` query | `period_id,date` → `rows,preview_hash,result_minor` | `finance.close` |
| `close_year` mutation | `period_id,date,preview_hash,reason` → posted document | `finance.close` |
| `reverse` on a close | Existing reversal payload, date in next open contiguous period | `documents.reverse` + `finance.close_reverse` |
| `customer_open_items` query | `search,after_id,limit` → receivable debit rows with residuals | `finance.read` |
| `receivable_adjustment` mutation | `kind,date,debit_line_id,amount,reason` | `finance.writeoff`, `finance.credit_note` or `finance.settlement_discount` |
| `bank_statements` query | `after_id,limit` → statement headers/status | `finance.reconcile` |
| `parse_statement_csv` query | `csv` → `{date,amount,reference,description}[]` | `finance.reconcile` |
| `save_bank_statement` mutation | `statement:{cash_account_id,starts_on,ends_on,reference,opening,closing,lines}` | `finance.reconcile` |
| `reconciliation` query | `statement_id,view,limit,offset` → `rows,summary` | `finance.reconcile` |
| `match_statement` mutation | `statement_line_id,journal_line_id,amount,date,reason` | `finance.reconcile` |
| `unmatch_statement` mutation | `match_id,date,reason` | `finance.reconcile` |
| `void_bank_statement` mutation | `statement_id,date,reason` | `finance.reconcile` |

Adjustment kinds: `bad_debt`, `customer_credit`, `settlement_discount`; reversal also
requires the kind-specific permission. No stock or VAT effects. Reconciliation views:
`statement`, `journal`, `matches`. Match amounts are positive; unmatch derives the
exact negative amount internally. A statement with active matches cannot be voided.
`bank_match` cannot be passed to the generic reversal command. CSV header is exactly
`date,amount,reference,description`, ISO dates and signed decimal currency amounts.
All new actions are granted to admin/accountant except close reversal (admin only).
The `document` result optionally adds `finance_context` rows, also available in
authorized print snapshots. Closing preview is stale if relevant balances/periods
change, and must be recomputed before posting.

## Purchasing and pricing stage 4

All new mutations use the normal UUID request, authorization, immediate transaction,
audit and idempotency receipt. IDs, micros, monetary values, revisions, grams and signed
`rate_bps` are integer strings. Bounded day counts and `minimum_margin_bps` are JSON
integers. Queries that paginate clamp `limit` to 1–200; the UI uses 50.

| Operation | Payload / result | Permission |
|---|---|---|
| `product_planning` query | `product_id` → planning row and nullable latest base-unit purchase cost | `products.read` |
| `save_product_planning` mutation | `planning:{product_id,revision,supplier_id,reorder_point,max_level,lead_time_days,weight_grams},reason` | `purchases.plan` |
| `supplier_prices` query | `product_id,after_id,limit` → frozen purchase rows, derived costs/status | `purchases.read` |
| `reorder_suggestions` query | `filter,after_id,limit` → current stock/orders, net sales, trigger, target, suggestion and supplier cost | `purchases.plan` |
| `suggested_orders_preview` query | `filter,product_ids` → `rows,preview_hash` | `purchases.plan` |
| `create_suggested_orders` mutation | `filter,product_ids,preview_hash,reason` → `{document_ids}` | `purchases.plan` + `purchases.write` |
| `pricing_policy` query | no fields → singleton row | `products.read` |
| `save_pricing_policy` mutation | `revision,minimum_margin_bps,reason` | `pricing.manage` |
| `price_units` query | `search,after_id,limit` → active unit prices | `products.price_update` |
| `price_preview` query | `unit_ids,rule` → `rows,preview_hash` | `products.price_update` |
| `apply_prices` mutation | `unit_ids,rule,preview_hash,reason` → `{id,count}` (batch ID) | `products.price_update` |
| `sale_margins` query | `warehouse_id,lines,discount` → per-line cost, net, profit, margin and override requirement | `sales.write` |
| `landed_preview` query | `basis,amount,receipt_line_ids` → `rows,preview_hash` | `purchases.write` |
| `allocate_landed_cost` mutation | `date,supplier_id,description,basis,amount,receipt_line_ids,preview_hash` → posted document | `purchases.write` |

`filter` is `{warehouse_id,through,lookback_days,coverage_days}`. Through is ISO date;
lookback and coverage are 1–365 days. Stock/open orders/prices are current, while the
sales window ends at through. `product_ids` has 1–200 unique IDs. Draft POs use base
units, one per supplier, no payments or VAT until reviewed through the normal editor.
`supplier_prices.after_id` is the last purchase-line ID (descending); other new cursors
are ascending product or unit IDs. Pass `"0"` to start. History's last cost excludes
reversed purchases and VAT/landed costs; null means no invoice. Suggestions expose
`suggested_unit_cost_minor` separately for standard-cost fallback.

`rule` is `{basis:"percentage"|"cost_markup",price_list:"retail"|"wholesale"|"both",rate_bps}`.
At most 500 unique units; percentage range is −10000 to 1000000 bps; markup cannot be
negative. All prices are bounded minor units with half-up integer rounding. Price/order/
landed hashes are recomputed during mutation; stale data returns an Arabic validation
error without side effects. Planning and policy saves use optimistic revisions.

Margin `lines` use existing `DraftLine` objects; `discount` is the invoice discount.
The response is advisory and simulates sequential issues. Actual sale posting rechecks
actual issue cost and the current policy and requires `sales.discount` plus document/
posting reason for below-cost or below-minimum sales. Margin is pre-VAT and after all
discounts. The reason supplied to `post` is saved on the draft within the transaction
before posting. Existing quotation, hold/resume and return pipelines are preserved.

Automatic landed basis is `value`, `quantity` or `weight`; at most 1000 unique positive
posted/unreversed receipt lines. Weight requires positive configured grams per base
unit. Manual allocation keeps the existing `landed_cost` mutation with explicit shares.
Both paths reuse `revalue`; `document.finance_context` and print snapshots expose the
actual allocation method and amount. New planning/pricing permissions are granted to
admin/accountant by migration, never by frontend state.

## Plumbing inventory stage 5

All examples below are `erp_query` payloads or `Mutation` payloads inside the existing
UUID request envelope. IDs, revisions, prices and quantities are decimal strings;
`days` is a bounded integer or null. All mutations retain one transaction, audit entry
and idempotency receipt. No new filesystem/network command or posting engine is added.

| Query | Fields | Response / permission |
| --- | --- | --- |
| `variant_preview` | `template_id`, `variants:[{code,size,material,barcode}]` | `{rows,preview_hash}`; `products.variants` |
| `product_variants` | `template_id,after_id,limit` | Direct child rows, cursor `product_id`; `products.read` |
| `bundles` | `search,after_id,limit` | Headers including inactive, cursor `id`; `products.read` |
| `bundle` | `id` | `{header,items}`; items include `unit_id,product_id,quantity_micros`, names/conversions; `products.read` |
| `expand_bundle` | `id,quantity,wholesale` | `{rows,total_minor}`; `sales.write` |

Variant previews accept 1–100 combinations. Template active units/prices, warranty and
planning are hashed; a stale preview is rejected. Base-unit barcodes are optional;
alternate barcodes are cleared. Created products have independent prices and stock.
`product_variants` is an immutable origin link; size/material cannot subsequently change.
All cursor queries clamp `limit` to 1–200; the UI requests 50.

| Mutation | Fields | Permission and result |
| --- | --- | --- |
| `create_variants` | `template_id,variants,preview_hash,reason` | `products.variants` + `products.write`; `{product_ids,count}` |
| `save_bundle` | `bundle:{id,revision,code,name,active,items:[{unit_id,quantity}]},reason` | `products.bundles`; `{id}` |
| `set_warranty` | `product_id,revision,days,reason` | `products.warranty`; `{id}`; reread product for new revision |
| `set_quarantine_warehouse` | `warehouse_id,revision,quarantine,reason` | `inventory.quarantine_manage`; `{id}` |
| `quarantine_transfer` | `date,from_id,to_id,lines:[{unit_id,quantity}],reason` | `inventory.quarantine` + `inventory.transfer`; normal posted transfer result |

`save_bundle` uses null ID/revision `"0"` for creation and current revision for edits.
It accepts 1–100 distinct real product units, each with positive quantity in millionths
of that selected unit. Bundles cannot contain bundles. `expand_bundle.quantity` is
millionths of a kit; output component quantities are selected-unit micros, not base
micros. Expansion uses current active component prices and exact rational conversion,
returning `unit_price_minor`, `subtotal_minor`, `bundle_name` and product/unit IDs.
`total_minor` is the sum before taxes/discounts. Expansion is read-only. The caller adds
ordinary `DraftLine` values and a bundle-name note, then uses normal save/post. The
user can edit individual lines; no parent kit quantity or fixed kit price is persisted.
Linked returns select the original component lines, even after the recipe changes.

`set_warranty.days` accepts null or 1–36500 and increments the product revision. Product
queries include `warranty_days` and nullable `template_product_id`. New sale/quotation
lines snapshot warranty; linked returns and quotation conversion preserve source terms.
Draft edits regenerate terms from current products; posted lines never change. Historical
lines from before migration 0013 remain null. Document/print line rows include
`warranty_days`, rendered on A4 and thermal receipts without extra print permissions.

Bootstrap warehouse rows include `is_quarantine` and `quarantine_revision`; use that
revision for classification changes and reload bootstrap after success. Existing
`save_master.warehouse` cannot change classification. Removing quarantine requires
zero quantity and value in all warehouse pools. The focused transfer requires exactly
one normal and one quarantine warehouse, then saves/posts a normal transfer in the
same transaction. Generic transfers involving quarantine also require permission and
reason. Direct sale/margin preview is rejected for a quarantine warehouse. Other
outgoing movements and reversal enforce quarantine permission in addition to their
existing permissions. Every posted quarantine transfer freezes `from_quarantine` and
`to_quarantine`, exposed in document/print headers even if classification later changes.

`save_master.warehouse_product` adds optional `shelf` alongside `bin` (both at most 120
characters). Null/omitted shelf preserves an existing value; empty clears it. Inventory
rows add `shelf_location,bin_location,available_quantity_micros,quarantine_quantity_micros`.
Locations are null for all-warehouse queries. Total quantity/value includes quarantine;
available quantity includes only active normal warehouses, with no draft reservations.
Rejected/damaged goods-receipt quantities are still excluded from owned stock.

## Print snapshot contract

`erp_query` accepts `{"type":"print_snapshot","source":{"type":"document","id":"42"}}`.
Other sources are `{"type":"labels","unit_ids":["1","2"]}`,
`{"type":"stock_card","product_id":"1","warehouse_id":"1"}`, and
`{"type":"report","report":"party_statement","from":"2026-01-01","to":"2026-12-31","party_id":"1"}`.
A report without a party uses `party_id:null`; party statements require a party.

The response contains `document` (nullable), `rows`, `opening`, `context`, `references`,
`store`, `preferences` and `generated_at`. All financial values remain integer strings.
New report print snapshots include `summary` with the same whole-report totals as the
screen/export and emphasize discrepancies. Documents preserve historical store and line snapshots; reports/labels use current
settings. Snapshots load all selected report rows up to 10,000, reject overflow and
require the same source permission as the on-screen query. Labels accept 1–100 unique
active product units with barcodes. Read operations never mutate ledgers.

Both print commands take the same `source` and a `paper` object, for example
`{"width_mm":210,"height_mm":297}`. Allowed sizes are A4 portrait/landscape or width
80 mm with integer height 60–800 mm. Only the main Tauri window may invoke them. Each authorized output request appends
`print_requested` or `pdf_export_requested` to the audit log, including its source.
This records intent rather than physical printer success or completion of a save dialog.
No output path or arbitrary HTML is accepted over IPC. Native dialogs select the
PDF destination, and existing files cannot be overwritten. See [PRINTING.md](PRINTING.md)
for the template, snapshot and native-export implementation.
