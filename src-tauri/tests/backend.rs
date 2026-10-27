use plumbing_erp::{models::arithmetic::*, models::*, services::costing::Pool, state::App};
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;

const DAY: &str = "2026-10-27";
struct Fixture {
    app: App,
    temp: TempDir,
    product: i64,
    unit: i64,
    party: i64,
}
fn id(value: &Value) -> i64 {
    value["id"].as_str().unwrap().parse().unwrap()
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app = App::open(temp.path().join("store")).unwrap();
        app.setup(Setup {
            store_name: "متجر الاختبار".into(),
            username: "admin".into(),
            display_name: "المدير".into(),
            password: "CorrectHorse12!".into(),
            fiscal_year: 2026,
            demo: false,
        })
        .unwrap();
        app.login("admin", "CorrectHorse12!").unwrap();
        let product = id(&app
            .mutate(request(Mutation::SaveProduct {
                product: Product {
                    id: None,
                    revision: Int(0),
                    code: "PPR20".into(),
                    name: "ماسورة مياه".into(),
                    category_id: Int(1),
                    brand: "محلي".into(),
                    material: "PPR".into(),
                    size: "20 مم".into(),
                    base_unit_id: Int(1),
                    standard_cost: Int(1000),
                    min_stock: Int(5_000_000),
                    active: true,
                    units: vec![ProductUnit {
                        id: None,
                        unit_id: Int(1),
                        numerator: Int(1),
                        denominator: Int(1),
                        barcode: Some("6220000000015".into()),
                        retail_price: Int(1500),
                        wholesale_price: Int(1400),
                        active: true,
                    }],
                },
            }))
            .unwrap());
        let party = id(&app
            .mutate(request(Mutation::SaveParty {
                party: Party {
                    id: None,
                    code: "P1".into(),
                    name: "طرف الاختبار".into(),
                    customer: true,
                    supplier: true,
                    phone: String::new(),
                    address: String::new(),
                    tax_number: String::new(),
                    credit_limit: None,
                    terms_days: 30,
                    active: true,
                    notes: String::new(),
                },
            }))
            .unwrap());
        let unit = app.query(Query::Product { id: Int(product) }).unwrap()["units"][0]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        Self {
            app,
            temp,
            product,
            unit,
            party,
        }
    }
    fn db(&self) -> Connection {
        Connection::open(self.temp.path().join("store/store.db")).unwrap()
    }
    fn draft(&self, kind: DocumentKind, q: i64, price: i64, pay: i64) -> Draft {
        Draft {
            id: None,
            revision: Int(0),
            kind,
            date: DAY.into(),
            due_date: None,
            party_id: if matches!(kind, DocumentKind::OpeningStock | DocumentKind::Transfer) {
                None
            } else {
                Some(Int(self.party))
            },
            warehouse_id: Some(Int(1)),
            destination_id: None,
            source_id: None,
            wholesale: false,
            discount: Int(0),
            lines: vec![DraftLine {
                unit_id: Int(self.unit),
                quantity: Int(q),
                price: Int(price),
                discount: Int(0),
                tax_bps: 0,
                order_line_id: None,
                damaged: Int(0),
                short: Int(0),
                matches: vec![],
                notes: String::new(),
            }],
            payments: if pay > 0 { vec![payment(pay)] } else { vec![] },
            notes: String::new(),
            reason: "اختبار".into(),
        }
    }
    fn save(&self, draft: Draft) -> i64 {
        id(&self
            .app
            .mutate(request(Mutation::SaveDraft { draft }))
            .unwrap())
    }
    fn post(&self, doc: i64) -> Value {
        let revision: i64 = self
            .db()
            .query_row("SELECT revision FROM documents WHERE id=?1", [doc], |r| {
                r.get(0)
            })
            .unwrap();
        self.app
            .mutate(request(Mutation::DocumentAction {
                id: Int(doc),
                revision: Int(revision),
                action: DocumentAction::Post,
                reason: String::new(),
                acknowledge_credit: false,
            }))
            .unwrap()
    }
    fn opening(&self) {
        let doc = self.save(self.draft(DocumentKind::OpeningStock, 10_000_000, 1000, 0));
        self.post(doc);
    }
    fn quantity_value(&self) -> (i64, i64) {
        self.db().query_row("SELECT quantity_micros,value_minor FROM v_stock_on_hand WHERE product_id=?1 AND warehouse_id=1",[self.product],|r|Ok((r.get(0)?,r.get(1)?))).unwrap()
    }
    fn reconcile(&self) {
        let db = self.db();
        let balance: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(debit_minor-credit_minor),0) FROM v_journal",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(balance, 0);
        let stock: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(value_minor),0) FROM v_stock_on_hand",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let ledger:i64=db.query_row("SELECT COALESCE(SUM(debit_minor-credit_minor),0) FROM v_journal WHERE control_kind='inventory'",[],|r|r.get(0)).unwrap();
        assert_eq!(stock, ledger);
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM documents WHERE status='posting'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
}
fn request(mutation: Mutation) -> Request {
    Request {
        request_id: uuid::Uuid::new_v4().to_string(),
        mutation,
    }
}
fn payment(amount: i64) -> Payment {
    Payment {
        method: PaymentMethod::Cash,
        cash_account_id: Some(Int(1)),
        shift_id: None,
        amount: Int(amount),
        reference: String::new(),
        cheque: None,
    }
}

