-- Lifecycle gates. A posting state may exist only inside the service transaction.
CREATE TRIGGER documents_insert_guard BEFORE INSERT ON documents BEGIN
    SELECT RAISE(ABORT, 'يجب إنشاء المستند كمسودة') WHERE NEW.status <> 'draft';
    SELECT RAISE(ABORT, 'الفترة المالية مغلقة') WHERE EXISTS (
        SELECT 1 FROM fiscal_periods WHERE NEW.document_date BETWEEN starts_on AND ends_on AND locked_at IS NOT NULL);
END;
CREATE TRIGGER documents_update_guard BEFORE UPDATE ON documents BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل مستند مرحل أو مغلق؛ أنشئ مستند عكس')
        WHERE OLD.status IN ('posted', 'cancelled', 'closed');
    SELECT RAISE(ABORT, 'لا يمكن تغيير هوية المستند')
        WHERE NEW.id <> OLD.id OR NEW.kind <> OLD.kind OR NEW.created_by <> OLD.created_by OR NEW.created_at <> OLD.created_at;
    SELECT RAISE(ABORT, 'الفترة المالية مغلقة') WHERE EXISTS (
        SELECT 1 FROM fiscal_periods WHERE locked_at IS NOT NULL
        AND (OLD.document_date BETWEEN starts_on AND ends_on OR NEW.document_date BETWEEN starts_on AND ends_on));
    SELECT RAISE(ABORT, 'انتقال حالة المستند غير مسموح') WHERE NEW.status <> OLD.status AND NOT (
        (OLD.status = 'draft' AND NEW.status = 'held' AND NEW.kind = 'sale')
        OR (OLD.status = 'held' AND NEW.status = 'draft')
        OR (OLD.status = 'draft' AND NEW.status = 'approved' AND NEW.kind IN ('purchase_order', 'quotation'))
        OR (OLD.status = 'approved' AND NEW.status = 'closed' AND NEW.kind IN ('purchase_order', 'quotation'))
        OR (OLD.status IN ('draft', 'held', 'approved') AND NEW.status = 'cancelled' AND length(trim(NEW.reason)) > 0)
        OR (OLD.status = 'draft' AND NEW.status = 'posting' AND (SELECT can_post FROM document_types WHERE code = NEW.kind) = 1)
        OR (OLD.status = 'posting' AND NEW.status = 'posted'));
    SELECT RAISE(ABORT, 'يجب تحديد فترة مالية مفتوحة') WHERE NEW.status IN ('posting', 'posted') AND NOT EXISTS (
        SELECT 1 FROM fiscal_periods WHERE NEW.document_date BETWEEN starts_on AND ends_on AND locked_at IS NULL);
    SELECT RAISE(ABORT, 'رقم المستند مطلوب') WHERE NEW.status IN ('posting', 'posted', 'approved') AND (NEW.number IS NULL OR length(trim(NEW.number)) = 0);
    SELECT RAISE(ABORT, 'العميل أو المورد غير صالح') WHERE NEW.party_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM parties WHERE id = NEW.party_id AND active = 1
        AND (NEW.kind NOT IN ('sale', 'sale_return', 'quotation') OR is_customer = 1)
        AND (NEW.kind NOT IN ('purchase', 'purchase_return', 'purchase_order', 'goods_receipt') OR is_supplier = 1));
    SELECT RAISE(ABORT, 'يجب تحديد المورد') WHERE NEW.status IN ('posting', 'approved')
        AND NEW.kind IN ('purchase', 'purchase_return', 'purchase_order', 'goods_receipt') AND NEW.party_id IS NULL;
    SELECT RAISE(ABORT, 'عملة المستند لا تطابق عملة النظام') WHERE NEW.status = 'posting' AND NOT EXISTS (
        SELECT 1 FROM store_settings WHERE id = 1 AND currency_code = NEW.currency_code AND currency_minor_digits = NEW.currency_minor_digits);
    SELECT RAISE(ABORT, 'يجب تحديد المخزن') WHERE NEW.status = 'posting'
        AND NEW.kind IN ('sale', 'sale_return', 'purchase_return', 'goods_receipt', 'transfer', 'adjustment', 'stocktake', 'opening_stock')
        AND NEW.warehouse_id IS NULL;
    SELECT RAISE(ABORT, 'يجب تحديد مخزن الاستلام') WHERE NEW.status = 'posting' AND NEW.kind = 'transfer' AND NEW.destination_warehouse_id IS NULL;
    SELECT RAISE(ABORT, 'يجب ذكر سبب التسوية أو العكس') WHERE NEW.status = 'posting'
        AND NEW.kind IN ('adjustment', 'stocktake', 'reversal') AND length(trim(NEW.reason)) = 0;
    SELECT RAISE(ABORT, 'المستند الأصلي غير قابل للعكس') WHERE NEW.status = 'posting' AND NEW.kind = 'reversal' AND NOT EXISTS (
        SELECT 1 FROM documents WHERE id = NEW.reversal_of_id AND status = 'posted' AND kind <> 'reversal');
