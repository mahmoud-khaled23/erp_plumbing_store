INSERT INTO permissions(code,name_ar) VALUES
('products.variants','إنشاء مقاسات وخامات من قالب'),
('products.bundles','إدارة أطقم البيع'),('products.warranty','إدارة ضمان الأصناف'),
('inventory.quarantine','عزل البضاعة والإفراج عنها'),
('inventory.quarantine_manage','تحديد مخازن العزل');
INSERT INTO role_permissions SELECT 1,code FROM permissions WHERE code IN ('products.variants','products.bundles','products.warranty','inventory.quarantine','inventory.quarantine_manage');
INSERT INTO role_permissions SELECT 3,code FROM permissions WHERE code IN ('products.variants','products.bundles','products.warranty','inventory.quarantine');

ALTER TABLE products ADD COLUMN warranty_days INTEGER CHECK(warranty_days BETWEEN 1 AND 36500);
ALTER TABLE document_lines ADD COLUMN warranty_days INTEGER CHECK(warranty_days BETWEEN 1 AND 36500);
ALTER TABLE warehouses ADD COLUMN is_quarantine INTEGER NOT NULL DEFAULT 0 CHECK(is_quarantine IN (0,1));
ALTER TABLE warehouses ADD COLUMN quarantine_revision INTEGER NOT NULL DEFAULT 0 CHECK(quarantine_revision>=0);
ALTER TABLE warehouse_product_settings ADD COLUMN shelf_location TEXT NOT NULL DEFAULT '' CHECK(length(shelf_location)<=120);

-- No backfill: historical documents never acquire today's warranty terms.
CREATE TRIGGER document_line_warranty_snapshot AFTER INSERT ON document_lines
WHEN (SELECT kind FROM documents WHERE id=NEW.document_id) IN ('sale','quotation','sale_return') BEGIN
    UPDATE document_lines SET warranty_days=CASE
      WHEN NEW.source_line_id IS NOT NULL THEN (SELECT warranty_days FROM document_lines WHERE id=NEW.source_line_id)
      ELSE (SELECT warranty_days FROM products WHERE id=NEW.product_id) END
    WHERE id=NEW.id;
END;

CREATE TABLE product_variants (
    product_id INTEGER PRIMARY KEY REFERENCES products(id),
    template_product_id INTEGER NOT NULL REFERENCES products(id),
    template_revision INTEGER NOT NULL CHECK(template_revision>=0),
    size_snapshot TEXT NOT NULL CHECK(length(trim(size_snapshot)) BETWEEN 1 AND 120),
    material_snapshot TEXT NOT NULL CHECK(length(trim(material_snapshot)) BETWEEN 1 AND 120),
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(template_product_id,size_snapshot,material_snapshot),
    CHECK(product_id<>template_product_id)
) STRICT;
CREATE INDEX idx_product_variants_user ON product_variants(created_by);
CREATE INDEX idx_product_variants_date ON product_variants(created_at);
CREATE TRIGGER variants_no_update BEFORE UPDATE ON product_variants BEGIN SELECT RAISE(ABORT,'مرجع إنشاء الصنف ثابت'); END;
CREATE TRIGGER variants_no_delete BEFORE DELETE ON product_variants BEGIN SELECT RAISE(ABORT,'مرجع إنشاء الصنف ثابت'); END;
CREATE TRIGGER variant_identity_guard BEFORE UPDATE OF size,material ON products
WHEN (NEW.size<>OLD.size OR NEW.material<>OLD.material) AND EXISTS(SELECT 1 FROM product_variants WHERE product_id=OLD.id) BEGIN
    SELECT RAISE(ABORT,'مقاس وخامة الصنف المشتق ثابتان؛ أنشئ صنفاً جديداً');
END;