#[test]
fn weighted_average_depletion_and_fractional_quantities() {
    let mut pool = Pool {
        quantity: 10 * SCALE,
        value: 10000,
    };
    assert_eq!(pool.receive(5 * SCALE, 6000).unwrap(), (6000, 0));
    assert_eq!(pool.issue(3 * SCALE, 0, false).unwrap(), 3200);
    assert_eq!(
        pool,
        Pool {
            quantity: 12 * SCALE,
            value: 12800
        }
    );
    assert_eq!(pool.issue(250_000, 0, false).unwrap(), 267);
    let remaining = pool.value;
    assert_eq!(pool.issue(pool.quantity, 0, false).unwrap(), remaining);
    assert_eq!(pool, Pool::default());
}
#[test]
fn negative_stock_is_reconciled_at_actual_receipt_cost() {
    let mut p = Pool::default();
    assert!(p.issue(5 * SCALE, 1000, false).is_err());
    assert_eq!(p.issue(5 * SCALE, 1000, true).unwrap(), 5000);
    assert_eq!(p.receive(10 * SCALE, 12000).unwrap(), (11000, 1000));
    assert_eq!(
        p,
        Pool {
            quantity: 5 * SCALE,
            value: 6000
        }
    );
}
#[test]
fn allocations_conserve_every_piastre_and_parse_arabic_digits() {
    for amount in 0..150 {
        for n in 1..20 {
            let weights: Vec<i64> = (1..=n).collect();
            let shares = allocate(amount, &weights).unwrap();
            assert_eq!(shares.iter().sum::<i64>(), amount);
        }
    }
    assert_eq!(allocate(2, &[1, 1, 1]).unwrap(), vec![1, 1, 0]);
    assert_eq!(parse_decimal("١٢٫٣٤", 2).unwrap(), 1234);
    assert_eq!(parse_decimal("-0.25", 6).unwrap(), -250000);
    assert!(parse_decimal("1.001", 2).is_err());
    assert!(convert(1, 1, 3).is_err());
    assert!(parse_decimal("99999999999999999999999999999999999.99", 2).is_err());
    assert!(serde_json::from_value::<Int>(json!(9007199254740993_u64)).is_err());
    assert_eq!(
        serde_json::to_value(Int(i64::MAX)).unwrap(),
        json!(i64::MAX.to_string())
    );
}
#[test]
fn posting_is_idempotent_and_balanced() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 1000));
    let request = request(Mutation::DocumentAction {
        id: Int(sale),
        revision: Int(1),
        action: DocumentAction::Post,
        reason: String::new(),
        acknowledge_credit: false,
    });
    let first = f.app.mutate(request.clone()).unwrap();
    assert_eq!(f.app.mutate(request).unwrap(), first);
    assert_eq!(f.quantity_value(), (8 * SCALE, 8000));
    let due: i64 = f
        .db()
        .query_row(
            "SELECT SUM(outstanding_signed_minor) FROM v_party_open_items",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(due, 2000);
    f.reconcile();
}
#[test]
fn failed_posting_rolls_back_and_remains_an_editable_draft() {
    let f = Fixture::new();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    let result = f.app.mutate(request(Mutation::DocumentAction {
        id: Int(sale),
        revision: Int(1),
        action: DocumentAction::Post,
        reason: String::new(),
        acknowledge_credit: false,
    }));
    assert!(result.is_err());
    assert_eq!(
        f.db()
            .query_row("SELECT status FROM documents WHERE id=?1", [sale], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        "draft"
    );
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM stock_movements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    f.reconcile();
}
#[test]
fn linked_partial_returns_cap_quantity_and_original_cost() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 3000));
    f.post(sale);
    let line: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    let mutation = Mutation::Return {
        source_id: Int(sale),
        date: DAY.into(),
        warehouse_id: None,
        lines: vec![ReturnLine {
            source_line_id: Int(line),
            quantity: Int(SCALE),
        }],
        payments: vec![payment(1500)],
        reason: "استرجاع قطعة".into(),
    };
    f.app.mutate(request(mutation.clone())).unwrap();
    assert_eq!(f.quantity_value(), (9 * SCALE, 9000));
    f.app.mutate(request(mutation.clone())).unwrap();
    assert!(f.app.mutate(request(mutation)).is_err());
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    f.reconcile();
}
#[test]
fn goods_receipt_purchase_invoice_and_late_costs_do_not_duplicate_stock() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 1000, 0));
    f.post(receipt);
    let received: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get(0),
        )
        .unwrap();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 3000));
    f.post(sale);
    let mut invoice = f.draft(DocumentKind::Purchase, 10 * SCALE, 1100, 0);
    invoice.lines[0].matches.push(PurchaseMatch {
        receipt_line_id: Int(received),
        base_quantity: Int(10 * SCALE),
    });
    let invoice = f.save(invoice);
    f.post(invoice);
    assert_eq!(f.quantity_value(), (8 * SCALE, 8800));
    f.app
        .mutate(request(Mutation::LandedCost {
            date: DAY.into(),
            supplier_id: Int(f.party),
            description: "شحن".into(),
            shares: vec![CostShare {
                receipt_line_id: Int(received),
                amount: Int(500),
            }],
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (8 * SCALE, 9200));
    f.reconcile();
    let cogs: i64 = f
        .db()
        .query_row(
            "SELECT SUM(debit_minor-credit_minor) FROM v_journal WHERE account_id=501",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cogs, 2300);
}
#[test]
fn stocktake_rejects_movement_after_snapshot() {
    let f = Fixture::new();
    f.opening();
    let count = id(&f
        .app
        .mutate(request(Mutation::StartStocktake {
            date: DAY.into(),
            warehouse_id: Int(1),
            product_ids: vec![Int(f.product)],
            reason: "جرد".into(),
        }))
        .unwrap());
    f.app
        .mutate(request(Mutation::CountStocktake {
            id: Int(count),
            revision: Int(0),
            lines: vec![CountLine {
                product_id: Int(f.product),
                quantity: Int(9 * SCALE),
                reason: "عجز".into(),
            }],
        }))
        .unwrap();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.post(sale);
    assert!(
        f.app
            .mutate(request(Mutation::DocumentAction {
                id: Int(count),
                revision: Int(1),
                action: DocumentAction::Post,
                reason: String::new(),
                acknowledge_credit: false
            }))
            .is_err()
    );
    assert_eq!(f.quantity_value(), (9 * SCALE, 9000));
    f.reconcile();
}
#[test]
fn permissions_failed_login_and_last_admin_are_enforced() {
    let f = Fixture::new();
    let user = id(&f
        .app
        .create_user("cashier", "أمين الصندوق", "CashierPassword12!", &[Int(2)])
        .unwrap());
    assert!(
        f.app
            .mutate(request(Mutation::SetUserAccess {
                user_id: Int(1),
                active: false,
                roles: vec![Int(2)]
            }))
            .is_err()
    );
    f.app.logout().unwrap();
    assert!(f.app.login("cashier", "wrong").is_err());
    assert_eq!(
        f.db()
            .query_row(
                "SELECT failed_login_count FROM users WHERE id=?1",
                [user],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    f.app.login("cashier", "CashierPassword12!").unwrap();
    assert!(
        f.app
            .mutate(request(Mutation::Journal {
                date: DAY.into(),
                opening: false,
                lines: vec![],
                notes: String::new()
            }))
            .is_err()
    );
    assert!(f.app.query(Query::Users).is_err());
}
#[test]
fn reversal_restores_stock_and_clears_party_open_items() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 1000));
    f.post(sale);
    f.app
        .mutate(request(Mutation::Reverse {
            id: Int(sale),
            date: DAY.into(),
            reason: "فاتورة مكررة".into(),
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    let outstanding: i64 = f
        .db()
        .query_row(
            "SELECT COALESCE(SUM(abs(outstanding_signed_minor)),0) FROM v_party_open_items",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outstanding, 0);
    f.reconcile();
}
#[test]
fn reports_and_dashboard_use_integer_strings() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 0));
    f.post(sale);
    for report in [
        Report::TrialBalance,
        Report::IncomeStatement,
        Report::CashFlow,
        Report::ProductProfit,
        Report::CategoryProfit,
        Report::CustomerProfit,
        Report::PartyStatement,
        Report::Receivables,
        Report::Payables,
        Report::Aging,
        Report::Vat,
        Report::LowStock,
        Report::Valuation,
        Report::Cheques,
        Report::PurchaseOrders,
        Report::OpenItems,
        Report::CashShifts,
    ] {
        f.app
            .query(Query::Report {
                report,
                from: "2026-01-01".into(),
                to: DAY.into(),
                party_id: Some(Int(f.party)),
                limit: 100,
                offset: 0,
            })
            .unwrap();
    }
    let dashboard = f.app.query(Query::Dashboard { date: DAY.into() }).unwrap();
    assert_eq!(dashboard["sales"][0]["today_sales_minor"], "3000");
    let search = f
        .app
        .query(Query::Products {
            search: "ماسورة".into(),
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(search.as_array().unwrap().len(), 1);
}
#[test]
fn backup_restore_and_existing_backup_protection() {
    let f = Fixture::new();
    f.opening();
    let backup = f.temp.path().join("backup.erpbackup");
    f.app.backup(&backup).unwrap();
    let original = std::fs::read(&backup).unwrap();
    assert!(f.app.backup(&backup).is_err());
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.post(sale);
    f.app.restore(&backup).unwrap();
    f.app.login("admin", "CorrectHorse12!").unwrap();
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    f.reconcile();
}
#[test]
fn csv_and_excel_roundtrip_preserve_units_and_integer_prices() {
    let f = Fixture::new();
    for excel in [false, true] {
        let path = f.temp.path().join(if excel {
            "products.xlsx"
        } else {
            "products.csv"
        });
        f.app.export_products(&path, excel).unwrap();
        let preview = f.app.preview_products(&path).unwrap();
        assert_eq!(preview["count"], "1");
        assert_eq!(preview["preview"][0]["units"][0]["retail_price"], "1500");
        f.app
            .import_products(preview["token"].as_str().unwrap(), true)
            .unwrap();
    }
}
#[test]
fn optional_demo_contains_one_hundred_unique_arabic_products() {
    let temp = tempfile::tempdir().unwrap();
    let app = App::open(temp.path().join("store")).unwrap();
    app.setup(Setup {
        store_name: "تجريبي".into(),
        username: "admin".into(),
        display_name: "مدير".into(),
        password: "CorrectHorse12!".into(),
        fiscal_year: 2026,
        demo: true,
    })
    .unwrap();
    let db = Connection::open(temp.path().join("store/store.db")).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM products", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        100
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(DISTINCT barcode) FROM product_units",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        100
    );
}
#[test]
fn locked_period_rejects_posting_and_applied_migration_tampering() {
    let f = Fixture::new();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.app
        .mutate(request(Mutation::FiscalPeriod {
            id: Some(Int(1)),
            name: "2026".into(),
            starts_on: "2026-01-01".into(),
            ends_on: "2026-12-31".into(),
            locked: true,
            reason: "إغلاق".into(),
        }))
        .unwrap();
    assert!(
        f.app
            .mutate(request(Mutation::DocumentAction {
                id: Int(sale),
                revision: Int(1),
                action: DocumentAction::Post,
                reason: String::new(),
                acknowledge_credit: false
            }))
            .is_err()
    );
    f.db()
        .execute(
            "UPDATE schema_migrations SET sha256=?1 WHERE version=1",
            ["0".repeat(64)],
        )
        .unwrap();
    assert!(App::open(f.temp.path().join("store")).is_err());
}

#[test]
fn returns_after_late_cost_restore_adjusted_original_cogs() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 1000, 0));
    f.post(receipt);
    let receipt_line: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get(0),
        )
        .unwrap();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 3000));
    f.post(sale);
    f.app
        .mutate(request(Mutation::LandedCost {
            date: DAY.into(),
            supplier_id: Int(f.party),
            description: "شحن متأخر".into(),
            shares: vec![CostShare {
                receipt_line_id: Int(receipt_line),
                amount: Int(1000),
            }],
        }))
        .unwrap();
    let source: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    f.app
        .mutate(request(Mutation::Return {
            source_id: Int(sale),
            date: DAY.into(),
            warehouse_id: None,
            lines: vec![ReturnLine {
                source_line_id: Int(source),
                quantity: Int(2 * SCALE),
            }],
            payments: vec![payment(3000)],
            reason: "مرتجع بعد الشحن".into(),
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (10 * SCALE, 11000));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT SUM(debit_minor-credit_minor) FROM v_journal WHERE account_id=501",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    f.reconcile();
}

#[test]
fn aging_buckets_and_future_settlements_are_historically_correct() {
    let f = Fixture::new();
    let day = chrono::NaiveDate::parse_from_str(DAY, "%Y-%m-%d").unwrap();
    for overdue in [-1, 0, 30, 31, 60, 61, 90, 91] {
        f.app
            .mutate(request(Mutation::Journal {
                date: "2026-01-01".into(),
                opening: true,
                lines: vec![
                    JournalLine {
                        account_id: Int(110),
                        party_id: Some(Int(f.party)),
                        due_date: Some((day - chrono::Duration::days(overdue)).to_string()),
                        amount: Int(100),
                        memo: String::new(),
                    },
                    JournalLine {
                        account_id: Int(303),
                        party_id: None,
                        due_date: None,
                        amount: Int(-100),
                        memo: String::new(),
                    },
                ],
                notes: String::new(),
            }))
            .unwrap();
    }
    let result = f
        .app
        .query(Query::Report {
            report: Report::Aging,
            from: "2026-01-01".into(),
            to: DAY.into(),
            party_id: Some(Int(f.party)),
            limit: 100,
            offset: 0,
        })
        .unwrap();
    let row = &result[0];
    for (key, value) in [
        ("not_due_minor", "100"),
        ("days_0_30_minor", "200"),
        ("days_31_60_minor", "200"),
        ("days_61_90_minor", "200"),
        ("over_90_minor", "100"),
        ("net_due_minor", "800"),
    ] {
        assert_eq!(row[key], value);
    }
    let debit: i64 = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE account_id=110 ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let voucher = id(&f
        .app
        .mutate(request(Mutation::Voucher {
            date: "2026-10-28".into(),
            party_id: Int(f.party),
            supplier: false,
            outgoing: false,
            payments: vec![payment(100)],
            notes: String::new(),
        }))
        .unwrap());
    let credit: i64 = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=110",
            [voucher],
            |r| r.get(0),
        )
        .unwrap();
    f.app
        .mutate(request(Mutation::Settle {
            date: "2026-10-28".into(),
            debit_line_id: Int(debit),
            credit_line_id: Int(credit),
            amount: Int(100),
        }))
        .unwrap();
    for (as_of, total) in [(DAY, "800"), ("2026-10-28", "700")] {
        let rows = f
            .app
            .query(Query::Report {
                report: Report::Aging,
                from: "2026-01-01".into(),
                to: as_of.into(),
                party_id: Some(Int(f.party)),
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(rows[0]["net_due_minor"], total);
    }
}

#[test]
fn cheque_clear_then_bounce_restores_customer_debt() {
    let f = Fixture::new();
    f.opening();
    let mut draft = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    draft.payments = vec![
        payment(500),
        Payment {
            method: PaymentMethod::Cheque,
            cash_account_id: None,
            shift_id: None,
            amount: Int(1000),
            reference: String::new(),
            cheque: Some(ChequeInput {
                number: "CH-1".into(),
                bank: "بنك الاختبار".into(),
                due_date: "2026-11-01".into(),
            }),
        },
    ];
    let sale = f.save(draft);
    f.post(sale);
    let cheque: i64 = f
        .db()
        .query_row("SELECT id FROM cheques", [], |r| r.get(0))
        .unwrap();
    f.app
        .mutate(request(Mutation::ChequeEvent {
            date: DAY.into(),
            cheque_id: Int(cheque),
            cleared: true,
            bank_account_id: Some(Int(2)),
            reason: "تحصيل".into(),
        }))
        .unwrap();
    f.app
        .mutate(request(Mutation::ChequeEvent {
            date: DAY.into(),
            cheque_id: Int(cheque),
            cleared: false,
            bank_account_id: None,
            reason: "ارتداد بنكي".into(),
        }))
        .unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT receivable_minor FROM v_party_balances WHERE party_id=?1",
                [f.party],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1000
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT balance_minor FROM v_cash_balances WHERE cash_account_id=2",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    f.reconcile();
}

#[test]
fn opening_imports_are_atomic_and_support_fractional_quantities() {
    let f = Fixture::new();
    let stock = f.temp.path().join("stock.csv");
    std::fs::write(
        &stock,
        "product_code,unit_id,quantity,unit_cost\nPPR20,1,2.5,10.00\n",
    )
    .unwrap();
    let preview = f.app.preview_opening(&stock, false).unwrap();
    f.app
        .import_opening(preview["token"].as_str().unwrap(), false, DAY, Some(Int(1)))
        .unwrap();
    assert_eq!(f.quantity_value(), (2_500_000, 2500));
    let balances = f.temp.path().join("balances.csv");
    std::fs::write(
        &balances,
        "account_code,party_code,amount,due_date\n1200,P1,100.25,2026-11-01\n",
    )
    .unwrap();
    let preview = f.app.preview_opening(&balances, true).unwrap();
    f.app
        .import_opening(preview["token"].as_str().unwrap(), true, DAY, None)
        .unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT receivable_minor FROM v_party_balances WHERE party_id=?1",
                [f.party],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        10025
    );
    f.reconcile();
}

#[test]
fn purchase_return_uses_current_average_and_records_variance() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 1000, 0));
    f.post(receipt);
    let received: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get(0),
        )
        .unwrap();
    let mut invoice = f.draft(DocumentKind::Purchase, 10 * SCALE, 1000, 0);
    invoice.lines[0].matches = vec![PurchaseMatch {
        receipt_line_id: Int(received),
        base_quantity: Int(10 * SCALE),
    }];
    let invoice = f.save(invoice);
    f.post(invoice);
    let source: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [invoice],
            |r| r.get(0),
        )
        .unwrap();
    f.app
        .mutate(request(Mutation::Return {
            source_id: Int(invoice),
            date: DAY.into(),
            warehouse_id: Some(Int(1)),
            lines: vec![ReturnLine {
                source_line_id: Int(source),
                quantity: Int(2 * SCALE),
            }],
            payments: vec![],
            reason: "رد للمورد".into(),
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (8 * SCALE, 8000));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT payable_minor FROM v_party_balances WHERE party_id=?1",
                [f.party],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        8000
    );
    f.reconcile();
}