END;
CREATE TRIGGER documents_delete_guard BEFORE DELETE ON documents BEGIN
    SELECT RAISE(ABORT, 'يمكن حذف المسودات فقط') WHERE OLD.status NOT IN ('draft', 'held');
    SELECT RAISE(ABORT, 'الفترة المالية مغلقة') WHERE EXISTS (
        SELECT 1 FROM fiscal_periods WHERE OLD.document_date BETWEEN starts_on AND ends_on AND locked_at IS NOT NULL);
END;

CREATE TRIGGER periods_insert_guard BEFORE INSERT ON fiscal_periods BEGIN
    SELECT RAISE(ABORT, 'الفترات المالية متداخلة') WHERE EXISTS (
        SELECT 1 FROM fiscal_periods WHERE NEW.starts_on <= ends_on AND NEW.ends_on >= starts_on);
END;
CREATE TRIGGER periods_update_guard BEFORE UPDATE ON fiscal_periods BEGIN
    SELECT RAISE(ABORT, 'الفترات المالية متداخلة') WHERE EXISTS (
        SELECT 1 FROM fiscal_periods WHERE id <> OLD.id AND NEW.starts_on <= ends_on AND NEW.ends_on >= starts_on);
    SELECT RAISE(ABORT, 'لا يمكن تغيير حدود فترة مستخدمة') WHERE
        (NEW.starts_on <> OLD.starts_on OR NEW.ends_on <> OLD.ends_on) AND EXISTS (
        SELECT 1 FROM documents WHERE document_date BETWEEN OLD.starts_on AND OLD.ends_on);
    SELECT RAISE(ABORT, 'يوجد ترحيل غير مكتمل') WHERE NEW.locked_at IS NOT NULL AND EXISTS (
        SELECT 1 FROM documents WHERE status = 'posting' AND document_date BETWEEN NEW.starts_on AND NEW.ends_on);
END;
CREATE TRIGGER periods_delete_guard BEFORE DELETE ON fiscal_periods BEGIN
    SELECT RAISE(ABORT, 'لا يمكن حذف فترة مستخدمة أو مغلقة') WHERE OLD.locked_at IS NOT NULL OR EXISTS (
        SELECT 1 FROM documents WHERE document_date BETWEEN OLD.starts_on AND OLD.ends_on);
END;
CREATE TRIGGER settings_currency_guard BEFORE UPDATE ON store_settings
WHEN (NEW.currency_code <> OLD.currency_code OR NEW.currency_minor_digits <> OLD.currency_minor_digits)
AND EXISTS (SELECT 1 FROM documents WHERE status = 'posted') BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تغيير عملة دفاتر تحتوي على مستندات مرحلة');
END;
CREATE TRIGGER settings_delete_guard BEFORE DELETE ON store_settings BEGIN
    SELECT RAISE(ABORT, 'لا يمكن حذف إعدادات النظام');
END;
CREATE TRIGGER product_base_unit_guard BEFORE UPDATE OF base_unit_id ON products
WHEN NEW.base_unit_id <> OLD.base_unit_id AND (EXISTS (SELECT 1 FROM product_units WHERE product_id = OLD.id)
OR EXISTS (SELECT 1 FROM document_lines WHERE product_id = OLD.id)) BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تغيير الوحدة الأساسية لصنف مستخدم');
END;
CREATE TRIGGER product_unit_insert_guard BEFORE INSERT ON product_units
WHEN NEW.unit_id = (SELECT base_unit_id FROM products WHERE id = NEW.product_id)
AND NEW.factor_numerator <> NEW.factor_denominator BEGIN
    SELECT RAISE(ABORT, 'معامل الوحدة الأساسية يجب أن يساوي واحداً');
