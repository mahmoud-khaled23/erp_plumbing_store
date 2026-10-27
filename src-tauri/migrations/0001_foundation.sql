-- Applied by the migration runner inside BEGIN IMMEDIATE; do not add COMMIT here.
-- All monetary values are integer minor units. All quantities use 1,000,000/base unit.
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY CHECK (version > 0),
    name TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE roles (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name_ar TEXT NOT NULL
) STRICT;
CREATE TABLE permissions (
    code TEXT PRIMARY KEY,
    name_ar TEXT NOT NULL
) STRICT;
CREATE TABLE role_permissions (
    role_id INTEGER NOT NULL REFERENCES roles(id),
    permission_code TEXT NOT NULL REFERENCES permissions(code),
    PRIMARY KEY (role_id, permission_code)
) STRICT;
CREATE INDEX idx_role_permissions_permission ON role_permissions(permission_code);
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(username)) BETWEEN 3 AND 80),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    password_hash TEXT NOT NULL CHECK (password_hash GLOB '$argon2id$*'),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    must_change_password INTEGER NOT NULL DEFAULT 0 CHECK (must_change_password IN (0, 1)),
    failed_login_count INTEGER NOT NULL DEFAULT 0 CHECK (failed_login_count >= 0),
    locked_until TEXT,
    password_changed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE user_roles (
    user_id INTEGER NOT NULL REFERENCES users(id),
    role_id INTEGER NOT NULL REFERENCES roles(id),
    PRIMARY KEY (user_id, role_id)
) STRICT;
CREATE INDEX idx_user_roles_role ON user_roles(role_id);

CREATE TABLE store_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    store_name TEXT NOT NULL DEFAULT 'متجر أدوات السباكة',
    tax_number TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL DEFAULT '',
    phones TEXT NOT NULL DEFAULT '',
    logo_path TEXT,
    currency_code TEXT NOT NULL DEFAULT 'EGP' CHECK (length(currency_code) = 3),
    currency_symbol TEXT NOT NULL DEFAULT 'ج.م',
    currency_minor_digits INTEGER NOT NULL DEFAULT 2 CHECK (currency_minor_digits BETWEEN 0 AND 3),
    digit_style TEXT NOT NULL DEFAULT 'western' CHECK (digit_style IN ('western', 'arabic_indic')),
    timezone TEXT NOT NULL DEFAULT 'Africa/Cairo',
    vat_enabled INTEGER NOT NULL DEFAULT 0 CHECK (vat_enabled IN (0, 1)),
    default_vat_rate_bps INTEGER NOT NULL DEFAULT 1400 CHECK (default_vat_rate_bps BETWEEN 0 AND 10000),
    allow_negative_stock INTEGER NOT NULL DEFAULT 0 CHECK (allow_negative_stock IN (0, 1)),
    credit_limit_policy TEXT NOT NULL DEFAULT 'warn' CHECK (credit_limit_policy IN ('warn', 'block')),
    default_print_layout TEXT NOT NULL DEFAULT 'a4' CHECK (default_print_layout IN ('a4', 'thermal80')),
    receipt_footer TEXT NOT NULL DEFAULT 'شكراً لزيارتكم',
    print_copies INTEGER NOT NULL DEFAULT 1 CHECK (print_copies BETWEEN 1 AND 10),
    backup_directory TEXT,
    backup_retention_days INTEGER NOT NULL DEFAULT 30 CHECK (backup_retention_days = 30),
    auto_backup INTEGER NOT NULL DEFAULT 1 CHECK (auto_backup IN (0, 1)),
    setup_completed_at TEXT,
    demo_seeded_at TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0)
) STRICT;