#[test]
fn credit_warning_and_cash_shift_close_are_explicit() {
    let f = Fixture::new();
    f.opening();
    f.db()
        .execute(
            "UPDATE parties SET credit_limit_minor=1000 WHERE id=?1",
            [f.party],
        )
        .unwrap();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 0));
    let mut m = Mutation::DocumentAction {
        id: Int(sale),
        revision: Int(1),
        action: DocumentAction::Post,
        reason: String::new(),
        acknowledge_credit: false,
    };
    assert!(f.app.mutate(request(m.clone())).is_err());
    if let Mutation::DocumentAction {
        acknowledge_credit, ..
    } = &mut m
    {
        *acknowledge_credit = true;
    }
    f.app.mutate(request(m)).unwrap();
    let shift = id(&f
        .app
        .mutate(request(Mutation::OpenShift {
            cash_account_id: Int(1),
        }))
        .unwrap());
    f.app
        .mutate(request(Mutation::Voucher {
            date: DAY.into(),
            party_id: Int(f.party),
            supplier: false,
            outgoing: false,
            payments: vec![payment(500)],
            notes: String::new(),
        }))
        .unwrap();
    f.app
        .mutate(request(Mutation::CloseShift {
            id: Int(shift),
            date: DAY.into(),
            counted: Int(490),
            reason: "عجز عشرة قروش".into(),
        }))
        .unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT balance_minor FROM v_cash_balances WHERE cash_account_id=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        490
    );
    f.reconcile();
}

