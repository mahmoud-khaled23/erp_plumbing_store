-- New documents and immutable reconciliation/closing evidence; no historical edits.
INSERT INTO document_types(code,name_ar,requires_journal,requires_lines,can_post) VALUES
('year_end_close','إقفال السنة المالية',1,0,1),
('bad_debt','إعدام دين عميل',1,0,1),
('customer_credit','إشعار دائن مالي للعميل',1,0,1),
('settlement_discount','خصم تسوية عميل',1,0,1),
('bank_match','مطابقة كشف حساب',0,0,1);
INSERT INTO permissions(code,name_ar) VALUES
('finance.close','ترحيل إقفال السنة'),('finance.close_reverse','عكس إقفال السنة'),
('finance.reconcile','مطابقة الخزينة والبنك'),('finance.writeoff','إعدام ديون العملاء'),
('finance.credit_note','إشعارات العملاء الدائنة'),('finance.settlement_discount','خصومات تسوية العملاء');
INSERT INTO role_permissions SELECT 1,code FROM permissions WHERE code IN
('finance.close','finance.close_reverse','finance.reconcile','finance.writeoff','finance.credit_note','finance.settlement_discount');
INSERT INTO role_permissions SELECT 4,code FROM permissions WHERE code IN
('finance.close','finance.reconcile','finance.writeoff','finance.credit_note','finance.settlement_discount');
INSERT INTO accounts(code,name_ar,parent_id,kind,is_postable)
VALUES ('SYS-BAD-DEBT','ديون معدومة',5,'expense',1);
INSERT INTO posting_accounts(key,account_id) VALUES ('retained_earnings',302);
INSERT INTO posting_accounts SELECT 'bad_debt',id FROM accounts WHERE code='SYS-BAD-DEBT';

