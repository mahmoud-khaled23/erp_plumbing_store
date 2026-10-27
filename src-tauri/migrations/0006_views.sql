-- Public report views exclude in-flight documents. Rust derives average unit cost with i128.
CREATE VIEW v_stock_on_hand AS
SELECT s.product_id, s.warehouse_id,
    SUM(s.quantity_micros) AS quantity_micros,
    SUM(s.value_minor) AS value_minor,
    MAX(s.id) AS last_movement_id
FROM stock_movements s JOIN documents d ON d.id = s.document_id
WHERE d.status = 'posted'
GROUP BY s.product_id, s.warehouse_id;

CREATE VIEW v_stock_card AS
SELECT s.id, s.product_id, s.warehouse_id, s.movement_type, s.quantity_micros,
    s.unit_cost_micropiastres, s.value_minor, s.document_id, d.number,
    d.document_date, s.created_at, s.created_by, s.reason,
    SUM(s.quantity_micros) OVER (PARTITION BY s.product_id, s.warehouse_id ORDER BY s.id ROWS UNBOUNDED PRECEDING) AS running_quantity_micros,
    SUM(s.value_minor) OVER (PARTITION BY s.product_id, s.warehouse_id ORDER BY s.id ROWS UNBOUNDED PRECEDING) AS running_value_minor
FROM stock_movements s JOIN documents d ON d.id = s.document_id WHERE d.status = 'posted';

CREATE VIEW v_low_stock AS
SELECT p.id AS product_id, p.code, p.name_ar, w.id AS warehouse_id, w.name_ar AS warehouse_name,
    COALESCE(s.quantity_micros, 0) AS quantity_micros,
    COALESCE(ws.min_stock_micros, p.min_stock_micros) AS min_stock_micros
FROM products p CROSS JOIN warehouses w
LEFT JOIN warehouse_product_settings ws ON ws.product_id = p.id AND ws.warehouse_id = w.id
LEFT JOIN v_stock_on_hand s ON s.product_id = p.id AND s.warehouse_id = w.id
WHERE p.active = 1 AND w.active = 1 AND COALESCE(s.quantity_micros, 0) <= COALESCE(ws.min_stock_micros, p.min_stock_micros);

CREATE VIEW v_journal AS
SELECT l.*, d.number AS document_number, d.kind AS document_kind, d.document_date,
    d.posted_at, a.code AS account_code, a.name_ar AS account_name, a.kind AS account_kind, a.control_kind
FROM journal_lines l JOIN documents d ON d.id = l.document_id JOIN accounts a ON a.id = l.account_id
WHERE d.status = 'posted';

CREATE VIEW v_trial_balance AS
SELECT a.id AS account_id, a.code, a.name_ar, a.kind,
    COALESCE(SUM(j.debit_minor), 0) AS debit_minor,
    COALESCE(SUM(j.credit_minor), 0) AS credit_minor,
    COALESCE(SUM(j.debit_minor - j.credit_minor), 0) AS balance_minor
FROM accounts a LEFT JOIN v_journal j ON j.account_id = a.id WHERE a.is_postable = 1 GROUP BY a.id;

CREATE VIEW v_party_open_items AS
WITH debit_matches AS (
    SELECT s.debit_line_id AS line_id, SUM(s.amount_minor) AS amount_minor
    FROM settlements s JOIN documents d ON d.id = s.document_id WHERE d.status = 'posted' GROUP BY s.debit_line_id
), credit_matches AS (
    SELECT s.credit_line_id AS line_id, SUM(s.amount_minor) AS amount_minor
    FROM settlements s JOIN documents d ON d.id = s.document_id WHERE d.status = 'posted' GROUP BY s.credit_line_id
)
SELECT j.id AS journal_line_id, j.party_id, j.account_id, j.control_kind,
    j.document_id, j.document_number, j.document_kind, j.document_date, j.due_date,
    j.debit_minor - j.credit_minor AS original_signed_minor,
    CASE WHEN j.debit_minor > 0 THEN j.debit_minor - COALESCE(dm.amount_minor, 0)
        ELSE -j.credit_minor + COALESCE(cm.amount_minor, 0) END AS outstanding_signed_minor
FROM v_journal j LEFT JOIN debit_matches dm ON dm.line_id = j.id LEFT JOIN credit_matches cm ON cm.line_id = j.id
WHERE j.control_kind IN ('receivable', 'payable');