#[test]
fn invalid_restore_does_not_change_live_store_and_daily_backup_is_once_per_day() {
    let f = Fixture::new();
    f.opening();
    f.app.daily_backup().unwrap();
    f.app.daily_backup().unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM backup_runs WHERE kind='automatic' AND status='verified'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let bad = f.temp.path().join("bad.erpbackup");
    std::fs::write(&bad, b"not an archive").unwrap();
    assert!(f.app.restore(&bad).is_err());
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    f.reconcile();
}

#[test]
fn damaged_store_recovery_requires_backup_admin_and_preserves_original() {
    let f = Fixture::new();
    f.opening();
    let backup = f.temp.path().join("recovery.erpbackup");
    f.app.backup(&backup).unwrap();
    let store = f.temp.path().join("store");
    drop(f.app);
    let db = Connection::open(store.join("store.db")).unwrap();
    db.execute(
        "UPDATE schema_migrations SET sha256=?1 WHERE version=1",
        ["f".repeat(64)],
    )
    .unwrap();
    drop(db);
    let recovery = App::open_with_recovery(&store).unwrap();
    assert_eq!(
        recovery.query(Query::Bootstrap).unwrap()["recovery_required"],
        true
    );
    assert!(recovery.recover_backup(&backup, "admin", "wrong").is_err());
    let result = recovery
        .recover_backup(&backup, "admin", "CorrectHorse12!")
        .unwrap();
    assert!(
        std::path::Path::new(result["preserved_store"].as_str().unwrap())
            .join("store.db")
            .exists()
    );
    recovery.login("admin", "CorrectHorse12!").unwrap();
    assert_eq!(
        recovery
            .query(Query::Inventory {
                warehouse_id: Some(Int(1)),
                after_id: Int(0),
                limit: 50
            })
            .unwrap()[0]["quantity_micros"],
        "10000000"
    );
}

