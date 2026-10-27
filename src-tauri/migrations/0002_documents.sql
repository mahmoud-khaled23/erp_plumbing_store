CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL REFERENCES document_types(code),
    number TEXT UNIQUE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'held', 'approved', 'posting', 'posted', 'closed', 'cancelled')),
    document_date TEXT NOT NULL CHECK (length(document_date) = 10 AND date(document_date, '+0 days') IS document_date),
    due_date TEXT CHECK (due_date IS NULL OR (length(due_date) = 10 AND date(due_date, '+0 days') IS due_date)),
    party_id INTEGER REFERENCES parties(id),
    warehouse_id INTEGER REFERENCES warehouses(id),
    destination_warehouse_id INTEGER REFERENCES warehouses(id),
    source_document_id INTEGER REFERENCES documents(id),
    reversal_of_id INTEGER REFERENCES documents(id),
    price_list TEXT NOT NULL DEFAULT 'retail' CHECK (price_list IN ('retail', 'wholesale')),
    currency_code TEXT NOT NULL CHECK (length(currency_code) = 3),
    currency_symbol TEXT NOT NULL,
    currency_minor_digits INTEGER NOT NULL CHECK (currency_minor_digits BETWEEN 0 AND 3),
    party_name_snapshot TEXT NOT NULL DEFAULT '',
    party_address_snapshot TEXT NOT NULL DEFAULT '',
    party_tax_number_snapshot TEXT NOT NULL DEFAULT '',
    store_snapshot_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(store_snapshot_json)),
    subtotal_minor INTEGER NOT NULL DEFAULT 0 CHECK (subtotal_minor BETWEEN 0 AND 1000000000000000),
    line_discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (line_discount_minor BETWEEN 0 AND subtotal_minor),
    invoice_discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (invoice_discount_minor BETWEEN 0 AND subtotal_minor - line_discount_minor),
    tax_minor INTEGER NOT NULL DEFAULT 0 CHECK (tax_minor BETWEEN 0 AND 1000000000000000),
    total_minor INTEGER NOT NULL DEFAULT 0 CHECK (total_minor BETWEEN 0 AND 2000000000000000),
    notes TEXT NOT NULL DEFAULT '',
    reason TEXT NOT NULL DEFAULT '',
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    posted_by INTEGER REFERENCES users(id),
    posted_at TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    CHECK (total_minor = subtotal_minor - line_discount_minor - invoice_discount_minor + tax_minor),
    CHECK (due_date IS NULL OR due_date >= document_date),
    CHECK (warehouse_id IS NULL OR destination_warehouse_id IS NULL OR warehouse_id <> destination_warehouse_id),
    CHECK ((kind = 'reversal') = (reversal_of_id IS NOT NULL)),
    CHECK (source_document_id IS NULL OR source_document_id <> id),
    CHECK (reversal_of_id IS NULL OR reversal_of_id <> id),
    CHECK ((status = 'posted') = (posted_at IS NOT NULL AND posted_by IS NOT NULL))
) STRICT;
CREATE INDEX idx_documents_kind_date ON documents(kind, document_date, id);
CREATE INDEX idx_documents_status_date ON documents(status, document_date);
CREATE INDEX idx_documents_party_date ON documents(party_id, document_date, id);
CREATE INDEX idx_documents_warehouse ON documents(warehouse_id);
CREATE INDEX idx_documents_destination ON documents(destination_warehouse_id);
CREATE INDEX idx_documents_source ON documents(source_document_id);
CREATE INDEX idx_documents_reversal ON documents(reversal_of_id);
CREATE UNIQUE INDEX idx_documents_active_reversal ON documents(reversal_of_id) WHERE reversal_of_id IS NOT NULL AND status <> 'cancelled';
CREATE UNIQUE INDEX idx_documents_quote_conversion ON documents(source_document_id) WHERE kind = 'sale' AND source_document_id IS NOT NULL AND status <> 'cancelled';
CREATE INDEX idx_documents_created_by ON documents(created_by);
CREATE INDEX idx_documents_posted_by ON documents(posted_by);
CREATE INDEX idx_documents_due ON documents(due_date) WHERE due_date IS NOT NULL;

