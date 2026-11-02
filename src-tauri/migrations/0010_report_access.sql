-- Read-only reporting actions; no change to posting rules or existing ledger rows.
INSERT INTO permissions(code,name_ar) VALUES
('reports.export','تصدير التقارير'), ('reports.drilldown','عرض تفاصيل التقارير');
INSERT INTO role_permissions(role_id,permission_code)
SELECT r.id,p.code FROM roles r CROSS JOIN permissions p
WHERE r.code IN ('admin','accountant') AND p.code IN ('reports.export','reports.drilldown');