#[test]
fn password_reset_revokes_sessions_and_requires_change_before_writes() {
    let f = Fixture::new();
    let user = id(&f
        .app
        .create_user("cashier", "كاشير", "InitialPassword1!", &[Int(2)])
        .unwrap());
    f.app
        .reset_password(Int(user), "TemporaryPassword1!")
        .unwrap();
    f.app.login("cashier", "TemporaryPassword1!").unwrap();
    assert_eq!(
        f.app.query(Query::Bootstrap).unwrap()["must_change_password"],
        true
    );
    assert!(
        f.app
            .mutate(request(Mutation::OpenShift {
                cash_account_id: Int(1)
            }))
            .is_err()
    );
    f.app
        .change_password("TemporaryPassword1!", "NewSecurePassword1!")
        .unwrap();
    f.app.login("cashier", "NewSecurePassword1!").unwrap();
    f.app
        .mutate(request(Mutation::OpenShift {
            cash_account_id: Int(1),
        }))
        .unwrap();
}

#[test]
fn negative_destination_transfer_corrects_provisional_cost_without_losing_value() {
    let f = Fixture::new();
    f.opening();
    let second = id(&f
        .app
        .mutate(request(Mutation::SaveMaster {
            master: Master::Warehouse {
                id: None,
                name: "المخزن الثاني".into(),
                code: "SECOND".into(),
                address: String::new(),
                active: true,
            },
        }))
        .unwrap());
    f.db()
        .execute("UPDATE store_settings SET allow_negative_stock=1", [])
        .unwrap();
    f.db()
        .execute(
            "UPDATE products SET standard_cost_minor=800 WHERE id=?1",
            [f.product],
        )
        .unwrap();
    let mut sale = f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 3000);
    sale.warehouse_id = Some(Int(second));
    let sale = f.save(sale);
    f.post(sale);
    let mut transfer = f.draft(DocumentKind::Transfer, 5 * SCALE, 0, 0);
    transfer.destination_id = Some(Int(second));
    let transfer = f.save(transfer);
    f.post(transfer);
    assert_eq!(
        f.db()
            .query_row(
                "SELECT quantity_micros,value_minor FROM v_stock_on_hand WHERE warehouse_id=?1",
                [second],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            )
            .unwrap(),
        (3 * SCALE, 3000)
    );
    f.reconcile();
}

