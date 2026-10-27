-- Bind :as_of as a validated store-local ISO calendar date.
-- Historical balances must exclude later settlements, including later unmatches.
-- Positive AR = customer debt; negative AP = supplier debt. Advances remain separate.
WITH matched_debits AS (
    SELECT s.debit_line_id AS line_id, SUM(s.amount_minor) AS matched_minor
    FROM settlements s JOIN documents d ON d.id = s.document_id
    WHERE d.status = 'posted' AND d.document_date <= :as_of GROUP BY s.debit_line_id
), matched_credits AS (
    SELECT s.credit_line_id AS line_id, SUM(s.amount_minor) AS matched_minor
    FROM settlements s JOIN documents d ON d.id = s.document_id
    WHERE d.status = 'posted' AND d.document_date <= :as_of GROUP BY s.credit_line_id
), open_items AS (
    SELECT j.party_id, j.control_kind, j.due_date,
        (CASE WHEN j.debit_minor > 0 THEN j.debit_minor - COALESCE(md.matched_minor, 0)
            ELSE -j.credit_minor + COALESCE(mc.matched_minor, 0) END)
            * CASE j.control_kind WHEN 'payable' THEN -1 ELSE 1 END AS amount_minor,
        CAST(julianday(:as_of) - julianday(j.due_date) AS INTEGER) AS days_overdue
    FROM v_journal j LEFT JOIN matched_debits md ON md.line_id = j.id LEFT JOIN matched_credits mc ON mc.line_id = j.id
    WHERE j.document_date <= :as_of AND j.control_kind IN ('receivable', 'payable')
)
SELECT o.party_id, p.code, p.name_ar, o.control_kind,
    SUM(CASE WHEN amount_minor > 0 AND days_overdue < 0 THEN amount_minor ELSE 0 END) AS not_due_minor,
    SUM(CASE WHEN amount_minor > 0 AND days_overdue BETWEEN 0 AND 30 THEN amount_minor ELSE 0 END) AS days_0_30_minor,
    SUM(CASE WHEN amount_minor > 0 AND days_overdue BETWEEN 31 AND 60 THEN amount_minor ELSE 0 END) AS days_31_60_minor,
    SUM(CASE WHEN amount_minor > 0 AND days_overdue BETWEEN 61 AND 90 THEN amount_minor ELSE 0 END) AS days_61_90_minor,
    SUM(CASE WHEN amount_minor > 0 AND days_overdue > 90 THEN amount_minor ELSE 0 END) AS over_90_minor,
    SUM(CASE WHEN amount_minor < 0 THEN -amount_minor ELSE 0 END) AS unapplied_credit_minor,
    SUM(amount_minor) AS net_due_minor
FROM open_items o JOIN parties p ON p.id = o.party_id GROUP BY o.party_id, o.control_kind
ORDER BY p.normalized_name, o.party_id, o.control_kind;