CREATE TABLE fiscal_periods (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    starts_on TEXT NOT NULL CHECK (length(starts_on) = 10 AND date(starts_on, '+0 days') IS starts_on),
    ends_on TEXT NOT NULL CHECK (length(ends_on) = 10 AND date(ends_on, '+0 days') IS ends_on),
    locked_at TEXT,
    locked_by INTEGER REFERENCES users(id),
    CHECK (starts_on <= ends_on),
    CHECK ((locked_at IS NULL) = (locked_by IS NULL))
) STRICT;
CREATE INDEX idx_fiscal_periods_dates ON fiscal_periods(starts_on, ends_on);
CREATE INDEX idx_fiscal_periods_user ON fiscal_periods(locked_by);

CREATE TABLE document_types (
    code TEXT PRIMARY KEY,
    name_ar TEXT NOT NULL,
    requires_journal INTEGER NOT NULL CHECK (requires_journal IN (0, 1)),
    requires_lines INTEGER NOT NULL CHECK (requires_lines IN (0, 1)),
    can_post INTEGER NOT NULL DEFAULT 1 CHECK (can_post IN (0, 1))
) STRICT;
CREATE TABLE number_sequences (
    document_type TEXT NOT NULL REFERENCES document_types(code),
    fiscal_year INTEGER NOT NULL CHECK (fiscal_year BETWEEN 2000 AND 9999),
    prefix TEXT NOT NULL,
    next_value INTEGER NOT NULL DEFAULT 1 CHECK (next_value BETWEEN 1 AND 999999999),
    padding INTEGER NOT NULL DEFAULT 6 CHECK (padding BETWEEN 4 AND 9),
    PRIMARY KEY (document_type, fiscal_year)
) STRICT;

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name_ar TEXT NOT NULL,
    parent_id INTEGER REFERENCES accounts(id),
    kind TEXT NOT NULL CHECK (kind IN ('asset', 'liability', 'equity', 'income', 'expense')),
    control_kind TEXT CHECK (control_kind IN ('receivable', 'payable', 'inventory', 'cash', 'bank', 'cheque_received', 'cheque_issued', 'grni', 'vat_input', 'vat_output')),
    is_postable INTEGER NOT NULL DEFAULT 1 CHECK (is_postable IN (0, 1)),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;
CREATE INDEX idx_accounts_parent ON accounts(parent_id);
CREATE INDEX idx_accounts_control ON accounts(control_kind);
CREATE TABLE posting_accounts (
    key TEXT PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id)
) STRICT;
CREATE INDEX idx_posting_accounts_account ON posting_accounts(account_id);
CREATE TABLE cash_accounts (
    id INTEGER PRIMARY KEY,
    name_ar TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('cash', 'bank')),
    account_id INTEGER NOT NULL UNIQUE REFERENCES accounts(id),
    bank_name TEXT NOT NULL DEFAULT '',
    account_number TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
) STRICT;
CREATE TABLE expense_categories (
    id INTEGER PRIMARY KEY,
    name_ar TEXT NOT NULL UNIQUE,
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
) STRICT;
CREATE INDEX idx_expense_categories_account ON expense_categories(account_id);

CREATE TABLE parties (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name_ar TEXT NOT NULL CHECK (length(trim(name_ar)) > 0),
    normalized_name TEXT NOT NULL,
    is_customer INTEGER NOT NULL DEFAULT 0 CHECK (is_customer IN (0, 1)),
    is_supplier INTEGER NOT NULL DEFAULT 0 CHECK (is_supplier IN (0, 1)),
    phone TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL DEFAULT '',
    tax_number TEXT NOT NULL DEFAULT '',
    credit_limit_minor INTEGER CHECK (credit_limit_minor BETWEEN 0 AND 1000000000000000),
    payment_terms_days INTEGER NOT NULL DEFAULT 0 CHECK (payment_terms_days BETWEEN 0 AND 3650),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (is_customer = 1 OR is_supplier = 1)
) STRICT;
CREATE INDEX idx_parties_name ON parties(normalized_name);
CREATE INDEX idx_parties_phone ON parties(phone);

CREATE TABLE warehouses (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name_ar TEXT NOT NULL UNIQUE,
    address TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
) STRICT;
CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name_ar TEXT NOT NULL UNIQUE,
    parent_id INTEGER REFERENCES categories(id),
    CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;
