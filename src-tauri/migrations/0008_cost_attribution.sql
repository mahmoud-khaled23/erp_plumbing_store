-- Preserve the link from a current-date cost correction to its original issue/return.
-- This lets later sales returns restore the adjusted original COGS exactly.
CREATE TABLE cost_attributions (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id),
    source_movement_id INTEGER NOT NULL REFERENCES stock_movements(id),
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    delta_minor INTEGER NOT NULL CHECK (delta_minor BETWEEN -2000000000000000 AND 2000000000000000 AND delta_minor <> 0)
) STRICT;
CREATE INDEX idx_cost_attributions_document ON cost_attributions(document_id);
CREATE INDEX idx_cost_attributions_source ON cost_attributions(source_movement_id);
CREATE INDEX idx_cost_attributions_account ON cost_attributions(account_id);
CREATE TRIGGER cost_attributions_insert_guard BEFORE INSERT ON cost_attributions BEGIN
    SELECT RAISE(ABORT, 'توزيع التكلفة ينشأ داخل الترحيل فقط') WHERE
        COALESCE((SELECT status FROM documents WHERE id = NEW.document_id), '') <> 'posting';
    SELECT RAISE(ABORT, 'حساب توزيع التكلفة غير صالح') WHERE NOT EXISTS (
        SELECT 1 FROM posting_accounts WHERE account_id = NEW.account_id AND key IN ('cogs', 'stock_variance', 'purchase_variance'));
END;
CREATE TRIGGER cost_attributions_immutable_update BEFORE UPDATE ON cost_attributions BEGIN
    SELECT RAISE(ABORT, 'توزيع التكلفة غير قابل للتعديل؛ استخدم مستند عكس');
END;
CREATE TRIGGER cost_attributions_immutable_delete BEFORE DELETE ON cost_attributions BEGIN
    SELECT RAISE(ABORT, 'توزيع التكلفة غير قابل للحذف؛ استخدم مستند عكس');
END;