CREATE TABLE document_lines (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    line_number INTEGER NOT NULL CHECK (line_number BETWEEN 1 AND 1000),
    product_id INTEGER NOT NULL REFERENCES products(id),
    product_unit_id INTEGER NOT NULL,
    source_line_id INTEGER REFERENCES document_lines(id),
    product_code_snapshot TEXT NOT NULL,
    product_name_snapshot TEXT NOT NULL,
    unit_name_snapshot TEXT NOT NULL,
    category_id_snapshot INTEGER NOT NULL REFERENCES categories(id),
    factor_numerator INTEGER NOT NULL CHECK (factor_numerator BETWEEN 1 AND 1000000000),
    factor_denominator INTEGER NOT NULL CHECK (factor_denominator BETWEEN 1 AND 1000000000),
    quantity_micros INTEGER NOT NULL CHECK (quantity_micros BETWEEN 0 AND 1000000000000000),
    base_quantity_micros INTEGER NOT NULL CHECK (base_quantity_micros BETWEEN 0 AND 1000000000000000),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor BETWEEN 0 AND 1000000000000),
    subtotal_minor INTEGER NOT NULL CHECK (subtotal_minor BETWEEN 0 AND 1000000000000),
    line_discount_minor INTEGER NOT NULL DEFAULT 0 CHECK (line_discount_minor BETWEEN 0 AND subtotal_minor),
    invoice_discount_share_minor INTEGER NOT NULL DEFAULT 0 CHECK (invoice_discount_share_minor BETWEEN 0 AND subtotal_minor - line_discount_minor),
    tax_rate_bps INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bps BETWEEN 0 AND 10000),
    tax_minor INTEGER NOT NULL DEFAULT 0 CHECK (tax_minor BETWEEN 0 AND 1000000000000),
    total_minor INTEGER NOT NULL CHECK (total_minor BETWEEN 0 AND 2000000000000),
    notes TEXT NOT NULL DEFAULT '',
    UNIQUE (document_id, line_number),
    UNIQUE (id, document_id),
    UNIQUE (id, document_id, product_id),
    FOREIGN KEY (product_unit_id, product_id) REFERENCES product_units(id, product_id),
    CHECK (total_minor = subtotal_minor - line_discount_minor - invoice_discount_share_minor + tax_minor),
    CHECK (source_line_id IS NULL OR source_line_id <> id)
) STRICT;
CREATE INDEX idx_document_lines_product ON document_lines(product_id);
CREATE INDEX idx_document_lines_product_unit ON document_lines(product_unit_id, product_id);
CREATE INDEX idx_document_lines_source ON document_lines(source_line_id);
CREATE INDEX idx_document_lines_category ON document_lines(category_id_snapshot);