CREATE VIEW v_party_balances AS
SELECT p.id AS party_id, p.code, p.name_ar,
    COALESCE(SUM(CASE WHEN j.control_kind = 'receivable' THEN j.debit_minor - j.credit_minor ELSE 0 END), 0) AS receivable_minor,
    COALESCE(SUM(CASE WHEN j.control_kind = 'payable' THEN j.credit_minor - j.debit_minor ELSE 0 END), 0) AS payable_minor
FROM parties p LEFT JOIN v_journal j ON j.party_id = p.id GROUP BY p.id;

CREATE VIEW v_purchase_order_progress AS
WITH accepted AS (
    SELECT r.purchase_order_line_id AS order_line_id, SUM(l.base_quantity_micros) AS accepted_micros
    FROM receipt_lines r JOIN document_lines l ON l.id = r.document_line_id JOIN documents d ON d.id = l.document_id
    WHERE d.status = 'posted' AND NOT EXISTS (SELECT 1 FROM documents rev WHERE rev.reversal_of_id = d.id AND rev.status = 'posted')
    GROUP BY r.purchase_order_line_id
)
SELECT d.id AS document_id, d.number, d.party_id, d.document_date,
    SUM(l.base_quantity_micros) AS ordered_micros, SUM(COALESCE(a.accepted_micros, 0)) AS accepted_micros,
    CASE WHEN d.status = 'cancelled' THEN 'cancelled' WHEN d.status = 'draft' THEN 'draft'
        WHEN d.status = 'closed' OR SUM(COALESCE(a.accepted_micros, 0)) = SUM(l.base_quantity_micros) THEN 'closed'
        WHEN SUM(COALESCE(a.accepted_micros, 0)) > 0 THEN 'partially_received' ELSE 'approved' END AS progress
FROM documents d JOIN document_lines l ON l.document_id = d.id LEFT JOIN accepted a ON a.order_line_id = l.id
WHERE d.kind = 'purchase_order' GROUP BY d.id;

CREATE VIEW v_cheque_register AS
SELECT c.id, c.cheque_number, c.bank_name, c.party_id, c.due_date, p.amount_minor, p.direction,
    p.document_id, e.status, e.bank_account_id, e.created_at AS status_changed_at
FROM cheques c JOIN payment_legs p ON p.id = c.payment_leg_id JOIN documents d ON d.id = p.document_id
JOIN cheque_events e ON e.id = (
    SELECT ce.id FROM cheque_events ce JOIN documents ed ON ed.id = ce.document_id
    WHERE ce.cheque_id = c.id AND ed.status = 'posted' ORDER BY ce.id DESC LIMIT 1)
WHERE d.status = 'posted';

CREATE VIEW v_cash_balances AS
SELECT c.id AS cash_account_id, c.name_ar, c.kind, c.active, COALESCE(SUM(j.debit_minor - j.credit_minor), 0) AS balance_minor
FROM cash_accounts c LEFT JOIN v_journal j ON j.account_id = c.account_id GROUP BY c.id;

CREATE VIEW v_cash_flow AS
SELECT j.document_date, j.document_id, j.document_kind, c.id AS cash_account_id,
    j.debit_minor AS cash_in_minor, j.credit_minor AS cash_out_minor
FROM v_journal j JOIN cash_accounts c ON c.account_id = j.account_id;

-- Revenue and COGS dimensions are snapshotted on journal lines at posting time.
CREATE VIEW v_product_profit AS
SELECT j.product_id, j.category_id, j.party_id, j.document_date,
    SUM(CASE WHEN j.account_kind = 'income' THEN j.credit_minor - j.debit_minor ELSE 0 END) AS net_sales_minor,
    SUM(CASE WHEN j.account_id = (SELECT account_id FROM posting_accounts WHERE key = 'cogs')
        THEN j.debit_minor - j.credit_minor ELSE 0 END) AS cogs_minor
FROM v_journal j WHERE j.product_id IS NOT NULL GROUP BY j.product_id, j.category_id, j.party_id, j.document_date;

CREATE VIEW v_vat_summary AS
SELECT document_date,
    SUM(CASE WHEN control_kind = 'vat_input' THEN debit_minor - credit_minor ELSE 0 END) AS input_vat_minor,
    SUM(CASE WHEN control_kind = 'vat_output' THEN credit_minor - debit_minor ELSE 0 END) AS output_vat_minor
FROM v_journal WHERE control_kind IN ('vat_input', 'vat_output') GROUP BY document_date;
