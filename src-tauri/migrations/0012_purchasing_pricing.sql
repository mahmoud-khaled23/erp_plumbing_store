INSERT INTO permissions(code,name_ar) VALUES
('purchases.plan','تخطيط إعادة الطلب'),('products.price_update','تحديث الأسعار الجماعي'),
('pricing.manage','إدارة الحد الأدنى للهامش');
INSERT INTO role_permissions SELECT 1,code FROM permissions WHERE code IN ('purchases.plan','products.price_update','pricing.manage');
INSERT INTO role_permissions SELECT 4,code FROM permissions WHERE code IN ('purchases.plan','products.price_update','pricing.manage');

CREATE TABLE product_planning (
    product_id INTEGER PRIMARY KEY REFERENCES products(id),
    supplier_id INTEGER REFERENCES parties(id),
    reorder_point_micros INTEGER NOT NULL DEFAULT 0 CHECK(reorder_point_micros BETWEEN 0 AND 1000000000000000),
    max_level_micros INTEGER NOT NULL DEFAULT 0 CHECK(max_level_micros BETWEEN reorder_point_micros AND 1000000000000000),
    lead_time_days INTEGER NOT NULL DEFAULT 0 CHECK(lead_time_days BETWEEN 0 AND 3650),
    weight_grams INTEGER NOT NULL DEFAULT 0 CHECK(weight_grams BETWEEN 0 AND 1000000),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0)
) STRICT;
CREATE INDEX idx_product_planning_supplier ON product_planning(supplier_id);
CREATE TABLE pricing_policy (
    id INTEGER PRIMARY KEY CHECK(id=1),
    minimum_margin_bps INTEGER NOT NULL DEFAULT 0 CHECK(minimum_margin_bps BETWEEN 0 AND 10000),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0)
) STRICT;
INSERT INTO pricing_policy(id) VALUES(1);

CREATE TABLE supplier_price_history (
    purchase_line_id INTEGER PRIMARY KEY REFERENCES document_lines(id),
    document_id INTEGER NOT NULL REFERENCES documents(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    product_unit_id INTEGER NOT NULL REFERENCES product_units(id),
    supplier_id INTEGER NOT NULL REFERENCES parties(id),
    document_date TEXT NOT NULL,
    base_quantity_micros INTEGER NOT NULL CHECK(base_quantity_micros>0),
    quantity_micros INTEGER NOT NULL CHECK(quantity_micros>0),
    net_minor INTEGER NOT NULL CHECK(net_minor BETWEEN 0 AND 1000000000000),
    unit_name_snapshot TEXT NOT NULL
) STRICT;
CREATE INDEX idx_supplier_prices_product ON supplier_price_history(product_id,document_date DESC,purchase_line_id DESC);
CREATE INDEX idx_supplier_prices_supplier ON supplier_price_history(supplier_id,product_id,document_date DESC);
CREATE INDEX idx_supplier_prices_document ON supplier_price_history(document_id);
CREATE INDEX idx_supplier_prices_unit ON supplier_price_history(product_unit_id);
CREATE INDEX idx_supplier_prices_date ON supplier_price_history(document_date);
-- Preserve raw net value and quantity: no SQL multiplication or rounded historical cost.
INSERT INTO supplier_price_history
SELECT l.id,d.id,l.product_id,l.product_unit_id,d.party_id,d.document_date,l.base_quantity_micros,l.quantity_micros,
       l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor,l.unit_name_snapshot
FROM document_lines l JOIN documents d ON d.id=l.document_id WHERE d.kind='purchase' AND d.status='posted';
CREATE TRIGGER supplier_prices_post AFTER UPDATE OF status ON documents
WHEN NEW.kind='purchase' AND NEW.status='posted' AND OLD.status='posting' BEGIN
    INSERT INTO supplier_price_history
    SELECT l.id,NEW.id,l.product_id,l.product_unit_id,NEW.party_id,NEW.document_date,l.base_quantity_micros,l.quantity_micros,
           l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor,l.unit_name_snapshot
    FROM document_lines l WHERE l.document_id=NEW.id;
END;
CREATE TRIGGER supplier_prices_insert BEFORE INSERT ON supplier_price_history BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents d JOIN document_lines l ON l.document_id=d.id WHERE d.id=NEW.document_id AND d.kind='purchase' AND d.status='posted' AND l.id=NEW.purchase_line_id AND l.product_id=NEW.product_id AND l.product_unit_id=NEW.product_unit_id AND d.party_id=NEW.supplier_id AND d.document_date=NEW.document_date AND l.base_quantity_micros=NEW.base_quantity_micros AND l.quantity_micros=NEW.quantity_micros AND l.unit_name_snapshot=NEW.unit_name_snapshot AND l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor=NEW.net_minor) THEN RAISE(ABORT,'سجل سعر المورد لا يطابق فاتورة مرحلة') END;
END;
CREATE TRIGGER supplier_prices_no_update BEFORE UPDATE ON supplier_price_history BEGIN SELECT RAISE(ABORT,'سجل أسعار الموردين ثابت'); END;
CREATE TRIGGER supplier_prices_no_delete BEFORE DELETE ON supplier_price_history BEGIN SELECT RAISE(ABORT,'سجل أسعار الموردين ثابت'); END;

