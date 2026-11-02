-- Derived cache only: the immutable movement ledger remains authoritative.
-- Keep zero pools: their last movement is needed for backdating and stocktakes.
CREATE TABLE stock_balances (
    product_id INTEGER NOT NULL REFERENCES products(id),
    warehouse_id INTEGER NOT NULL REFERENCES warehouses(id),
    quantity_micros INTEGER NOT NULL,
    value_minor INTEGER NOT NULL,
    last_movement_id INTEGER NOT NULL REFERENCES stock_movements(id),
    PRIMARY KEY (product_id, warehouse_id)
) STRICT;
CREATE INDEX idx_stock_balances_warehouse ON stock_balances(warehouse_id, product_id);
CREATE INDEX idx_stock_balances_movement ON stock_balances(last_movement_id);

INSERT INTO stock_balances(product_id, warehouse_id, quantity_micros, value_minor, last_movement_id)
SELECT product_id, warehouse_id, SUM(quantity_micros), SUM(value_minor), MAX(id)
FROM stock_movements GROUP BY product_id, warehouse_id;

-- Every insertion path, including INSERT ... SELECT reversals and preview savepoints,
-- goes through this trigger. RAISE(ABORT) rolls back the entire inserting statement.
CREATE TRIGGER stock_balances_append AFTER INSERT ON stock_movements BEGIN
    SELECT RAISE(ABORT, 'تسلسل حركات المخزون غير صالح') WHERE NEW.id <= COALESCE((
        SELECT last_movement_id FROM stock_balances
        WHERE product_id = NEW.product_id AND warehouse_id = NEW.warehouse_id), 0);
    -- Check using subtraction BEFORE adding; SQLite must never promote overflow to REAL.
    SELECT RAISE(ABORT, 'رصيد المخزون يتجاوز الحدود الرقمية المسموحة') WHERE EXISTS (
        SELECT 1 FROM stock_balances WHERE product_id = NEW.product_id AND warehouse_id = NEW.warehouse_id AND (
        (NEW.quantity_micros > 0 AND quantity_micros > 9223372036854775807 - NEW.quantity_micros)
        OR (NEW.quantity_micros < 0 AND quantity_micros < -9223372036854775808 - NEW.quantity_micros)
        OR (NEW.value_minor > 0 AND value_minor > 9223372036854775807 - NEW.value_minor)
        OR (NEW.value_minor < 0 AND value_minor < -9223372036854775808 - NEW.value_minor)));
    INSERT INTO stock_balances(product_id, warehouse_id, quantity_micros, value_minor, last_movement_id)
    VALUES (NEW.product_id, NEW.warehouse_id, NEW.quantity_micros, NEW.value_minor, NEW.id)
    ON CONFLICT(product_id, warehouse_id) DO UPDATE SET
        quantity_micros = stock_balances.quantity_micros + excluded.quantity_micros,
        value_minor = stock_balances.value_minor + excluded.value_minor,
        last_movement_id = excluded.last_movement_id;
END;

-- Retain all existing checks, replacing only history-wide availability/date reads.
-- Posting dates are nondecreasing per pool; append order is enforced above.
DROP TRIGGER stock_movements_insert_guard;
CREATE TRIGGER stock_movements_insert_guard BEFORE INSERT ON stock_movements BEGIN
    SELECT RAISE(ABORT, 'حركة المخزون تنشأ داخل عملية الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
    SELECT RAISE(ABORT, 'الصنف أو المخزن غير نشط') WHERE NOT EXISTS (SELECT 1 FROM products WHERE id = NEW.product_id AND active = 1)
        OR NOT EXISTS (SELECT 1 FROM warehouses WHERE id = NEW.warehouse_id AND active = 1);
    SELECT RAISE(ABORT, 'نوع حركة المخزون لا يطابق المستند') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND (
        (d.kind = 'sale' AND NEW.movement_type = 'sale')
        OR (d.kind = 'sale_return' AND NEW.movement_type = 'sale_return')
        OR (d.kind = 'purchase_return' AND NEW.movement_type = 'purchase_return')
        OR (d.kind = 'goods_receipt' AND NEW.movement_type = 'receipt')
        OR (d.kind = 'transfer' AND NEW.movement_type IN ('transfer_in', 'transfer_out'))
        OR (d.kind IN ('adjustment', 'stocktake') AND NEW.movement_type = 'adjustment')
        OR (d.kind = 'opening_stock' AND NEW.movement_type = 'opening')
        OR (d.kind IN ('landed_cost', 'purchase', 'cost_adjustment') AND NEW.movement_type = 'revaluation')
        OR (d.kind = 'reversal' AND NEW.movement_type = 'reversal')));
    SELECT RAISE(ABORT, 'سطر الصنف مطلوب للحركة') WHERE NEW.document_line_id IS NULL
        AND NEW.movement_type IN ('sale', 'sale_return', 'purchase_return', 'receipt', 'transfer_in', 'transfer_out', 'opening');
    SELECT RAISE(ABORT, 'مخزن الحركة لا يطابق المستند') WHERE NEW.movement_type NOT IN ('revaluation', 'reversal') AND NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND NEW.warehouse_id =
        CASE WHEN NEW.movement_type = 'transfer_in' THEN d.destination_warehouse_id ELSE d.warehouse_id END);
    SELECT RAISE(ABORT, 'الرصيد غير كاف؛ المخزون السالب غير مسموح') WHERE NEW.quantity_micros < 0
        AND (SELECT allow_negative_stock FROM store_settings WHERE id = 1) = 0
        AND COALESCE((SELECT quantity_micros FROM stock_balances
            WHERE product_id = NEW.product_id AND warehouse_id = NEW.warehouse_id), 0) < -NEW.quantity_micros;
    SELECT RAISE(ABORT, 'لا يمكن ترحيل حركة مخزون بتاريخ أقدم من آخر حركة للصنف') WHERE EXISTS (
        SELECT 1 FROM stock_balances b JOIN stock_movements s ON s.id = b.last_movement_id
        JOIN documents d ON d.id = s.document_id
        WHERE b.product_id = NEW.product_id AND b.warehouse_id = NEW.warehouse_id
        AND d.document_date > (SELECT document_date FROM documents WHERE id = NEW.document_id));
    SELECT RAISE(ABORT, 'عكس حركة المخزون لا يطابق الأصل') WHERE NEW.movement_type = 'reversal' AND NOT EXISTS (
        SELECT 1 FROM stock_movements s JOIN documents d ON d.id = NEW.document_id
        WHERE s.id = NEW.reverses_movement_id AND s.document_id = d.reversal_of_id
        AND s.product_id = NEW.product_id AND s.warehouse_id = NEW.warehouse_id
        AND s.quantity_micros = -NEW.quantity_micros AND s.value_minor = -NEW.value_minor
        AND s.unit_cost_micropiastres = NEW.unit_cost_micropiastres);
END;

INSERT INTO permissions(code, name_ar) VALUES ('inventory.verify', 'فحص مطابقة أرصدة المخزون');
INSERT INTO role_permissions(role_id, permission_code)
SELECT id, 'inventory.verify' FROM roles WHERE code IN ('admin', 'storekeeper', 'accountant');