CREATE TABLE year_closings (
    document_id INTEGER PRIMARY KEY REFERENCES documents(id),
    period_id INTEGER NOT NULL REFERENCES fiscal_periods(id),
    result_minor INTEGER NOT NULL,
    preview_hash TEXT NOT NULL CHECK(length(preview_hash)=64)
) STRICT;
CREATE INDEX idx_year_closings_period ON year_closings(period_id);
CREATE TRIGGER year_closings_insert BEFORE INSERT ON year_closings BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents WHERE id=NEW.document_id AND kind='year_end_close' AND status='draft' AND length(trim(reason))>0)
        THEN RAISE(ABORT,'مستند الإقفال غير صالح') END;
    SELECT CASE WHEN EXISTS(SELECT 1 FROM year_closings c JOIN documents d ON d.id=c.document_id WHERE c.period_id=NEW.period_id AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted'))
        THEN RAISE(ABORT,'الفترة مقفلة بالفعل') END;
END;
CREATE TRIGGER year_closings_no_update BEFORE UPDATE ON year_closings BEGIN SELECT RAISE(ABORT,'سجل الإقفال ثابت'); END;
CREATE TRIGGER year_closings_no_delete BEFORE DELETE ON year_closings BEGIN SELECT RAISE(ABORT,'سجل الإقفال ثابت'); END;
CREATE TRIGGER fiscal_closed_unlock BEFORE UPDATE OF locked_at ON fiscal_periods
WHEN NEW.locked_at IS NULL AND EXISTS(SELECT 1 FROM year_closings c JOIN documents d ON d.id=c.document_id WHERE c.period_id=OLD.id AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted'))
BEGIN SELECT RAISE(ABORT,'اعكس مستند إقفال السنة قبل فتح الفترة'); END;

CREATE TABLE receivable_adjustments (
    document_id INTEGER PRIMARY KEY REFERENCES documents(id),
    source_debit_line_id INTEGER NOT NULL REFERENCES journal_lines(id),
    amount_minor INTEGER NOT NULL CHECK(amount_minor BETWEEN 1 AND 1000000000000000)
) STRICT;
CREATE INDEX idx_receivable_adjustments_source ON receivable_adjustments(source_debit_line_id);
CREATE TRIGGER receivable_adjustments_insert BEFORE INSERT ON receivable_adjustments BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents d JOIN journal_lines j ON j.id=NEW.source_debit_line_id JOIN accounts a ON a.id=j.account_id WHERE d.id=NEW.document_id AND d.status='draft' AND d.kind IN ('bad_debt','customer_credit','settlement_discount') AND length(trim(d.reason))>0 AND j.party_id=d.party_id AND j.debit_minor>=NEW.amount_minor AND a.control_kind='receivable')
        THEN RAISE(ABORT,'تسوية العميل غير صالحة') END;
END;
CREATE TRIGGER receivable_adjustments_no_update BEFORE UPDATE ON receivable_adjustments BEGIN SELECT RAISE(ABORT,'تسوية العميل ثابتة'); END;
CREATE TRIGGER receivable_adjustments_no_delete BEFORE DELETE ON receivable_adjustments BEGIN SELECT RAISE(ABORT,'تسوية العميل ثابتة'); END;

CREATE TABLE bank_statements (
    id INTEGER PRIMARY KEY,
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id),
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    starts_on TEXT NOT NULL CHECK(date(starts_on,'+0 days') IS starts_on AND length(starts_on)=10),
    ends_on TEXT NOT NULL CHECK(date(ends_on,'+0 days') IS ends_on AND length(ends_on)=10 AND ends_on>=starts_on),
    reference TEXT NOT NULL CHECK(length(trim(reference)) BETWEEN 1 AND 200),
    opening_minor INTEGER NOT NULL CHECK(opening_minor BETWEEN -2000000000000000 AND 2000000000000000),
    closing_minor INTEGER NOT NULL CHECK(closing_minor BETWEEN -2000000000000000 AND 2000000000000000),
    source_hash TEXT NOT NULL CHECK(length(source_hash)=64),
    line_count INTEGER NOT NULL CHECK(line_count BETWEEN 1 AND 5000),
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(cash_account_id,reference),
    UNIQUE(cash_account_id,source_hash)
) STRICT;
CREATE INDEX idx_bank_statements_account ON bank_statements(account_id,ends_on);
CREATE INDEX idx_bank_statements_user ON bank_statements(created_by);
CREATE TABLE bank_statement_lines (
    id INTEGER PRIMARY KEY,
    statement_id INTEGER NOT NULL REFERENCES bank_statements(id),
    line_number INTEGER NOT NULL CHECK(line_number BETWEEN 1 AND 5000),
    value_date TEXT NOT NULL CHECK(date(value_date,'+0 days') IS value_date AND length(value_date)=10),
    amount_minor INTEGER NOT NULL CHECK(amount_minor BETWEEN -1000000000000000 AND 1000000000000000 AND amount_minor<>0),
    reference TEXT NOT NULL,
    description TEXT NOT NULL,
    UNIQUE(statement_id,line_number)
) STRICT;
CREATE INDEX idx_bank_statement_lines_date ON bank_statement_lines(value_date);
CREATE TABLE bank_statement_voids (
    statement_id INTEGER PRIMARY KEY REFERENCES bank_statements(id),
    document_id INTEGER NOT NULL UNIQUE REFERENCES documents(id)
) STRICT;
CREATE TABLE reconciliation_matches (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL UNIQUE REFERENCES documents(id),
    statement_line_id INTEGER NOT NULL REFERENCES bank_statement_lines(id),
    journal_line_id INTEGER NOT NULL REFERENCES journal_lines(id),
    amount_minor INTEGER NOT NULL CHECK(amount_minor BETWEEN -1000000000000000 AND 1000000000000000 AND amount_minor<>0),
    reversal_of_id INTEGER UNIQUE REFERENCES reconciliation_matches(id),
    CHECK((amount_minor<0)=(reversal_of_id IS NOT NULL))
) STRICT;
CREATE INDEX idx_reconciliation_matches_statement ON reconciliation_matches(statement_line_id);
CREATE INDEX idx_reconciliation_matches_journal ON reconciliation_matches(journal_line_id);
CREATE TRIGGER bank_statements_insert BEFORE INSERT ON bank_statements BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM cash_accounts WHERE id=NEW.cash_account_id AND account_id=NEW.account_id AND active=1) THEN RAISE(ABORT,'حساب الكشف غير صالح') END;
END;
CREATE TRIGGER bank_statements_no_update BEFORE UPDATE ON bank_statements BEGIN SELECT RAISE(ABORT,'الكشف ثابت؛ ألغ الكشف وأدخل كشفاً جديداً'); END;
CREATE TRIGGER bank_statements_no_delete BEFORE DELETE ON bank_statements BEGIN SELECT RAISE(ABORT,'الكشف ثابت'); END;
CREATE TRIGGER bank_statement_lines_insert BEFORE INSERT ON bank_statement_lines BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM bank_statements WHERE id=NEW.statement_id AND NEW.value_date BETWEEN starts_on AND ends_on AND NEW.line_number<=line_count) THEN RAISE(ABORT,'تاريخ أو رقم سطر الكشف غير صالح') END;
    SELECT CASE WHEN EXISTS(SELECT 1 FROM reconciliation_matches m JOIN bank_statement_lines l ON l.id=m.statement_line_id WHERE l.statement_id=NEW.statement_id) OR EXISTS(SELECT 1 FROM bank_statement_voids WHERE statement_id=NEW.statement_id) THEN RAISE(ABORT,'لا يمكن إضافة سطور إلى كشف مستخدم') END;
