-- Draft details are editable only before approval/posting; both old and new parents are checked.

CREATE TRIGGER document_lines_draft_insert BEFORE INSERT ON document_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER document_lines_draft_update BEFORE UPDATE ON document_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_id <> OLD.document_id;
END;

CREATE TRIGGER document_lines_draft_delete BEFORE DELETE ON document_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER receipt_lines_draft_insert BEFORE INSERT ON receipt_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = NEW.document_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER receipt_lines_draft_update BEFORE UPDATE ON receipt_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = OLD.document_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = NEW.document_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_line_id <> OLD.document_line_id;
END;

CREATE TRIGGER receipt_lines_draft_delete BEFORE DELETE ON receipt_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = OLD.document_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER purchase_matches_draft_insert BEFORE INSERT ON purchase_matches BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = NEW.invoice_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER purchase_matches_draft_update BEFORE UPDATE ON purchase_matches BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = OLD.invoice_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = NEW.invoice_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.invoice_line_id <> OLD.invoice_line_id;
END;

CREATE TRIGGER purchase_matches_draft_delete BEFORE DELETE ON purchase_matches BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM document_lines WHERE id = OLD.invoice_line_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER landed_charges_draft_insert BEFORE INSERT ON landed_charges BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER landed_charges_draft_update BEFORE UPDATE ON landed_charges BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_id <> OLD.document_id;
END;

CREATE TRIGGER landed_charges_draft_delete BEFORE DELETE ON landed_charges BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER landed_allocations_draft_insert BEFORE INSERT ON landed_allocations BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM landed_charges WHERE id = NEW.charge_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER landed_allocations_draft_update BEFORE UPDATE ON landed_allocations BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM landed_charges WHERE id = OLD.charge_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM landed_charges WHERE id = NEW.charge_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.charge_id <> OLD.charge_id;
END;

CREATE TRIGGER landed_allocations_draft_delete BEFORE DELETE ON landed_allocations BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM landed_charges WHERE id = OLD.charge_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER stocktake_lines_draft_insert BEFORE INSERT ON stocktake_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER stocktake_lines_draft_update BEFORE UPDATE ON stocktake_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_id <> OLD.document_id;
END;

CREATE TRIGGER stocktake_lines_draft_delete BEFORE DELETE ON stocktake_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER payment_legs_draft_insert BEFORE INSERT ON payment_legs BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER payment_legs_draft_update BEFORE UPDATE ON payment_legs BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_id <> OLD.document_id;
END;

CREATE TRIGGER payment_legs_draft_delete BEFORE DELETE ON payment_legs BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER cheques_draft_insert BEFORE INSERT ON cheques BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM payment_legs WHERE id = NEW.payment_leg_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER cheques_draft_update BEFORE UPDATE ON cheques BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM payment_legs WHERE id = OLD.payment_leg_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM payment_legs WHERE id = NEW.payment_leg_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.payment_leg_id <> OLD.payment_leg_id;
END;

CREATE TRIGGER cheques_draft_delete BEFORE DELETE ON cheques BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = (SELECT document_id FROM payment_legs WHERE id = OLD.payment_leg_id) AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER expense_lines_draft_insert BEFORE INSERT ON expense_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER expense_lines_draft_update BEFORE UPDATE ON expense_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = NEW.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
    SELECT RAISE(ABORT, 'لا يمكن نقل التفاصيل إلى مستند آخر') WHERE NEW.document_id <> OLD.document_id;
END;

CREATE TRIGGER expense_lines_draft_delete BEFORE DELETE ON expense_lines BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل تفاصيل مستند معتمد أو مرحل أو فترة مغلقة') WHERE NOT EXISTS (
        SELECT 1 FROM documents d WHERE d.id = OLD.document_id AND d.status IN ('draft', 'held')
        AND NOT EXISTS (SELECT 1 FROM fiscal_periods p WHERE d.document_date BETWEEN p.starts_on AND p.ends_on AND p.locked_at IS NOT NULL));
END;