END;
CREATE TRIGGER product_unit_update_guard BEFORE UPDATE ON product_units BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تغيير تحويل وحدة مستخدمة') WHERE
        (NEW.product_id <> OLD.product_id OR NEW.unit_id <> OLD.unit_id OR NEW.factor_numerator <> OLD.factor_numerator OR NEW.factor_denominator <> OLD.factor_denominator)
        AND EXISTS (SELECT 1 FROM document_lines WHERE product_unit_id = OLD.id);
    SELECT RAISE(ABORT, 'معامل الوحدة الأساسية يجب أن يساوي واحداً') WHERE
        NEW.unit_id = (SELECT base_unit_id FROM products WHERE id = NEW.product_id) AND NEW.factor_numerator <> NEW.factor_denominator;
END;
CREATE TRIGGER accounts_used_guard BEFORE UPDATE ON accounts
WHEN EXISTS (SELECT 1 FROM journal_lines WHERE account_id = OLD.id)
AND (NEW.kind <> OLD.kind OR NEW.control_kind IS NOT OLD.control_kind OR NEW.is_postable <> OLD.is_postable) BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تغيير طبيعة حساب مستخدم');
END;

CREATE TRIGGER journal_entries_insert_guard BEFORE INSERT ON journal_entries BEGIN
    SELECT RAISE(ABORT, 'القيد ينشأ داخل عملية الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
END;
CREATE TRIGGER journal_lines_insert_guard BEFORE INSERT ON journal_lines BEGIN
    SELECT RAISE(ABORT, 'القيد ينشأ داخل عملية الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
    SELECT RAISE(ABORT, 'الحساب غير نشط أو تجميعي') WHERE NOT EXISTS (
        SELECT 1 FROM accounts WHERE id = NEW.account_id AND active = 1 AND is_postable = 1);
    SELECT RAISE(ABORT, 'حساب العملاء أو الموردين يحتاج إلى طرف وتاريخ استحقاق') WHERE
        (SELECT control_kind FROM accounts WHERE id = NEW.account_id) IN ('receivable', 'payable')
        AND (NEW.party_id IS NULL OR NEW.due_date IS NULL);
    SELECT RAISE(ABORT, 'عكس سطر القيد لا يطابق الأصل') WHERE NEW.reverses_line_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM journal_lines j JOIN documents d ON d.id = NEW.document_id
        WHERE j.id = NEW.reverses_line_id AND d.kind = 'reversal' AND d.reversal_of_id = j.document_id
        AND j.account_id = NEW.account_id AND j.party_id IS NEW.party_id
        AND j.product_id IS NEW.product_id AND j.category_id IS NEW.category_id
        AND j.debit_minor = NEW.credit_minor AND j.credit_minor = NEW.debit_minor);
    SELECT RAISE(ABORT, 'مرجع سطر القيد الأصلي مطلوب') WHERE NEW.reverses_line_id IS NULL
        AND (SELECT kind FROM documents WHERE id = NEW.document_id) = 'reversal';
END;

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
        AND NEW.quantity_micros + COALESCE((SELECT SUM(quantity_micros) FROM stock_movements
            WHERE product_id = NEW.product_id AND warehouse_id = NEW.warehouse_id), 0) < 0;
    SELECT RAISE(ABORT, 'لا يمكن ترحيل حركة مخزون بتاريخ أقدم من آخر حركة للصنف') WHERE EXISTS (
        SELECT 1 FROM stock_movements s JOIN documents d ON d.id = s.document_id
        WHERE s.product_id = NEW.product_id AND s.warehouse_id = NEW.warehouse_id
        AND d.document_date > (SELECT document_date FROM documents WHERE id = NEW.document_id));
    SELECT RAISE(ABORT, 'عكس حركة المخزون لا يطابق الأصل') WHERE NEW.movement_type = 'reversal' AND NOT EXISTS (
        SELECT 1 FROM stock_movements s JOIN documents d ON d.id = NEW.document_id
        WHERE s.id = NEW.reverses_movement_id AND s.document_id = d.reversal_of_id
        AND s.product_id = NEW.product_id AND s.warehouse_id = NEW.warehouse_id
        AND s.quantity_micros = -NEW.quantity_micros AND s.value_minor = -NEW.value_minor
        AND s.unit_cost_micropiastres = NEW.unit_cost_micropiastres);
END;

CREATE TRIGGER settlements_insert_guard BEFORE INSERT ON settlements BEGIN
    SELECT RAISE(ABORT, 'التسوية تنشأ داخل عملية الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
    SELECT RAISE(ABORT, 'طرفا التسوية غير متوافقين') WHERE NOT EXISTS (
        SELECT 1 FROM journal_lines a JOIN journal_lines b ON b.id = NEW.credit_line_id
        JOIN accounts c ON c.id = a.account_id
        JOIN documents da ON da.id = a.document_id JOIN documents db ON db.id = b.document_id
        WHERE a.id = NEW.debit_line_id AND a.debit_minor > 0 AND b.credit_minor > 0
        AND a.account_id = b.account_id AND a.party_id = b.party_id AND c.control_kind IN ('receivable', 'payable')
        AND (da.status = 'posted' OR (da.status = 'posting' AND da.id = NEW.document_id))
        AND (db.status = 'posted' OR (db.status = 'posting' AND db.id = NEW.document_id)));
    SELECT RAISE(ABORT, 'قيمة التسوية تتجاوز الرصيد المفتوح') WHERE NEW.amount_minor > 0 AND (
        NEW.amount_minor + COALESCE((SELECT SUM(amount_minor) FROM settlements WHERE debit_line_id = NEW.debit_line_id), 0)
            > (SELECT debit_minor FROM journal_lines WHERE id = NEW.debit_line_id)
        OR NEW.amount_minor + COALESCE((SELECT SUM(amount_minor) FROM settlements WHERE credit_line_id = NEW.credit_line_id), 0)
            > (SELECT credit_minor FROM journal_lines WHERE id = NEW.credit_line_id));
    SELECT RAISE(ABORT, 'عكس التسوية لا يطابق الأصل') WHERE NEW.amount_minor < 0 AND NOT EXISTS (
        SELECT 1 FROM settlements s JOIN documents d ON d.id = s.document_id
        WHERE s.id = NEW.reversal_of_id AND s.amount_minor > 0 AND d.status = 'posted'
        AND s.debit_line_id = NEW.debit_line_id AND s.credit_line_id = NEW.credit_line_id AND s.amount_minor = -NEW.amount_minor);
END;

CREATE TRIGGER documents_post_guard BEFORE UPDATE OF status ON documents
WHEN NEW.status = 'posted' AND OLD.status = 'posting' BEGIN
    SELECT RAISE(ABORT, 'المستند لا يحتوي على أصناف') WHERE
        (SELECT requires_lines FROM document_types WHERE code = NEW.kind) = 1
        AND NOT EXISTS (SELECT 1 FROM document_lines WHERE document_id = NEW.id);
    SELECT RAISE(ABORT, 'إجماليات المستند لا تطابق السطور') WHERE
        (SELECT requires_lines FROM document_types WHERE code = NEW.kind) = 1 AND (
        NEW.subtotal_minor <> (SELECT COALESCE(SUM(subtotal_minor), 0) FROM document_lines WHERE document_id = NEW.id)
        OR NEW.line_discount_minor <> (SELECT COALESCE(SUM(line_discount_minor), 0) FROM document_lines WHERE document_id = NEW.id)
        OR NEW.invoice_discount_minor <> (SELECT COALESCE(SUM(invoice_discount_share_minor), 0) FROM document_lines WHERE document_id = NEW.id)
        OR NEW.tax_minor <> (SELECT COALESCE(SUM(tax_minor), 0) FROM document_lines WHERE document_id = NEW.id));
    SELECT RAISE(ABORT, 'القيد المحاسبي مفقود') WHERE
        (SELECT requires_journal FROM document_types WHERE code = NEW.kind) = 1
        AND (NEW.total_minor > 0 OR EXISTS (SELECT 1 FROM stock_movements WHERE document_id = NEW.id AND value_minor <> 0)
            OR NEW.kind IN ('manual_journal', 'opening_balance'))
        AND NOT EXISTS (SELECT 1 FROM journal_entries WHERE document_id = NEW.id);
    SELECT RAISE(ABORT, 'القيد غير متوازن أو فارغ') WHERE EXISTS (
        SELECT 1 FROM journal_entries e LEFT JOIN journal_lines l ON l.entry_id = e.id WHERE e.document_id = NEW.id
        GROUP BY e.id HAVING COUNT(l.id) < 2 OR SUM(l.debit_minor) <> SUM(l.credit_minor));
    SELECT RAISE(ABORT, 'قيمة المخزون لا تطابق الحساب المحاسبي') WHERE
        COALESCE((SELECT SUM(value_minor) FROM stock_movements WHERE document_id = NEW.id), 0) <>
        COALESCE((SELECT SUM(j.debit_minor - j.credit_minor) FROM journal_lines j JOIN accounts a ON a.id = j.account_id
            WHERE j.document_id = NEW.id AND a.control_kind = 'inventory'), 0);
    SELECT RAISE(ABORT, 'كميات حركة المخزون لا تطابق الفاتورة') WHERE
        NEW.kind IN ('sale', 'sale_return', 'purchase_return', 'goods_receipt', 'opening_stock') AND EXISTS (
        SELECT 1 FROM document_lines l WHERE l.document_id = NEW.id AND l.base_quantity_micros <>
        COALESCE((SELECT SUM(s.quantity_micros) FROM stock_movements s WHERE s.document_line_id = l.id), 0)
        * CASE WHEN NEW.kind IN ('sale', 'purchase_return') THEN -1 ELSE 1 END);
    SELECT RAISE(ABORT, 'الكمية يجب أن تكون أكبر من صفر') WHERE NEW.kind IN ('sale', 'sale_return', 'purchase', 'purchase_return', 'transfer', 'opening_stock')
        AND EXISTS (SELECT 1 FROM document_lines WHERE document_id = NEW.id AND (quantity_micros = 0 OR base_quantity_micros = 0));
    SELECT RAISE(ABORT, 'طرفا التحويل غير متساويين') WHERE NEW.kind = 'transfer' AND (
        (SELECT COALESCE(SUM(value_minor), 0) FROM stock_movements WHERE document_id = NEW.id) <> 0 OR EXISTS (
        SELECT 1 FROM document_lines l WHERE l.document_id = NEW.id AND (
            l.base_quantity_micros <> COALESCE((SELECT SUM(quantity_micros) FROM stock_movements WHERE document_line_id = l.id AND movement_type = 'transfer_in'), 0)
            OR l.base_quantity_micros <> -COALESCE((SELECT SUM(quantity_micros) FROM stock_movements WHERE document_line_id = l.id AND movement_type = 'transfer_out'), 0)
            OR COALESCE((SELECT SUM(value_minor) FROM stock_movements WHERE document_line_id = l.id), 0) <> 0)));
    SELECT RAISE(ABORT, 'مرجع المرتجع غير صحيح أو الكمية تتجاوز الأصل') WHERE NEW.kind IN ('sale_return', 'purchase_return') AND EXISTS (
        SELECT 1 FROM document_lines l LEFT JOIN document_lines original ON original.id = l.source_line_id
        LEFT JOIN documents od ON od.id = original.document_id WHERE l.document_id = NEW.id AND (
        original.id IS NULL OR original.product_id <> l.product_id OR od.id IS NOT NEW.source_document_id
        OR od.status <> 'posted' OR od.party_id IS NOT NEW.party_id
        OR od.kind <> CASE NEW.kind WHEN 'sale_return' THEN 'sale' ELSE 'purchase' END
        OR EXISTS (SELECT 1 FROM documents r WHERE r.reversal_of_id = od.id AND r.status = 'posted')
        OR original.base_quantity_micros < (SELECT COALESCE(SUM(rl.base_quantity_micros), 0)
            FROM document_lines rl JOIN documents rd ON rd.id = rl.document_id
            WHERE rl.source_line_id = original.id AND rd.kind = NEW.kind AND rd.status IN ('posting', 'posted')
            AND NOT EXISTS (SELECT 1 FROM documents rev WHERE rev.reversal_of_id = rd.id AND rev.status = 'posted'))));
    SELECT RAISE(ABORT, 'بيانات فحص الاستلام مفقودة') WHERE NEW.kind = 'goods_receipt' AND EXISTS (
        SELECT 1 FROM document_lines l LEFT JOIN receipt_lines r ON r.document_line_id = l.id WHERE l.document_id = NEW.id AND r.document_line_id IS NULL);
    SELECT RAISE(ABORT, 'كمية الاستلام تتجاوز أمر الشراء أو المرجع غير صالح') WHERE NEW.kind = 'goods_receipt' AND EXISTS (
        SELECT 1 FROM document_lines l JOIN receipt_lines r ON r.document_line_id = l.id
        JOIN document_lines ol ON ol.id = r.purchase_order_line_id JOIN documents od ON od.id = ol.document_id
        WHERE l.document_id = NEW.id AND (od.kind <> 'purchase_order' OR od.status <> 'approved'
        OR od.party_id IS NOT NEW.party_id OR ol.product_id <> l.product_id
        OR ol.base_quantity_micros < (SELECT COALESCE(SUM(accepted.base_quantity_micros), 0)
            FROM receipt_lines rr JOIN document_lines accepted ON accepted.id = rr.document_line_id
            JOIN documents ad ON ad.id = accepted.document_id WHERE rr.purchase_order_line_id = ol.id
            AND ad.status IN ('posting', 'posted') AND NOT EXISTS (
                SELECT 1 FROM documents rev WHERE rev.reversal_of_id = ad.id AND rev.status = 'posted'))));
    SELECT RAISE(ABORT, 'توزيع التكلفة الإضافية غير مكتمل') WHERE EXISTS (
        SELECT 1 FROM landed_charges c WHERE c.document_id = NEW.id
        AND c.amount_minor <> (SELECT COALESCE(SUM(amount_minor), 0) FROM landed_allocations WHERE charge_id = c.id));
    SELECT RAISE(ABORT, 'فاتورة المورد لا تطابق الاستلام') WHERE NEW.kind = 'purchase' AND (
        EXISTS (SELECT 1 FROM document_lines l WHERE l.document_id = NEW.id AND l.base_quantity_micros <>
            (SELECT COALESCE(SUM(base_quantity_micros), 0) FROM purchase_matches WHERE invoice_line_id = l.id))
        OR EXISTS (SELECT 1 FROM purchase_matches m JOIN document_lines l ON l.id = m.invoice_line_id
            JOIN document_lines received ON received.id = m.receipt_line_id JOIN documents rd ON rd.id = received.document_id
            WHERE l.document_id = NEW.id AND (rd.kind <> 'goods_receipt' OR rd.status <> 'posted'
            OR rd.party_id IS NOT NEW.party_id OR received.product_id <> l.product_id
            OR EXISTS (SELECT 1 FROM documents rev WHERE rev.reversal_of_id = rd.id AND rev.status = 'posted')
            OR received.base_quantity_micros < (SELECT COALESCE(SUM(pm.base_quantity_micros), 0)
                FROM purchase_matches pm JOIN document_lines il ON il.id = pm.invoice_line_id JOIN documents inv ON inv.id = il.document_id
                WHERE pm.receipt_line_id = received.id AND inv.status IN ('posting', 'posted')
                AND NOT EXISTS (SELECT 1 FROM documents rev WHERE rev.reversal_of_id = inv.id AND rev.status = 'posted')))));
    SELECT RAISE(ABORT, 'إجمالي التكاليف الإضافية غير صحيح') WHERE NEW.kind = 'landed_cost'
        AND NEW.subtotal_minor <> (SELECT COALESCE(SUM(amount_minor), 0) FROM landed_charges WHERE document_id = NEW.id);
    SELECT RAISE(ABORT, 'مرجع توزيع التكلفة غير صحيح') WHERE EXISTS (
        SELECT 1 FROM landed_charges c JOIN landed_allocations a ON a.charge_id = c.id
        JOIN document_lines l ON l.id = a.receipt_line_id JOIN documents d ON d.id = l.document_id
        WHERE c.document_id = NEW.id AND (NEW.kind <> 'landed_cost' OR d.kind <> 'goods_receipt' OR d.status <> 'posted'
        OR EXISTS (SELECT 1 FROM documents rev WHERE rev.reversal_of_id = d.id AND rev.status = 'posted')));
    SELECT RAISE(ABORT, 'إجمالي المصروف غير صحيح') WHERE NEW.kind = 'expense' AND (
        NEW.subtotal_minor <> (SELECT COALESCE(SUM(net_minor), 0) FROM expense_lines WHERE document_id = NEW.id)
        OR NEW.tax_minor <> (SELECT COALESCE(SUM(tax_minor), 0) FROM expense_lines WHERE document_id = NEW.id)
        OR NOT EXISTS (SELECT 1 FROM expense_lines WHERE document_id = NEW.id));
    SELECT RAISE(ABORT, 'يجب استكمال ورقة الجرد') WHERE NEW.kind = 'stocktake' AND (
        NOT EXISTS (SELECT 1 FROM stocktake_lines WHERE document_id = NEW.id)
        OR EXISTS (SELECT 1 FROM stocktake_lines WHERE document_id = NEW.id AND counted_quantity_micros IS NULL));
    SELECT RAISE(ABORT, 'حركة تسوية الجرد لا تطابق الفرق') WHERE NEW.kind = 'stocktake' AND EXISTS (
        SELECT 1 FROM stocktake_lines l WHERE l.document_id = NEW.id AND l.counted_quantity_micros - l.expected_quantity_micros
        <> COALESCE((SELECT SUM(quantity_micros) FROM stock_movements WHERE document_id = NEW.id AND product_id = l.product_id), 0));
    SELECT RAISE(ABORT, 'تغير المخزون منذ بدء الجرد؛ أعد احتساب الورقة') WHERE NEW.kind = 'stocktake' AND EXISTS (
        SELECT 1 FROM stocktake_lines l WHERE l.document_id = NEW.id AND EXISTS (
        SELECT 1 FROM stock_movements s WHERE s.product_id = l.product_id AND s.warehouse_id = NEW.warehouse_id
        AND s.document_id <> NEW.id AND s.id > COALESCE(l.snapshot_movement_id, 0)));
    SELECT RAISE(ABORT, 'رصيد ورقة الجرد لا يطابق سجل المخزون') WHERE NEW.kind = 'stocktake' AND EXISTS (
        SELECT 1 FROM stocktake_lines l WHERE l.document_id = NEW.id AND l.expected_quantity_micros <>
        COALESCE((SELECT SUM(s.quantity_micros) FROM stock_movements s WHERE s.product_id = l.product_id
            AND s.warehouse_id = NEW.warehouse_id AND s.document_id <> NEW.id), 0));
    SELECT RAISE(ABORT, 'عكس المستند غير مكتمل') WHERE NEW.kind = 'reversal' AND (
        (SELECT COUNT(*) FROM stock_movements WHERE document_id = NEW.id) <>
        (SELECT COUNT(*) FROM stock_movements WHERE document_id = NEW.reversal_of_id)
        OR (SELECT COUNT(*) FROM journal_lines WHERE document_id = NEW.id) <>
        (SELECT COUNT(*) FROM journal_lines WHERE document_id = NEW.reversal_of_id));
    SELECT RAISE(ABORT, 'بيانات الشيك غير مكتملة') WHERE EXISTS (
        SELECT 1 FROM payment_legs p LEFT JOIN cheques c ON c.payment_leg_id = p.id
        WHERE p.document_id = NEW.id AND p.method = 'cheque' AND (c.id IS NULL OR NOT EXISTS (
            SELECT 1 FROM cheque_events e WHERE e.cheque_id = c.id AND e.document_id = NEW.id AND e.status = 'pending')));
END;

CREATE TRIGGER cheque_events_insert_guard BEFORE INSERT ON cheque_events BEGIN
    SELECT RAISE(ABORT, 'حالة الشيك تتغير داخل عملية الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
    SELECT RAISE(ABORT, 'انتقال حالة الشيك غير مسموح') WHERE NOT (
        (NEW.status = 'pending' AND NOT EXISTS (SELECT 1 FROM cheque_events WHERE cheque_id = NEW.cheque_id))
        OR (NEW.status IN ('cleared', 'bounced') AND (SELECT status FROM cheque_events WHERE cheque_id = NEW.cheque_id ORDER BY id DESC LIMIT 1) = 'pending')
        OR (NEW.status = 'bounced' AND (SELECT status FROM cheque_events WHERE cheque_id = NEW.cheque_id ORDER BY id DESC LIMIT 1) = 'cleared'));
    SELECT RAISE(ABORT, 'يجب تحديد حساب بنكي نشط') WHERE NEW.bank_account_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM cash_accounts WHERE id = NEW.bank_account_id AND kind = 'bank' AND active = 1);
END;