CREATE TABLE receipt_lines (
    document_line_id INTEGER PRIMARY KEY REFERENCES document_lines(id),
    purchase_order_line_id INTEGER REFERENCES document_lines(id),
    damaged_quantity_micros INTEGER NOT NULL DEFAULT 0 CHECK (damaged_quantity_micros BETWEEN 0 AND 1000000000000000),
    short_quantity_micros INTEGER NOT NULL DEFAULT 0 CHECK (short_quantity_micros BETWEEN 0 AND 1000000000000000),
    inspection_notes TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX idx_receipt_lines_order ON receipt_lines(purchase_order_line_id);
CREATE TABLE purchase_matches (
    id INTEGER PRIMARY KEY,
    invoice_line_id INTEGER NOT NULL REFERENCES document_lines(id),
    receipt_line_id INTEGER NOT NULL REFERENCES receipt_lines(document_line_id),
    base_quantity_micros INTEGER NOT NULL CHECK (base_quantity_micros BETWEEN 1 AND 1000000000000000),
    accrued_value_minor INTEGER NOT NULL CHECK (accrued_value_minor BETWEEN 0 AND 1000000000000),
    UNIQUE (invoice_line_id, receipt_line_id)
) STRICT;
CREATE INDEX idx_purchase_matches_receipt ON purchase_matches(receipt_line_id);

CREATE TABLE landed_charges (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    supplier_id INTEGER REFERENCES parties(id),
    description TEXT NOT NULL,
    amount_minor INTEGER NOT NULL CHECK (amount_minor BETWEEN 1 AND 1000000000000),
    allocation_basis TEXT NOT NULL CHECK (allocation_basis IN ('value', 'quantity', 'manual')),
    UNIQUE (id, document_id)
) STRICT;
CREATE INDEX idx_landed_charges_document ON landed_charges(document_id);
CREATE INDEX idx_landed_charges_supplier ON landed_charges(supplier_id);
CREATE TABLE landed_allocations (
    charge_id INTEGER NOT NULL REFERENCES landed_charges(id),
    receipt_line_id INTEGER NOT NULL REFERENCES receipt_lines(document_line_id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor BETWEEN 1 AND 1000000000000),
    inventory_share_minor INTEGER NOT NULL CHECK (inventory_share_minor BETWEEN 0 AND amount_minor),
    cogs_share_minor INTEGER NOT NULL CHECK (cogs_share_minor BETWEEN 0 AND amount_minor),
    PRIMARY KEY (charge_id, receipt_line_id),
    CHECK (inventory_share_minor + cogs_share_minor = amount_minor)
) STRICT;
CREATE INDEX idx_landed_allocations_receipt ON landed_allocations(receipt_line_id);

CREATE TABLE stocktake_lines (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    expected_quantity_micros INTEGER NOT NULL CHECK (expected_quantity_micros BETWEEN -1000000000000000 AND 1000000000000000),
    counted_quantity_micros INTEGER CHECK (counted_quantity_micros BETWEEN 0 AND 1000000000000000),
    snapshot_movement_id INTEGER,
    counted_by INTEGER REFERENCES users(id),
    counted_at TEXT,
    reason TEXT NOT NULL DEFAULT '',
    UNIQUE (document_id, product_id),
    CHECK ((counted_quantity_micros IS NULL) = (counted_at IS NULL)),
    CHECK ((counted_quantity_micros IS NULL) = (counted_by IS NULL)),
    FOREIGN KEY (snapshot_movement_id) REFERENCES stock_movements(id)
) STRICT;
CREATE INDEX idx_stocktake_lines_product ON stocktake_lines(product_id);
CREATE INDEX idx_stocktake_lines_user ON stocktake_lines(counted_by);
CREATE INDEX idx_stocktake_lines_snapshot ON stocktake_lines(snapshot_movement_id);

CREATE TABLE cash_shifts (
    id INTEGER PRIMARY KEY,
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id),
    opened_by INTEGER NOT NULL REFERENCES users(id),
    opened_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    opening_balance_minor INTEGER NOT NULL CHECK (opening_balance_minor BETWEEN -1000000000000000 AND 1000000000000000),
    closed_by INTEGER REFERENCES users(id),
    closed_at TEXT,
    expected_balance_minor INTEGER,
    counted_balance_minor INTEGER CHECK (counted_balance_minor BETWEEN 0 AND 1000000000000000),
    close_document_id INTEGER UNIQUE REFERENCES documents(id),
    notes TEXT NOT NULL DEFAULT '',
    CHECK ((closed_at IS NULL AND closed_by IS NULL AND expected_balance_minor IS NULL AND counted_balance_minor IS NULL AND close_document_id IS NULL)
        OR (closed_at IS NOT NULL AND closed_by IS NOT NULL AND expected_balance_minor IS NOT NULL AND counted_balance_minor IS NOT NULL AND close_document_id IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX idx_cash_shifts_open ON cash_shifts(cash_account_id) WHERE closed_at IS NULL;
CREATE INDEX idx_cash_shifts_account_date ON cash_shifts(cash_account_id, opened_at);
CREATE INDEX idx_cash_shifts_opened_by ON cash_shifts(opened_by);
CREATE INDEX idx_cash_shifts_closed_by ON cash_shifts(closed_by);

CREATE TABLE payment_legs (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
    method TEXT NOT NULL CHECK (method IN ('cash', 'bank', 'cheque')),
    cash_account_id INTEGER REFERENCES cash_accounts(id),
    shift_id INTEGER REFERENCES cash_shifts(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor BETWEEN 1 AND 1000000000000000),
    reference TEXT NOT NULL DEFAULT '',
    CHECK ((method = 'cheque' AND cash_account_id IS NULL AND shift_id IS NULL)
        OR (method IN ('cash', 'bank') AND cash_account_id IS NOT NULL))
) STRICT;
CREATE INDEX idx_payment_legs_document ON payment_legs(document_id);
CREATE INDEX idx_payment_legs_cash ON payment_legs(cash_account_id);
CREATE INDEX idx_payment_legs_shift ON payment_legs(shift_id);
CREATE TABLE cheques (
    id INTEGER PRIMARY KEY,
    payment_leg_id INTEGER NOT NULL UNIQUE REFERENCES payment_legs(id),
    party_id INTEGER NOT NULL REFERENCES parties(id),
    cheque_number TEXT NOT NULL CHECK (length(trim(cheque_number)) > 0),
    bank_name TEXT NOT NULL,
    due_date TEXT NOT NULL CHECK (length(due_date) = 10 AND date(due_date, '+0 days') IS due_date),
    notes TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX idx_cheques_party ON cheques(party_id);
CREATE INDEX idx_cheques_due ON cheques(due_date);
CREATE INDEX idx_cheques_number ON cheques(cheque_number, bank_name);
CREATE TABLE cheque_events (
    id INTEGER PRIMARY KEY,
    cheque_id INTEGER NOT NULL REFERENCES cheques(id),
    document_id INTEGER NOT NULL REFERENCES documents(id),
    status TEXT NOT NULL CHECK (status IN ('pending', 'cleared', 'bounced')),
    bank_account_id INTEGER REFERENCES cash_accounts(id),
    reason TEXT NOT NULL DEFAULT '',
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (status <> 'cleared' OR bank_account_id IS NOT NULL),
    UNIQUE (cheque_id, document_id)
) STRICT;
CREATE INDEX idx_cheque_events_document ON cheque_events(document_id);
CREATE INDEX idx_cheque_events_bank ON cheque_events(bank_account_id);
CREATE INDEX idx_cheque_events_user ON cheque_events(created_by);

CREATE TABLE expense_lines (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    category_id INTEGER NOT NULL REFERENCES expense_categories(id),
    description TEXT NOT NULL,
    net_minor INTEGER NOT NULL CHECK (net_minor BETWEEN 1 AND 1000000000000),
    tax_rate_bps INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bps BETWEEN 0 AND 10000),
    tax_minor INTEGER NOT NULL DEFAULT 0 CHECK (tax_minor BETWEEN 0 AND 1000000000000)
) STRICT;
CREATE INDEX idx_expense_lines_document ON expense_lines(document_id);
CREATE INDEX idx_expense_lines_category ON expense_lines(category_id);

CREATE TABLE document_events (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    from_status TEXT,
    to_status TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    user_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX idx_document_events_document ON document_events(document_id, id);
CREATE INDEX idx_document_events_user ON document_events(user_id);
