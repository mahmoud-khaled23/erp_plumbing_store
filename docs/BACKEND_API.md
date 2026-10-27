# Backend interface for the React stage

The authoritative input types are `src-tauri/src/models/requests.rs`. Requests reject
unknown fields. Every `Int` is a signed decimal **string**, never a JSON number. IDs,
minor-unit prices, quantities, revisions and totals use this representation. Dates are
ISO calendar dates. Boolean options, bounded page sizes and basis-point rates use their
normal JSON types. Database query result integers also serialize as strings, including
SQLite 0/1 flag columns; the frontend wrapper must decode each field explicitly.

Stage 3 generates `src/api/contracts.ts` from these request models using
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
| `erp_recover` | `username`, `password` | Select a backup and recover a damaged store using backup administrator credentials |

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

Queries provide keyset pagination for catalogue/documents/stock and bounded report
pagination. Stock without a selected warehouse is aggregated per product, so its cursor
cannot skip a warehouse row. A statement includes an opening balance and running balances;
historical aging excludes later payments and settlements. Credits/advances are reported
separately from overdue debt. The `over_90_minor` bucket starts at day 91.

All command failures return `code`, Arabic `message` and `retryable`. Constraint messages
are Arabic; raw SQLite queries, internal paths and credentials are not returned. A `busy`
result is retryable. Validation, revision conflicts, credit warnings and insufficient
stock require user correction or the explicit authorized acknowledgement.

Printing consumes the frozen `Query::Document` snapshots. Native printing/PDF commands
will be added with the complete stage 4 templates; no simulated print-success command
is present in stage 2.
