CREATE TABLE journal_entries (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL UNIQUE REFERENCES documents(id),
    memo TEXT NOT NULL DEFAULT '',
    UNIQUE (id, document_id)
) STRICT;
CREATE TABLE journal_lines (
    id INTEGER PRIMARY KEY,
    entry_id INTEGER NOT NULL,
    document_id INTEGER NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number BETWEEN 1 AND 10000),
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    party_id INTEGER REFERENCES parties(id),
    product_id INTEGER REFERENCES products(id),
    category_id INTEGER REFERENCES categories(id),
    document_line_id INTEGER,
    reverses_line_id INTEGER UNIQUE REFERENCES journal_lines(id),
    due_date TEXT CHECK (due_date IS NULL OR (length(due_date) = 10 AND date(due_date, '+0 days') IS due_date)),
    debit_minor INTEGER NOT NULL DEFAULT 0 CHECK (debit_minor BETWEEN 0 AND 2000000000000000),
    credit_minor INTEGER NOT NULL DEFAULT 0 CHECK (credit_minor BETWEEN 0 AND 2000000000000000),
    memo TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (entry_id, document_id) REFERENCES journal_entries(id, document_id),
    FOREIGN KEY (document_line_id, document_id) REFERENCES document_lines(id, document_id),
    UNIQUE (entry_id, line_number),
    CHECK ((debit_minor > 0 AND credit_minor = 0) OR (credit_minor > 0 AND debit_minor = 0))
) STRICT;
CREATE INDEX idx_journal_lines_document ON journal_lines(document_id);
CREATE INDEX idx_journal_lines_entry_document ON journal_lines(entry_id, document_id);
CREATE INDEX idx_journal_lines_account ON journal_lines(account_id, document_id);
CREATE INDEX idx_journal_lines_party ON journal_lines(party_id, account_id, due_date);
CREATE INDEX idx_journal_lines_product ON journal_lines(product_id);
CREATE INDEX idx_journal_lines_category ON journal_lines(category_id);
CREATE INDEX idx_journal_lines_doc_line ON journal_lines(document_line_id, document_id);
CREATE INDEX idx_journal_lines_due ON journal_lines(due_date) WHERE due_date IS NOT NULL;

-- Matches opposite AR/AP journal lines, including advances, credit notes and opening balances.
-- Negative rows reverse one complete earlier allocation; partial unmatching uses a new match.
CREATE TABLE settlements (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    debit_line_id INTEGER NOT NULL REFERENCES journal_lines(id),
    credit_line_id INTEGER NOT NULL REFERENCES journal_lines(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor BETWEEN -2000000000000000 AND 2000000000000000 AND amount_minor <> 0),
    reversal_of_id INTEGER UNIQUE REFERENCES settlements(id),
    CHECK (debit_line_id <> credit_line_id),
    CHECK ((amount_minor < 0) = (reversal_of_id IS NOT NULL)),
    CHECK (reversal_of_id IS NULL OR reversal_of_id <> id),
    UNIQUE (document_id, debit_line_id, credit_line_id)
) STRICT;
CREATE INDEX idx_settlements_document ON settlements(document_id);
CREATE INDEX idx_settlements_debit ON settlements(debit_line_id);
CREATE INDEX idx_settlements_credit ON settlements(credit_line_id);