CREATE TABLE sales_bundles (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(trim(code)) BETWEEN 1 AND 64),
    name_ar TEXT NOT NULL CHECK(length(trim(name_ar)) BETWEEN 1 AND 200),
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN(0,1)),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0)
) STRICT;
CREATE INDEX idx_sales_bundles_name ON sales_bundles(name_ar);
CREATE TABLE sales_bundle_items (
    bundle_id INTEGER NOT NULL REFERENCES sales_bundles(id),
    product_unit_id INTEGER NOT NULL REFERENCES product_units(id),
    quantity_micros INTEGER NOT NULL CHECK(quantity_micros BETWEEN 1 AND 1000000000000000),
    PRIMARY KEY(bundle_id,product_unit_id)
) STRICT;
CREATE INDEX idx_sales_bundle_items_unit ON sales_bundle_items(product_unit_id);
-- Bundles reference only real product units; recursive bundles cannot be represented.

CREATE TABLE quarantine_transfers (
    document_id INTEGER PRIMARY KEY REFERENCES documents(id),
    from_quarantine INTEGER NOT NULL CHECK(from_quarantine IN(0,1)),
    to_quarantine INTEGER NOT NULL CHECK(to_quarantine IN(0,1)),
    CHECK(from_quarantine=1 OR to_quarantine=1)
) STRICT;
CREATE TRIGGER quarantine_transfer_insert BEFORE INSERT ON quarantine_transfers BEGIN
    SELECT RAISE(ABORT,'مرجع تحويل العزل غير صالح') WHERE NOT EXISTS(
      SELECT 1 FROM documents d JOIN warehouses f ON f.id=d.warehouse_id JOIN warehouses t ON t.id=d.destination_warehouse_id
      WHERE d.id=NEW.document_id AND d.status='draft' AND d.kind='transfer' AND length(trim(d.reason))>0
      AND f.is_quarantine=NEW.from_quarantine AND t.is_quarantine=NEW.to_quarantine);
END;
CREATE TRIGGER quarantine_transfer_no_update BEFORE UPDATE ON quarantine_transfers BEGIN SELECT RAISE(ABORT,'سجل تحويل العزل ثابت'); END;
CREATE TRIGGER quarantine_transfer_no_delete BEFORE DELETE ON quarantine_transfers BEGIN SELECT RAISE(ABORT,'سجل تحويل العزل ثابت'); END;
CREATE TRIGGER quarantine_declassify_guard BEFORE UPDATE OF is_quarantine ON warehouses
WHEN OLD.is_quarantine=1 AND NEW.is_quarantine=0 BEGIN
    SELECT RAISE(ABORT,'أفرج عن رصيد مخزن العزل بتحويل قبل تغيير تصنيفه') WHERE EXISTS(
      SELECT 1 FROM stock_balances WHERE warehouse_id=OLD.id AND (quantity_micros<>0 OR value_minor<>0));
END;
CREATE TRIGGER quarantine_sale_guard BEFORE UPDATE OF status ON documents
WHEN NEW.status='posting' AND NEW.kind='sale' BEGIN
    SELECT RAISE(ABORT,'لا يمكن البيع من مخزن العزل؛ أفرج عن البضاعة أولاً') WHERE EXISTS(
      SELECT 1 FROM warehouses WHERE id=NEW.warehouse_id AND is_quarantine=1);
END;
CREATE TRIGGER quarantine_movement_guard BEFORE INSERT ON stock_movements
WHEN EXISTS(SELECT 1 FROM warehouses WHERE id=NEW.warehouse_id AND is_quarantine=1) BEGIN
    SELECT RAISE(ABORT,'لا يمكن البيع من مخزن العزل') WHERE NEW.movement_type='sale';
    SELECT RAISE(ABORT,'إخراج بضاعة من العزل يتطلب الصلاحية وسبباً') WHERE NEW.quantity_micros<0 AND (
      length(trim(NEW.reason))=0 OR NOT EXISTS(SELECT 1 FROM user_roles u JOIN role_permissions p ON p.role_id=u.role_id WHERE u.user_id=NEW.created_by AND p.permission_code='inventory.quarantine'));
END;