END;
CREATE TRIGGER bank_statement_lines_no_update BEFORE UPDATE ON bank_statement_lines BEGIN SELECT RAISE(ABORT,'سطر الكشف ثابت'); END;
CREATE TRIGGER bank_statement_lines_no_delete BEFORE DELETE ON bank_statement_lines BEGIN SELECT RAISE(ABORT,'سطر الكشف ثابت'); END;
CREATE TRIGGER bank_statement_voids_insert BEFORE INSERT ON bank_statement_voids BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents WHERE id=NEW.document_id AND kind='bank_match' AND status='posting' AND length(trim(reason))>0) THEN RAISE(ABORT,'مستند إلغاء الكشف غير صالح') END;
    SELECT CASE WHEN EXISTS(SELECT 1 FROM reconciliation_matches m JOIN bank_statement_lines l ON l.id=m.statement_line_id WHERE l.statement_id=NEW.statement_id AND m.amount_minor>0 AND NOT EXISTS(SELECT 1 FROM reconciliation_matches r WHERE r.reversal_of_id=m.id)) THEN RAISE(ABORT,'ألغ المطابقات أولاً') END;
END;
CREATE TRIGGER bank_statement_voids_no_update BEFORE UPDATE ON bank_statement_voids BEGIN SELECT RAISE(ABORT,'إلغاء الكشف ثابت'); END;
CREATE TRIGGER bank_statement_voids_no_delete BEFORE DELETE ON bank_statement_voids BEGIN SELECT RAISE(ABORT,'إلغاء الكشف ثابت'); END;
CREATE TRIGGER reconciliation_matches_insert BEFORE INSERT ON reconciliation_matches BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM documents d JOIN bank_statement_lines l ON l.id=NEW.statement_line_id JOIN bank_statements s ON s.id=l.statement_id JOIN journal_lines j ON j.id=NEW.journal_line_id JOIN documents src ON src.id=j.document_id WHERE d.id=NEW.document_id AND d.kind='bank_match' AND d.status='posting' AND length(trim(d.reason))>0 AND j.account_id=s.account_id AND src.status='posted' AND (j.debit_minor-j.credit_minor)*(CASE WHEN l.amount_minor>0 THEN 1 ELSE -1 END)>0 AND d.document_date>=l.value_date AND d.document_date>=src.document_date AND NOT EXISTS(SELECT 1 FROM bank_statement_voids v WHERE v.statement_id=s.id)) THEN RAISE(ABORT,'المطابقة لا تطابق الحساب أو الاتجاه أو التاريخ') END;
    SELECT CASE WHEN NEW.amount_minor>0 AND (
        NEW.amount_minor>abs((SELECT amount_minor FROM bank_statement_lines WHERE id=NEW.statement_line_id))-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE statement_line_id=NEW.statement_line_id),0)
        OR NEW.amount_minor>(SELECT debit_minor+credit_minor FROM journal_lines WHERE id=NEW.journal_line_id)-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE journal_line_id=NEW.journal_line_id),0)
        OR EXISTS(SELECT 1 FROM journal_lines j JOIN documents r ON r.reversal_of_id=j.document_id WHERE j.id=NEW.journal_line_id AND r.status='posted')) THEN RAISE(ABORT,'المبلغ مطابق مسبقاً أو المستند معكوس') END;
    SELECT CASE WHEN NEW.amount_minor<0 AND NOT EXISTS(SELECT 1 FROM reconciliation_matches m JOIN documents d ON d.id=m.document_id JOIN documents n ON n.id=NEW.document_id WHERE m.id=NEW.reversal_of_id AND m.amount_minor=-NEW.amount_minor AND m.statement_line_id=NEW.statement_line_id AND m.journal_line_id=NEW.journal_line_id AND n.document_date>=d.document_date) THEN RAISE(ABORT,'عكس المطابقة غير صالح') END;