CREATE TABLE stock_movements (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    document_line_id INTEGER,
    product_id INTEGER NOT NULL REFERENCES products(id),
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    movement_type TEXT NOT NULL CHECK (movement_type IN ('opening', 'receipt', 'sale', 'sale_return', 'purchase_return', 'transfer_in', 'transfer_out', 'adjustment', 'revaluation', 'reversal')),
    quantity_micros INTEGER NOT NULL CHECK (quantity_micros BETWEEN -1000000000000000 AND 1000000000000000),
    value_minor INTEGER NOT NULL CHECK (value_minor BETWEEN -2000000000000000 AND 2000000000000000),
    unit_cost_micropiastres INTEGER NOT NULL CHECK (unit_cost_micropiastres BETWEEN 0 AND 2000000000000000000),
    reverses_movement_id INTEGER UNIQUE REFERENCES stock_movements(id),
    reason TEXT NOT NULL DEFAULT '',
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (document_line_id, document_id, product_id) REFERENCES document_lines(id, document_id, product_id),
    CHECK (quantity_micros <> 0 OR (value_minor <> 0 AND movement_type IN ('revaluation', 'reversal'))),
    CHECK ((movement_type = 'reversal') = (reverses_movement_id IS NOT NULL)),
    CHECK (movement_type NOT IN ('receipt', 'sale_return', 'transfer_in', 'opening') OR quantity_micros > 0),
    CHECK (movement_type NOT IN ('sale', 'purchase_return', 'transfer_out') OR quantity_micros < 0),
    CHECK (movement_type <> 'revaluation' OR quantity_micros = 0),
    CHECK (quantity_micros <= 0 OR value_minor >= 0),
    CHECK (quantity_micros >= 0 OR value_minor <= 0)
) STRICT;
CREATE INDEX idx_stock_movements_product_warehouse ON stock_movements(product_id, warehouse_id, id);
CREATE INDEX idx_stock_movements_warehouse ON stock_movements(warehouse_id, product_id);
CREATE INDEX idx_stock_movements_document ON stock_movements(document_id);
CREATE INDEX idx_stock_movements_line ON stock_movements(document_line_id, document_id, product_id);
CREATE INDEX idx_stock_movements_user ON stock_movements(created_by);
CREATE INDEX idx_stock_movements_date ON stock_movements(created_at);

CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    document_id INTEGER REFERENCES documents(id),
    before_json TEXT CHECK (before_json IS NULL OR json_valid(before_json)),
    after_json TEXT CHECK (after_json IS NULL OR json_valid(after_json)),
    reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX idx_audit_log_user ON audit_log(user_id, created_at);
CREATE INDEX idx_audit_log_document ON audit_log(document_id);
CREATE INDEX idx_audit_log_entity ON audit_log(entity_type, entity_id);
CREATE INDEX idx_audit_log_date ON audit_log(created_at);
CREATE TABLE command_receipts (
    request_id TEXT PRIMARY KEY CHECK (length(request_id) BETWEEN 16 AND 100),
    command TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL CHECK (length(payload_sha256) = 64),
    user_id INTEGER NOT NULL REFERENCES users(id),
    document_id INTEGER REFERENCES documents(id),
    result_json TEXT NOT NULL CHECK (json_valid(result_json)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX idx_command_receipts_user ON command_receipts(user_id);
CREATE INDEX idx_command_receipts_document ON command_receipts(document_id);
CREATE TABLE backup_runs (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('automatic', 'manual', 'pre_restore', 'pre_migration')),
    path TEXT NOT NULL,
    sha256 TEXT CHECK (sha256 IS NULL OR length(sha256) = 64),
    size_bytes INTEGER CHECK (size_bytes >= 0),
    status TEXT NOT NULL CHECK (status IN ('started', 'verified', 'failed')),
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT,
    user_id INTEGER REFERENCES users(id),
    CHECK (status <> 'verified' OR (sha256 IS NOT NULL AND size_bytes IS NOT NULL AND completed_at IS NOT NULL))
) STRICT;
CREATE INDEX idx_backup_runs_date ON backup_runs(started_at);
CREATE INDEX idx_backup_runs_user ON backup_runs(user_id);
CREATE TABLE import_batches (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('products', 'parties', 'opening_stock', 'opening_balances')),
    source_name TEXT NOT NULL,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    row_count INTEGER NOT NULL CHECK (row_count >= 0),
    document_id INTEGER REFERENCES documents(id),
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (kind, sha256)
) STRICT;
CREATE INDEX idx_import_batches_document ON import_batches(document_id);
CREATE INDEX idx_import_batches_user ON import_batches(created_by);
