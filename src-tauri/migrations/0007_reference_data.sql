-- Safe reference data only. No passwords, transactions, fiscal dates or demo customers.
INSERT INTO store_settings(id) VALUES (1);
INSERT INTO roles(id, code, name_ar) VALUES
(1, 'admin', 'مدير النظام'), (2, 'cashier', 'أمين الصندوق'),
(3, 'storekeeper', 'أمين المخزن'), (4, 'accountant', 'المحاسب');
INSERT INTO permissions(code, name_ar) VALUES
('dashboard.read', 'عرض لوحة المتابعة'), ('products.read', 'عرض الأصناف'),
('products.write', 'إدارة الأصناف'), ('products.import', 'استيراد الأصناف'),
('inventory.read', 'عرض المخزون'), ('inventory.receive', 'استلام البضاعة'),
('inventory.transfer', 'تحويل المخزون'), ('inventory.adjust', 'تسوية وجرد المخزون'),
('sales.read', 'عرض المبيعات'), ('sales.write', 'إنشاء وترحيل المبيعات'),
('sales.return', 'مرتجعات المبيعات'), ('sales.discount', 'منح الخصومات'),
('sales.credit_override', 'تجاوز الحد الائتماني'), ('purchases.read', 'عرض المشتريات'),
('purchases.write', 'إدارة المشتريات'), ('purchases.approve', 'اعتماد أوامر الشراء'),
('parties.read', 'عرض العملاء والموردين'), ('parties.write', 'إدارة العملاء والموردين'),
('finance.read', 'عرض الحسابات'), ('finance.write', 'إدارة القيود والمصروفات'),
('payments.write', 'سندات القبض والصرف'), ('cheques.write', 'إدارة الشيكات'),
('cash.shift', 'فتح وإغلاق الوردية'), ('reports.read', 'عرض التقارير'),
('documents.reverse', 'عكس المستندات المرحلة'), ('periods.lock', 'إغلاق الفترات المالية'),
('periods.unlock', 'إعادة فتح الفترات المالية'), ('settings.write', 'تعديل الإعدادات'),
('users.manage', 'إدارة المستخدمين'), ('audit.read', 'عرض سجل المراجعة'),
('backup.create', 'إنشاء نسخة احتياطية'), ('backup.restore', 'استعادة نسخة احتياطية');
INSERT INTO role_permissions SELECT 1, code FROM permissions;
INSERT INTO role_permissions SELECT 2, code FROM permissions WHERE code IN (
    'dashboard.read', 'products.read', 'inventory.read', 'sales.read', 'sales.write',
    'sales.return', 'parties.read', 'payments.write', 'cash.shift');
INSERT INTO role_permissions SELECT 3, code FROM permissions WHERE code IN (
    'dashboard.read', 'products.read', 'products.write', 'products.import', 'inventory.read',
    'inventory.receive', 'inventory.transfer', 'inventory.adjust', 'purchases.read');
INSERT INTO role_permissions SELECT 4, code FROM permissions WHERE code IN (
    'dashboard.read', 'products.read', 'inventory.read', 'sales.read', 'purchases.read',
    'purchases.write', 'purchases.approve', 'parties.read', 'parties.write', 'finance.read',
    'finance.write', 'payments.write', 'cheques.write', 'cash.shift', 'reports.read',
    'documents.reverse', 'periods.lock', 'audit.read', 'backup.create');

INSERT INTO document_types(code, name_ar, requires_journal, requires_lines, can_post) VALUES
('sale', 'فاتورة مبيعات', 1, 1, 1),
('sale_return', 'مرتجع مبيعات', 1, 1, 1),
('quotation', 'عرض أسعار', 0, 1, 0),
('purchase_order', 'أمر شراء', 0, 1, 0),
('purchase', 'فاتورة مشتريات', 1, 1, 1),
('purchase_return', 'مرتجع مشتريات', 1, 1, 1),
('goods_receipt', 'إذن استلام بضاعة', 1, 1, 1),
('transfer', 'تحويل مخزني', 0, 1, 1),
('adjustment', 'تسوية مخزنية', 1, 0, 1),
('stocktake', 'محضر جرد', 1, 0, 1),
('opening_stock', 'مخزون افتتاحي', 1, 1, 1),
('opening_balance', 'أرصدة افتتاحية', 1, 0, 1),
('receipt_voucher', 'سند قبض', 1, 0, 1),
('payment_voucher', 'سند صرف', 1, 0, 1),
('cash_transfer', 'تحويل خزينة أو بنك', 1, 0, 1),
('expense', 'مصروف', 1, 0, 1),
('manual_journal', 'قيد يومية', 1, 0, 1),
('landed_cost', 'تكاليف إضافية للمخزون', 1, 0, 1),
('cost_adjustment', 'تسوية تكلفة المخزون', 1, 0, 1),
('settlement', 'تسوية حساب طرف', 0, 0, 1),
('cheque_clear', 'تحصيل أو صرف شيك', 1, 0, 1),
('cheque_bounce', 'ارتداد شيك', 1, 0, 1),
('cash_close', 'إغلاق وردية', 1, 0, 1),
('reversal', 'عكس مستند', 0, 0, 1);