END;
CREATE TRIGGER reconciliation_matches_no_update BEFORE UPDATE ON reconciliation_matches BEGIN SELECT RAISE(ABORT,'المطابقة ثابتة؛ استخدم إلغاء المطابقة'); END;
CREATE TRIGGER reconciliation_matches_no_delete BEFORE DELETE ON reconciliation_matches BEGIN SELECT RAISE(ABORT,'المطابقة ثابتة؛ استخدم إلغاء المطابقة'); END;
CREATE TRIGGER bank_cash_account_guard BEFORE UPDATE OF account_id ON cash_accounts
WHEN NEW.account_id<>OLD.account_id AND EXISTS(SELECT 1 FROM bank_statements WHERE cash_account_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'لا يمكن تغيير حساب دفتر لكشف مستخدم'); END;
CREATE TRIGGER reconciliation_reverse_guard BEFORE UPDATE OF status ON documents
WHEN NEW.status='posting' AND NEW.kind='reversal' AND EXISTS(SELECT 1 FROM reconciliation_matches m JOIN journal_lines j ON j.id=m.journal_line_id WHERE j.document_id=NEW.reversal_of_id AND m.amount_minor>0 AND NOT EXISTS(SELECT 1 FROM reconciliation_matches r WHERE r.reversal_of_id=m.id))
BEGIN SELECT RAISE(ABORT,'ألغ المطابقة البنكية قبل عكس المستند'); END;
CREATE TRIGGER finance_documents_metadata BEFORE UPDATE OF status ON documents WHEN NEW.status='posting' BEGIN
    SELECT CASE WHEN NEW.kind='year_end_close' AND NOT EXISTS(SELECT 1 FROM year_closings WHERE document_id=NEW.id) THEN RAISE(ABORT,'بيانات الإقفال مفقودة') END;
    SELECT CASE WHEN NEW.kind IN ('bad_debt','customer_credit','settlement_discount') AND NOT EXISTS(SELECT 1 FROM receivable_adjustments WHERE document_id=NEW.id) THEN RAISE(ABORT,'بيانات تسوية العميل مفقودة') END;
END;
CREATE TRIGGER closing_reverse_period_guard BEFORE UPDATE OF status ON documents
WHEN NEW.status='posting' AND NEW.kind='reversal' AND EXISTS(SELECT 1 FROM year_closings WHERE document_id=NEW.reversal_of_id) BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM year_closings c JOIN fiscal_periods p ON p.id=c.period_id JOIN fiscal_periods n ON n.starts_on=date(p.ends_on,'+1 day') WHERE c.document_id=NEW.reversal_of_id AND NEW.document_date BETWEEN n.starts_on AND n.ends_on AND n.locked_at IS NULL) THEN RAISE(ABORT,'عكس الإقفال داخل الفترة التالية المفتوحة فقط') END;
END;