CREATE TABLE price_update_batches (
    id INTEGER PRIMARY KEY,
    rule_json TEXT NOT NULL CHECK(json_valid(rule_json)),
    preview_hash TEXT NOT NULL CHECK(length(preview_hash)=64),
    reason TEXT NOT NULL CHECK(length(trim(reason))>0),
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX idx_price_update_batches_user ON price_update_batches(created_by);
CREATE INDEX idx_price_update_batches_date ON price_update_batches(created_at);
CREATE TABLE price_update_lines (
    batch_id INTEGER NOT NULL REFERENCES price_update_batches(id),
    product_unit_id INTEGER NOT NULL REFERENCES product_units(id),
    old_retail_minor INTEGER NOT NULL CHECK(old_retail_minor BETWEEN 0 AND 1000000000000),
    new_retail_minor INTEGER NOT NULL CHECK(new_retail_minor BETWEEN 0 AND 1000000000000),
    old_wholesale_minor INTEGER NOT NULL CHECK(old_wholesale_minor BETWEEN 0 AND 1000000000000),
    new_wholesale_minor INTEGER NOT NULL CHECK(new_wholesale_minor BETWEEN 0 AND 1000000000000),
    PRIMARY KEY(batch_id,product_unit_id)
) STRICT;
CREATE INDEX idx_price_update_lines_unit ON price_update_lines(product_unit_id);
CREATE TRIGGER price_batches_no_update BEFORE UPDATE ON price_update_batches BEGIN SELECT RAISE(ABORT,'دفعة تحديث الأسعار ثابتة'); END;
CREATE TRIGGER price_batches_no_delete BEFORE DELETE ON price_update_batches BEGIN SELECT RAISE(ABORT,'دفعة تحديث الأسعار ثابتة'); END;
CREATE TRIGGER price_lines_no_update BEFORE UPDATE ON price_update_lines BEGIN SELECT RAISE(ABORT,'سجل تحديث السعر ثابت'); END;
CREATE TRIGGER price_lines_no_delete BEFORE DELETE ON price_update_lines BEGIN SELECT RAISE(ABORT,'سجل تحديث السعر ثابت'); END;

CREATE TABLE sale_margin_checks (
    document_line_id INTEGER PRIMARY KEY REFERENCES document_lines(id),
    document_id INTEGER NOT NULL REFERENCES documents(id),
    net_minor INTEGER NOT NULL CHECK(net_minor BETWEEN 0 AND 1000000000000),
    cost_minor INTEGER NOT NULL CHECK(cost_minor BETWEEN 0 AND 2000000000000000),
    minimum_margin_bps INTEGER NOT NULL CHECK(minimum_margin_bps BETWEEN 0 AND 10000),
    override_user_id INTEGER REFERENCES users(id),
    reason TEXT NOT NULL,
    CHECK(override_user_id IS NULL OR length(trim(reason))>0)
) STRICT;
CREATE INDEX idx_sale_margin_checks_document ON sale_margin_checks(document_id);
CREATE INDEX idx_sale_margin_checks_user ON sale_margin_checks(override_user_id);
CREATE TRIGGER sale_margin_checks_insert BEFORE INSERT ON sale_margin_checks BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents d JOIN document_lines l ON l.document_id=d.id WHERE d.id=NEW.document_id AND d.kind='sale' AND d.status='posting' AND l.id=NEW.document_line_id AND l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor=NEW.net_minor) THEN RAISE(ABORT,'بيانات فحص الهامش غير صالحة') END;
END;
CREATE TRIGGER sale_margin_checks_no_update BEFORE UPDATE ON sale_margin_checks BEGIN SELECT RAISE(ABORT,'فحص هامش البيع ثابت'); END;
CREATE TRIGGER sale_margin_checks_no_delete BEFORE DELETE ON sale_margin_checks BEGIN SELECT RAISE(ABORT,'فحص هامش البيع ثابت'); END;

-- The old charge field remains the legacy explicit-share input. Actual new
-- calculation method and weights are frozen here, including weight allocation.
CREATE TABLE landed_calculations (
    document_id INTEGER PRIMARY KEY REFERENCES documents(id),
    basis TEXT NOT NULL CHECK(basis IN ('value','quantity','weight')),
    amount_minor INTEGER NOT NULL CHECK(amount_minor BETWEEN 1 AND 1000000000000000),
    inputs_json TEXT NOT NULL CHECK(json_valid(inputs_json)),
    preview_hash TEXT NOT NULL CHECK(length(preview_hash)=64)
) STRICT;
CREATE TRIGGER landed_calculations_insert BEFORE INSERT ON landed_calculations BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents WHERE id=NEW.document_id AND kind='landed_cost' AND status='posted' AND total_minor=NEW.amount_minor) THEN RAISE(ABORT,'بيانات توزيع التكلفة لا تطابق المستند') END;
END;
CREATE TRIGGER landed_calculations_no_update BEFORE UPDATE ON landed_calculations BEGIN SELECT RAISE(ABORT,'طريقة توزيع التكلفة ثابتة'); END;
CREATE TRIGGER landed_calculations_no_delete BEFORE DELETE ON landed_calculations BEGIN SELECT RAISE(ABORT,'طريقة توزيع التكلفة ثابتة'); END;