#[test]
fn interrupted_restore_rolls_back_before_creating_any_new_database() {
    let f = Fixture::new();
    f.opening();
    let root = f.temp.path().join("store");
    drop(f.app);
    let previous = f
        .temp
        .path()
        .join(format!("previous-{}", uuid::Uuid::new_v4()));
    let staging = f
        .temp
        .path()
        .join(format!("restore-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging).unwrap();
    std::fs::write(
        f.temp.path().join(".restore-store.json"),
        serde_json::to_vec(&json!({"previous":previous,"staging":staging})).unwrap(),
    )
    .unwrap();
    std::fs::rename(&root, &previous).unwrap();
    let app = App::open(&root).unwrap();
    app.login("admin", "CorrectHorse12!").unwrap();
    assert_eq!(
        app.query(Query::Inventory {
            warehouse_id: Some(Int(1)),
            after_id: Int(0),
            limit: 50
        })
        .unwrap()[0]["quantity_micros"],
        "10000000"
    );
    assert!(!f.temp.path().join(".restore-store.json").exists());
}

#[test]
fn migration_keeps_a_verified_backup_with_the_original_schema_version() {
    use plumbing_erp::db::{MIGRATIONS, digest};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let mut old = Connection::open(root.join("store.db")).unwrap();
    old.execute_batch(include_str!("../db/connection.sql"))
        .unwrap();
    for (index, (name, sql)) in MIGRATIONS.iter().take(7).enumerate() {
        let tx = old.transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        tx.execute(
            "INSERT INTO schema_migrations(version,name,sha256) VALUES (?1,?2,?3)",
            rusqlite::params![(index + 1) as i64, name, digest(sql.as_bytes())],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    drop(old);
    let app = App::open(&root).unwrap();
    let path = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "erpbackup"))
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let manifest: Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["schema"], 7);
    let preserved = temp.path().join("preserved.db");
    std::io::copy(
        &mut archive.by_name("store.db").unwrap(),
        &mut std::fs::File::create(&preserved).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest["files"]["store.db"],
        digest(&std::fs::read(&preserved).unwrap())
    );
    let original = Connection::open(preserved).unwrap();
    assert_eq!(
        original
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        7
    );
    let current = Connection::open(root.join("store.db")).unwrap();
    assert_eq!(
        current
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        MIGRATIONS.len() as i64
    );
    assert_eq!(app.query(Query::Bootstrap).unwrap()["setup_required"], true);
}