CREATE TRIGGER stock_movements_immutable_update BEFORE UPDATE ON stock_movements BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER stock_movements_immutable_delete BEFORE DELETE ON stock_movements BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER journal_entries_immutable_update BEFORE UPDATE ON journal_entries BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER journal_entries_immutable_delete BEFORE DELETE ON journal_entries BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER journal_lines_immutable_update BEFORE UPDATE ON journal_lines BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER journal_lines_immutable_delete BEFORE DELETE ON journal_lines BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER settlements_immutable_update BEFORE UPDATE ON settlements BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER settlements_immutable_delete BEFORE DELETE ON settlements BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER cheque_events_immutable_update BEFORE UPDATE ON cheque_events BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER cheque_events_immutable_delete BEFORE DELETE ON cheque_events BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER audit_log_immutable_update BEFORE UPDATE ON audit_log BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER audit_log_immutable_delete BEFORE DELETE ON audit_log BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER document_events_immutable_update BEFORE UPDATE ON document_events BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER document_events_immutable_delete BEFORE DELETE ON document_events BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER command_receipts_immutable_update BEFORE UPDATE ON command_receipts BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER command_receipts_immutable_delete BEFORE DELETE ON command_receipts BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER import_batches_immutable_update BEFORE UPDATE ON import_batches BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER import_batches_immutable_delete BEFORE DELETE ON import_batches BEGIN
    SELECT RAISE(ABORT, 'السجل غير قابل للتعديل أو الحذف؛ استخدم مستند عكس');
END;

CREATE TRIGGER categories_acyclic_insert BEFORE INSERT ON categories
WHEN NEW.parent_id IS NOT NULL BEGIN
    SELECT RAISE(ABORT, 'التسلسل الهرمي يحتوي على حلقة') WHERE NEW.id IN (
        WITH RECURSIVE ancestors(id, parent_id) AS (
            SELECT id, parent_id FROM categories WHERE id = NEW.parent_id
            UNION
            SELECT p.id, p.parent_id FROM categories p JOIN ancestors a ON p.id = a.parent_id
        ) SELECT id FROM ancestors);
END;

CREATE TRIGGER categories_acyclic_update BEFORE UPDATE ON categories
WHEN NEW.parent_id IS NOT NULL BEGIN
    SELECT RAISE(ABORT, 'التسلسل الهرمي يحتوي على حلقة') WHERE NEW.id IN (
        WITH RECURSIVE ancestors(id, parent_id) AS (
            SELECT id, parent_id FROM categories WHERE id = NEW.parent_id
            UNION
            SELECT p.id, p.parent_id FROM categories p JOIN ancestors a ON p.id = a.parent_id
        ) SELECT id FROM ancestors);
END;

CREATE TRIGGER accounts_acyclic_insert BEFORE INSERT ON accounts
WHEN NEW.parent_id IS NOT NULL BEGIN
    SELECT RAISE(ABORT, 'التسلسل الهرمي يحتوي على حلقة') WHERE NEW.id IN (
        WITH RECURSIVE ancestors(id, parent_id) AS (
            SELECT id, parent_id FROM accounts WHERE id = NEW.parent_id
            UNION
            SELECT p.id, p.parent_id FROM accounts p JOIN ancestors a ON p.id = a.parent_id
        ) SELECT id FROM ancestors);
END;

CREATE TRIGGER accounts_acyclic_update BEFORE UPDATE ON accounts
WHEN NEW.parent_id IS NOT NULL BEGIN
    SELECT RAISE(ABORT, 'التسلسل الهرمي يحتوي على حلقة') WHERE NEW.id IN (
        WITH RECURSIVE ancestors(id, parent_id) AS (
            SELECT id, parent_id FROM accounts WHERE id = NEW.parent_id
            UNION
            SELECT p.id, p.parent_id FROM accounts p JOIN ancestors a ON p.id = a.parent_id
        ) SELECT id FROM ancestors);
END;

CREATE TRIGGER cash_shifts_closed_guard BEFORE UPDATE ON cash_shifts WHEN OLD.closed_at IS NOT NULL BEGIN
    SELECT RAISE(ABORT, 'لا يمكن تعديل وردية مغلقة');
END;
CREATE TRIGGER cash_shifts_delete_guard BEFORE DELETE ON cash_shifts BEGIN
    SELECT RAISE(ABORT, 'لا يمكن حذف الوردية');
END;