CREATE INDEX idx_categories_parent ON categories(parent_id);
CREATE TABLE units (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name_ar TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL COLLATE NOCASE UNIQUE,
    name_ar TEXT NOT NULL CHECK (length(trim(name_ar)) > 0),
    normalized_name TEXT NOT NULL,
    category_id INTEGER NOT NULL REFERENCES categories(id),
    brand TEXT NOT NULL DEFAULT '',
    material TEXT NOT NULL DEFAULT '',
    size TEXT NOT NULL DEFAULT '',
    base_unit_id INTEGER NOT NULL REFERENCES units(id),
    image_path TEXT,
    standard_cost_minor INTEGER NOT NULL DEFAULT 0 CHECK (standard_cost_minor BETWEEN 0 AND 1000000000000),
    min_stock_micros INTEGER NOT NULL DEFAULT 0 CHECK (min_stock_micros BETWEEN 0 AND 1000000000000000),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0)
) STRICT;
CREATE INDEX idx_products_name ON products(normalized_name);
CREATE INDEX idx_products_category ON products(category_id, active);
CREATE INDEX idx_products_base_unit ON products(base_unit_id);
CREATE TABLE product_units (
    id INTEGER PRIMARY KEY,
    product_id INTEGER NOT NULL REFERENCES products(id),
    unit_id INTEGER NOT NULL REFERENCES units(id),
    factor_numerator INTEGER NOT NULL CHECK (factor_numerator BETWEEN 1 AND 1000000000),
    factor_denominator INTEGER NOT NULL DEFAULT 1 CHECK (factor_denominator BETWEEN 1 AND 1000000000),
    barcode TEXT UNIQUE CHECK (barcode IS NULL OR length(trim(barcode)) BETWEEN 1 AND 128),
    retail_price_minor INTEGER NOT NULL CHECK (retail_price_minor BETWEEN 0 AND 1000000000000),
    wholesale_price_minor INTEGER NOT NULL CHECK (wholesale_price_minor BETWEEN 0 AND 1000000000000),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    UNIQUE (product_id, unit_id),
    UNIQUE (id, product_id)
) STRICT;
CREATE INDEX idx_product_units_unit ON product_units(unit_id);
CREATE TABLE warehouse_product_settings (
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    min_stock_micros INTEGER NOT NULL CHECK (min_stock_micros BETWEEN 0 AND 1000000000000000),
    bin_location TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (warehouse_id, product_id)
) STRICT;
CREATE INDEX idx_warehouse_product_product ON warehouse_product_settings(product_id);

-- Token/prefix search; Arabic normalization is performed in Rust before writes/search.
CREATE VIRTUAL TABLE product_search USING fts5(
    name_ar, normalized_name, code, brand, material, size,
    content = 'products', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3 4'
);
CREATE TRIGGER products_search_insert AFTER INSERT ON products BEGIN
    INSERT INTO product_search(rowid, name_ar, normalized_name, code, brand, material, size)
    VALUES (NEW.id, NEW.name_ar, NEW.normalized_name, NEW.code, NEW.brand, NEW.material, NEW.size);
END;
CREATE TRIGGER products_search_delete AFTER DELETE ON products BEGIN
    INSERT INTO product_search(product_search, rowid, name_ar, normalized_name, code, brand, material, size)
    VALUES ('delete', OLD.id, OLD.name_ar, OLD.normalized_name, OLD.code, OLD.brand, OLD.material, OLD.size);
END;
CREATE TRIGGER products_search_update AFTER UPDATE ON products BEGIN
    INSERT INTO product_search(product_search, rowid, name_ar, normalized_name, code, brand, material, size)
    VALUES ('delete', OLD.id, OLD.name_ar, OLD.normalized_name, OLD.code, OLD.brand, OLD.material, OLD.size);
    INSERT INTO product_search(rowid, name_ar, normalized_name, code, brand, material, size)
    VALUES (NEW.id, NEW.name_ar, NEW.normalized_name, NEW.code, NEW.brand, NEW.material, NEW.size);
END;