INSERT INTO accounts(id, code, name_ar, parent_id, kind, control_kind, is_postable) VALUES
(1, '1000', 'الأصول', NULL, 'asset', NULL, 0),
(2, '2000', 'الالتزامات', NULL, 'liability', NULL, 0),
(3, '3000', 'حقوق الملكية', NULL, 'equity', NULL, 0),
(4, '4000', 'الإيرادات', NULL, 'income', NULL, 0),
(5, '5000', 'تكلفة المبيعات والمصروفات', NULL, 'expense', NULL, 0),
(101, '1101', 'الخزينة الرئيسية', 1, 'asset', 'cash', 1),
(102, '1102', 'البنك', 1, 'asset', 'bank', 1),
(103, '1110', 'شيكات تحت التحصيل', 1, 'asset', 'cheque_received', 1),
(110, '1200', 'العملاء', 1, 'asset', 'receivable', 1),
(120, '1300', 'مخزون البضاعة', 1, 'asset', 'inventory', 1),
(130, '1400', 'ضريبة قيمة مضافة على المدخلات', 1, 'asset', 'vat_input', 1),
(201, '2100', 'الموردون', 2, 'liability', 'payable', 1),
(202, '2110', 'شيكات صادرة', 2, 'liability', 'cheque_issued', 1),
(210, '2200', 'بضاعة مستلمة لم تصل فواتيرها', 2, 'liability', 'grni', 1),
(220, '2300', 'ضريبة قيمة مضافة على المخرجات', 2, 'liability', 'vat_output', 1),
(301, '3100', 'رأس المال', 3, 'equity', NULL, 1),
(302, '3200', 'الأرباح المحتجزة', 3, 'equity', NULL, 1),
(303, '3900', 'مقابل الأرصدة الافتتاحية', 3, 'equity', NULL, 1),
(401, '4100', 'مبيعات البضاعة', 4, 'income', NULL, 1),
(402, '4200', 'مردودات المبيعات', 4, 'income', NULL, 1),
(403, '4300', 'خصومات المبيعات', 4, 'income', NULL, 1),
(404, '4400', 'إيرادات متنوعة', 4, 'income', NULL, 1),
(501, '5100', 'تكلفة البضاعة المباعة', 5, 'expense', NULL, 1),
(502, '5110', 'فروق تكلفة المشتريات', 5, 'expense', NULL, 1),
(503, '5120', 'فروق جرد المخزون', 5, 'expense', NULL, 1),
(504, '5130', 'فروق الخزينة', 5, 'expense', NULL, 1),
(511, '5201', 'الإيجار', 5, 'expense', NULL, 1),
(512, '5202', 'الرواتب', 5, 'expense', NULL, 1),
(513, '5203', 'الكهرباء والمياه', 5, 'expense', NULL, 1),
(514, '5204', 'النقل', 5, 'expense', NULL, 1),
(515, '5205', 'الصيانة', 5, 'expense', NULL, 1),
(516, '5206', 'مصروفات متنوعة', 5, 'expense', NULL, 1);
INSERT INTO posting_accounts(key, account_id) VALUES
('receivable', 110), ('payable', 201), ('inventory', 120), ('grni', 210),
('vat_input', 130), ('vat_output', 220), ('opening_equity', 303),
('sales', 401), ('sales_returns', 402), ('sales_discounts', 403),
('cogs', 501), ('purchase_variance', 502), ('stock_variance', 503), ('cash_variance', 504),
('cheque_received', 103), ('cheque_issued', 202);
INSERT INTO cash_accounts(id, name_ar, kind, account_id) VALUES
(1, 'الخزينة الرئيسية', 'cash', 101), (2, 'الحساب البنكي', 'bank', 102);
INSERT INTO expense_categories(id, name_ar, account_id) VALUES
(1, 'إيجار', 511), (2, 'رواتب', 512), (3, 'كهرباء ومياه', 513),
(4, 'نقل', 514), (5, 'صيانة', 515), (6, 'مصروفات متنوعة', 516);
INSERT INTO warehouses(id, code, name_ar) VALUES (1, 'MAIN', 'المخزن الرئيسي');
INSERT INTO units(id, code, name_ar) VALUES
(1, 'piece', 'قطعة'), (2, 'carton', 'كرتونة'), (3, 'meter', 'متر'),
(4, 'roll', 'لفة'), (5, 'set', 'طقم'), (6, 'length', 'عود'), (7, 'bag', 'كيس');
INSERT INTO categories(id, name_ar) VALUES
(1, 'مواسير'), (2, 'وصلات'), (3, 'محابس'), (4, 'خلاطات'),
(5, 'مضخات'), (6, 'خزانات'), (7, 'عدد وأدوات'), (8, 'إكسسوارات وعوازل');
