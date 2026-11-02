use super::*;
use plumbing_erp::db;

fn filter(report: Report, from: &str, to: &str) -> ReportFilter {
    ReportFilter {
        report,
        from: from.into(),
        to: to.into(),
        party_id: None,
    }
}
fn report(f: &Fixture, kind: Report, from: &str, to: &str) -> Value {
    f.app
        .query(Query::Report {
            report: kind,
            from: from.into(),
            to: to.into(),
            party_id: None,
            limit: 2000,
            offset: 0,
        })
        .unwrap()
}
fn metric(v: &Value, name: &str) -> i64 {
    v["summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["metric"] == name)
        .unwrap()["amount_minor"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}
fn line(f: &Fixture, doc: i64) -> i64 {
    f.db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [doc],
            |r| r.get(0),
        )
        .unwrap()
}
fn posting(f: &Fixture, kind: DocumentKind, q: i64, price: i64, day: &str) -> i64 {
    let mut draft = f.draft(kind, q, price, 0);
    draft.date = day.into();
    let doc = f.save(draft);
    f.post(doc);
    doc
}
fn monetary_state(f: &Fixture) -> Value {
    let db = f.db();
    json!({"stock":db::rows(&db,"SELECT * FROM stock_movements ORDER BY id",[]).unwrap(),"journal":db::rows(&db,"SELECT * FROM journal_lines ORDER BY id",[]).unwrap(),"documents":db::rows(&db,"SELECT * FROM documents ORDER BY id",[]).unwrap(),"receipts":db::rows(&db,"SELECT * FROM command_receipts ORDER BY request_id",[]).unwrap()})
}

#[test]
fn balance_sheet_includes_prior_and_current_results_without_double_counting() {
    let f = Fixture::new();
    f.db().execute("INSERT INTO fiscal_periods(name,starts_on,ends_on) VALUES ('سابق','2025-01-01','2025-12-31')",[]).unwrap();
    posting(
        &f,
        DocumentKind::OpeningStock,
        10 * SCALE,
        1000,
        "2025-01-01",
    );
    posting(&f, DocumentKind::Sale, SCALE, 1500, "2025-12-31");
    posting(&f, DocumentKind::Sale, SCALE, 1500, DAY);
    let before = monetary_state(&f);
    let value = report(&f, Report::BalanceSheet, DAY, DAY);
    assert_eq!(metric(&value, "totalAssets"), 11000);
    assert_eq!(metric(&value, "totalEquity"), 11000);
    assert_eq!(metric(&value, "reconciliationDifference"), 0);
    let rows = value["rows"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r["drill_key"] == "result:prior")
            .unwrap()["balance_minor"],
        "500"
    );
    assert_eq!(
        rows.iter()
            .find(|r| r["drill_key"] == "result:current")
            .unwrap()["balance_minor"],
        "500"
    );
    let detail = f
        .app
        .query(Query::ReportDrilldown {
            filter: filter(Report::BalanceSheet, DAY, DAY),
            key: "result:prior".into(),
            limit: 100,
            offset: 0,
        })
        .unwrap();
    assert!(
        detail["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["document_date"] == "2025-12-31")
    );
    assert_eq!(before, monetary_state(&f));
}

#[test]
fn reconciliation_compares_independent_ledgers_and_highlights_document_difference() {
    let f = Fixture::new();
    f.opening();
    let clean = report(&f, Report::InventoryReconciliation, DAY, DAY);
    assert_eq!(metric(&clean, "reconciliationDifference"), 0);
    // External corruption fixture only: the production immutable guard is never removed.
    let db = f.db();
    db.execute_batch("DROP TRIGGER journal_lines_immutable_update")
        .unwrap();
    db.execute(
        "UPDATE journal_lines SET debit_minor=debit_minor-1 WHERE account_id=120",
        [],
    )
    .unwrap();
    let changed = report(&f, Report::InventoryReconciliation, DAY, DAY);
    assert_eq!(metric(&changed, "reconciliationDifference"), 1);
    let detail = f
        .app
        .query(Query::ReportDrilldown {
            filter: filter(Report::InventoryReconciliation, DAY, DAY),
            key: "reconciliation:all".into(),
            limit: 100,
            offset: 0,
        })
        .unwrap();
    assert_eq!(detail["rows"][0]["difference_minor"], "1");
    assert_eq!(detail["rows"][0]["stock_value_minor"], "10000");
    assert_eq!(detail["rows"][0]["ledger_value_minor"], "9999");
}

#[test]
fn discount_reports_keep_net_posting_and_date_returns_and_reversals() {
    let f = Fixture::new();
    f.opening();
    let mut sale = f.draft(DocumentKind::Sale, 3 * SCALE, 1001, 0);
    sale.lines[0].discount = Int(7);
    sale.discount = Int(5);
    let sale = f.save(sale);
    f.post(sale);
    assert_eq!(
        f.db()
            .query_row(
                "SELECT SUM(credit_minor) FROM journal_lines WHERE account_id=401",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2991
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM journal_lines WHERE account_id=403",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let returned = id(&f
        .app
        .mutate(request(Mutation::Return {
            source_id: Int(sale),
            date: "2026-10-28".into(),
            warehouse_id: None,
            lines: vec![ReturnLine {
                source_line_id: Int(line(&f, sale)),
                quantity: Int(3 * SCALE),
            }],
            payments: vec![],
            reason: "استرجاع".into(),
        }))
        .unwrap());
    assert_eq!(
        metric(
            &report(&f, Report::DiscountsGranted, DAY, DAY),
            "discountsGranted"
        ),
        12
    );
    assert_eq!(
        metric(
            &report(&f, Report::DiscountsGranted, DAY, "2026-10-28"),
            "discountsGranted"
        ),
        0
    );
    f.app
        .mutate(request(Mutation::Reverse {
            id: Int(returned),
            date: "2026-10-29".into(),
            reason: "إلغاء المرتجع".into(),
        }))
        .unwrap();
    let value = report(&f, Report::DiscountsGranted, DAY, "2026-10-29");
    assert_eq!(metric(&value, "discountsGranted"), 12);
    assert_eq!(value["rows"][1]["discount_minor"], "-12");
    assert_eq!(value["rows"][2]["discount_minor"], "12");
    f.reconcile();
}

#[test]
fn vat_return_reconciles_rates_returns_and_unclassified_journal_adjustments() {
    let f = Fixture::new();
    f.db()
        .execute("UPDATE store_settings SET vat_enabled=1", [])
        .unwrap();
    let receipt = posting(&f, DocumentKind::GoodsReceipt, 10 * SCALE, 1000, DAY);
    let mut purchase = f.draft(DocumentKind::Purchase, 10 * SCALE, 1000, 0);
    purchase.lines[0].tax_bps = 1400;
    purchase.lines[0].matches.push(PurchaseMatch {
        receipt_line_id: Int(line(&f, receipt)),
        base_quantity: Int(10 * SCALE),
    });
    let purchase = f.save(purchase);
    f.post(purchase);
    let mut sale = f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 0);
    sale.lines[0].tax_bps = 1400;
    let sale = f.save(sale);
    f.post(sale);
    f.app
        .mutate(request(Mutation::Journal {
            date: DAY.into(),
            opening: false,
            lines: vec![
                JournalLine {
                    account_id: Int(130),
                    party_id: None,
                    amount: Int(50),
                    due_date: None,
                    memo: String::new(),
                },
                JournalLine {
                    account_id: Int(101),
                    party_id: None,
                    amount: Int(-50),
                    due_date: None,
                    memo: String::new(),
                },
            ],
            notes: "تسوية ضريبة".into(),
        }))
        .unwrap();
    let value = report(&f, Report::VatReturn, DAY, DAY);
    assert_eq!(metric(&value, "inputVat"), 1450);
    assert_eq!(metric(&value, "outputVat"), 420);
    assert_eq!(metric(&value, "netVat"), -1030);
    let adjustment = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["tax_rate_bps"].is_null())
        .unwrap();
    assert!(adjustment["taxable_minor"].is_null());
    assert_eq!(adjustment["tax_minor"], "50");
    let details = f
        .app
        .query(Query::ReportDrilldown {
            filter: filter(Report::VatReturn, DAY, DAY),
            key: "vat:input:unclassified".into(),
            limit: 100,
            offset: 0,
        })
        .unwrap();
    assert_eq!(details["rows"].as_array().unwrap().len(), 1);
    f.app
        .mutate(request(Mutation::Return {
            source_id: Int(sale),
            date: "2026-10-28".into(),
            warehouse_id: None,
            lines: vec![ReturnLine {
                source_line_id: Int(line(&f, sale)),
                quantity: Int(SCALE),
            }],
            payments: vec![],
            reason: "مرتجع".into(),
        }))
        .unwrap();
    assert_eq!(
        metric(&report(&f, Report::VatReturn, DAY, DAY), "outputVat"),
        420
    );
    assert_eq!(
        metric(
            &report(&f, Report::VatReturn, DAY, "2026-10-28"),
            "outputVat"
        ),
        210
    );
    f.reconcile();
}

#[test]
fn collections_preserve_historical_buckets_and_last_payment_date() {
    let f = Fixture::new();
    let mut lines = vec![];
    for (amount, due) in [
        (100, "2026-10-27"),
        (200, "2026-09-26"),
        (300, "2026-08-27"),
        (400, "2026-07-28"),
    ] {
        lines.push(JournalLine {
            account_id: Int(110),
            party_id: Some(Int(f.party)),
            amount: Int(amount),
            due_date: Some(due.into()),
            memo: String::new(),
        });
    }
    lines.push(JournalLine {
        account_id: Int(303),
        party_id: None,
        amount: Int(-1000),
        due_date: None,
        memo: String::new(),
    });
    let original = id(&f
        .app
        .mutate(request(Mutation::Journal {
            date: "2026-01-01".into(),
            opening: true,
            lines,
            notes: String::new(),
        }))
        .unwrap());
    let old = report(&f, Report::Collections, DAY, DAY);
    for (key, value) in [
        ("days_0_30_minor", "100"),
        ("days_31_60_minor", "200"),
        ("days_61_90_minor", "300"),
        ("over_90_minor", "400"),
    ] {
        assert_eq!(old["rows"][0][key], value);
    }
    assert!(old["rows"][0]["last_payment_date"].is_null());
    let voucher = id(&f
        .app
        .mutate(request(Mutation::Voucher {
            date: "2026-10-28".into(),
            party_id: Int(f.party),
            supplier: false,
            outgoing: false,
            payments: vec![payment(50)],
            notes: String::new(),
        }))
        .unwrap());
    let debit=f.db().query_row("SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=110 ORDER BY id LIMIT 1",[original],|r|r.get::<_,i64>(0)).unwrap();
    let credit = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=110",
            [voucher],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    f.app
        .mutate(request(Mutation::Settle {
            date: "2026-10-28".into(),
            debit_line_id: Int(debit),
            credit_line_id: Int(credit),
            amount: Int(50),
        }))
        .unwrap();
    assert_eq!(report(&f, Report::Collections, DAY, DAY), old);
    let current = report(&f, Report::Collections, DAY, "2026-10-28");
    assert_eq!(current["rows"][0]["overdue_minor"], "950");
    assert_eq!(current["rows"][0]["last_payment_date"], "2026-10-28");
}

#[test]
fn stock_aging_and_turnover_are_exact_and_do_not_change_weighted_costing() {
    let f = Fixture::new();
    posting(
        &f,
        DocumentKind::OpeningStock,
        10 * SCALE,
        1000,
        "2026-01-01",
    );
    let old = report(&f, Report::StockAging, "2026-01-01", "2026-01-31");
    assert_eq!(old["rows"][0]["age_0_30_micros"], "10000000");
    f.app
        .mutate(request(Mutation::Adjustment {
            date: DAY.into(),
            warehouse_id: Int(1),
            product_id: Int(f.product),
            quantity: Int(5 * SCALE),
            unit_cost: Int(2000),
            reason: "استلام".into(),
        }))
        .unwrap();
    posting(&f, DocumentKind::Sale, 8 * SCALE, 1500, DAY);
    let before = monetary_state(&f);
    let aging = report(&f, Report::StockAging, DAY, DAY);
    assert_eq!(aging["rows"][0]["age_0_30_micros"], "5000000");
    assert_eq!(aging["rows"][0]["age_over_90_micros"], "2000000");
    assert_eq!(aging["rows"][0]["value_minor"], "9333");
    let turnover = report(&f, Report::StockTurnover, "2026-10-01", DAY);
    assert_eq!(turnover["rows"][0]["opening_value_minor"], "10000");
    assert_eq!(turnover["rows"][0]["closing_value_minor"], "9333");
    assert_eq!(turnover["rows"][0]["cogs_minor"], "10667");
    assert_eq!(turnover["rows"][0]["turnover_milli"], "1104");
    assert_eq!(before, monetary_state(&f));
    f.reconcile();
}

#[test]
fn all_new_reports_print_export_and_drill_down_without_financial_mutations() {
    let f = Fixture::new();
    f.opening();
    let mut sale = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    sale.lines[0].discount = Int(10);
    let sale = f.save(sale);
    f.post(sale);
    let before = monetary_state(&f);
    for (i, kind) in [
        Report::BalanceSheet,
        Report::InventoryReconciliation,
        Report::StockAbc,
        Report::StockActivity,
        Report::StockAging,
        Report::StockTurnover,
        Report::DiscountsGranted,
        Report::VatReturn,
        Report::Collections,
    ]
    .into_iter()
    .enumerate()
    {
        let to = "2026-12-31";
        let data = report(&f, kind, DAY, to);
        let printed = f
            .app
            .query(Query::PrintSnapshot {
                source: PrintSource::Report {
                    report: kind,
                    from: DAY.into(),
                    to: to.into(),
                    party_id: None,
                },
            })
            .unwrap();
        assert_eq!(printed["rows"], data["rows"]);
        assert_eq!(printed["summary"], data["summary"]);
        for row in data["rows"].as_array().unwrap() {
            let detail = f
                .app
                .query(Query::ReportDrilldown {
                    filter: filter(kind, DAY, to),
                    key: row["drill_key"].as_str().unwrap().into(),
                    limit: 100,
                    offset: 0,
                })
                .unwrap();
            assert!(detail["rows"].is_array());
        }
        for excel in [false, true] {
            let path = f
                .temp
                .path()
                .join(format!("report-{i}.{}", if excel { "xlsx" } else { "csv" }));
            f.app
                .export_report(&path, &filter(kind, DAY, to), excel)
                .unwrap();
            assert!(path.metadata().unwrap().len() > 10);
            assert!(
                f.app
                    .export_report(&path, &filter(kind, DAY, to), excel)
                    .is_err()
            );
            if excel {
                let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
                let mut xml = String::new();
                std::io::Read::read_to_string(
                    &mut zip.by_name("xl/worksheets/sheet1.xml").unwrap(),
                    &mut xml,
                )
                .unwrap();
                assert!(xml.contains("rightToLeft=\"1\""));
                assert!(!xml.contains("<f>"));
            } else {
                assert!(
                    std::fs::read(&path)
                        .unwrap()
                        .starts_with(&[0xef, 0xbb, 0xbf])
                );
            }
        }
    }
    assert_eq!(before, monetary_state(&f));
    f.reconcile();
}

#[test]
fn report_export_and_drilldown_permissions_and_row_limits_are_enforced() {
    let f = Fixture::new();
    f.opening();
    f.app
        .create_user("cashier", "أمين الصندوق", "CashierPassword12!", &[Int(2)])
        .unwrap();
    f.app.login("cashier", "CashierPassword12!").unwrap();
    let path = f.temp.path().join("forbidden.csv");
    assert_eq!(
        f.app
            .export_report(&path, &filter(Report::BalanceSheet, DAY, DAY), false)
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert!(!path.exists());
    assert!(
        f.app
            .query(Query::ReportDrilldown {
                filter: filter(Report::BalanceSheet, DAY, DAY),
                key: "account:120".into(),
                limit: 100,
                offset: 0
            })
            .is_err()
    );
    f.app.login("admin", "CorrectHorse12!").unwrap();
    let mut unsupported_filter = filter(Report::BalanceSheet, DAY, DAY);
    unsupported_filter.party_id = Some(Int(f.party));
    assert!(
        f.app
            .export_report(&path, &unsupported_filter, false)
            .is_err()
    );
    assert!(!path.exists());
    assert!(
        f.app
            .query(Query::ReportDrilldown {
                filter: filter(Report::StockAbc, DAY, DAY),
                key: "product:1 OR 1=1".into(),
                limit: 100,
                offset: 0
            })
            .is_err()
    );
    f.db().execute_batch("WITH RECURSIVE n(x) AS (SELECT 2 UNION ALL SELECT x+1 FROM n WHERE x<10001) INSERT INTO products(id,code,name_ar,normalized_name,category_id,base_unit_id) SELECT x,'LIMIT-'||x,'صنف','صنف',1,1 FROM n").unwrap();
    assert!(
        f.app
            .export_report(&path, &filter(Report::StockAbc, DAY, DAY), false)
            .is_err()
    );
    assert!(!path.exists());
    assert!(
        f.app
            .query(Query::PrintSnapshot {
                source: PrintSource::Report {
                    report: Report::StockAbc,
                    from: DAY.into(),
                    to: DAY.into(),
                    party_id: None
                }
            })
            .is_err()
    );
}

fn product(f: &Fixture, code: &str) -> i64 {
    let id = id(&f
        .app
        .mutate(request(Mutation::SaveProduct {
            product: Product {
                id: None,
                revision: Int(0),
                code: code.into(),
                name: format!("وصلة {code}"),
                category_id: Int(1),
                brand: String::new(),
                material: "PPR".into(),
                size: String::new(),
                base_unit_id: Int(1),
                standard_cost: Int(1),
                min_stock: Int(0),
                active: true,
                units: vec![ProductUnit {
                    id: None,
                    unit_id: Int(1),
                    numerator: Int(1),
                    denominator: Int(1),
                    barcode: None,
                    retail_price: Int(100),
                    wholesale_price: Int(100),
                    active: true,
                }],
            },
        }))
        .unwrap());
    f.app.query(Query::Product { id: Int(id) }).unwrap()["units"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn abc_thresholds_use_complete_period_and_activity_uses_as_of_sale_dates() {
    let f = Fixture::new();
    let day = chrono::NaiveDate::parse_from_str(DAY, "%Y-%m-%d").unwrap();
    for (code, price, age) in [("A", 80, 89), ("B", 15, 90), ("C", 5, 180)] {
        let unit = product(&f, code);
        let mut opening = f.draft(DocumentKind::OpeningStock, 10 * SCALE, 1, 0);
        opening.date = "2026-01-01".into();
        opening.lines[0].unit_id = Int(unit);
        f.post(f.save(opening));
        let mut sale = f.draft(DocumentKind::Sale, SCALE, price, 0);
        sale.date = (day - chrono::Duration::days(age)).to_string();
        sale.lines[0].unit_id = Int(unit);
        f.post(f.save(sale));
    }
    let abc = report(&f, Report::StockAbc, "2026-01-01", DAY);
    for (i, class, share) in [
        (0, "abc_a", "8000"),
        (1, "abc_b", "1500"),
        (2, "abc_c", "500"),
    ] {
        assert_eq!(abc["rows"][i]["stock_class"], class);
        assert_eq!(abc["rows"][i]["share_bps"], share);
    }
    let page = f
        .app
        .query(Query::Report {
            report: Report::StockAbc,
            from: "2026-01-01".into(),
            to: DAY.into(),
            party_id: None,
            limit: 1,
            offset: 1,
        })
        .unwrap();
    assert_eq!(page["rows"][0], abc["rows"][1]);
    assert_eq!(page["rows"][0]["cumulative_bps"], "9500");
    let activity = report(&f, Report::StockActivity, DAY, DAY);
    for (code, class) in [
        ("A", "moving_stock"),
        ("B", "slow_stock"),
        ("C", "dead_stock"),
    ] {
        assert_eq!(
            activity["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["code"] == code)
                .unwrap()["stock_activity"],
            class
        );
    }
    let historical = report(&f, Report::StockActivity, "2026-01-01", "2026-01-01");
    assert!(
        historical["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["stock_activity"] == "never_sold")
    );
    f.reconcile();
}

#[test]
fn stock_aging_keeps_negative_quantities_separate_and_receipts_cover_deficit() {
    let f = Fixture::new();
    f.db()
        .execute("UPDATE store_settings SET allow_negative_stock=1", [])
        .unwrap();
    posting(&f, DocumentKind::Sale, 2 * SCALE, 1500, DAY);
    let negative = report(&f, Report::StockAging, DAY, DAY);
    assert_eq!(negative["rows"][0]["negative_quantity_micros"], "-2000000");
    assert_eq!(negative["rows"][0]["age_0_30_micros"], "0");
    assert!(report(&f, Report::StockTurnover, DAY, DAY)["rows"][0]["turnover_milli"].is_null());
    posting(
        &f,
        DocumentKind::GoodsReceipt,
        3 * SCALE,
        1100,
        "2026-10-28",
    );
    assert_eq!(report(&f, Report::StockAging, DAY, DAY), negative);
    let received = report(&f, Report::StockAging, DAY, "2026-10-28");
    assert_eq!(received["rows"][0]["negative_quantity_micros"], "0");
    assert_eq!(received["rows"][0]["age_0_30_micros"], "1000000");
    assert_eq!(received["rows"][0]["value_minor"], "1100");
    f.reconcile();
}

#[test]
fn exports_preserve_large_decimal_strings_and_escape_spreadsheet_formulas() {
    let f = Fixture::new();
    // Each document stays below the existing posting limit; the account aggregate
    // intentionally exceeds JavaScript's exact-integer range.
    for amount in [
        1_801_439_850_948_198,
        1_801_439_850_948_198,
        1_801_439_850_948_198,
        1_801_439_850_948_198,
        1_801_439_850_948_201,
    ] {
        f.app
            .mutate(request(Mutation::Journal {
                date: DAY.into(),
                opening: true,
                lines: vec![
                    JournalLine {
                        account_id: Int(101),
                        party_id: None,
                        amount: Int(amount),
                        due_date: None,
                        memo: String::new(),
                    },
                    JournalLine {
                        account_id: Int(303),
                        party_id: None,
                        amount: Int(-amount),
                        due_date: None,
                        memo: String::new(),
                    },
                ],
                notes: "اختبار دقة التصدير".into(),
            }))
            .unwrap();
    }
    f.db()
        .execute(
            "UPDATE products SET name_ar='=1+1' WHERE id=?1",
            [f.product],
        )
        .unwrap();
    for (kind, needle) in [
        (Report::BalanceSheet, "90071992547409.93"),
        (Report::StockAbc, "=1+1"),
    ] {
        for excel in [false, true] {
            let path = f.temp.path().join(format!("exact-{kind:?}-{excel}"));
            f.app
                .export_report(&path, &filter(kind, DAY, DAY), excel)
                .unwrap();
            if excel {
                let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
                let mut xml = String::new();
                std::io::Read::read_to_string(
                    &mut zip.by_name("xl/sharedStrings.xml").unwrap(),
                    &mut xml,
                )
                .unwrap();
                assert!(xml.contains(needle));
            } else {
                let text = std::fs::read_to_string(path).unwrap();
                assert!(text.contains(needle));
                if matches!(kind, Report::StockAbc) {
                    assert!(text.contains("'=1+1"));
                }
            }
        }
    }
}

#[test]
fn discount_summary_and_export_cover_all_pages() {
    let f = Fixture::new();
    f.opening();
    for discount in [10, 20] {
        let mut sale = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
        sale.lines[0].discount = Int(discount);
        f.post(f.save(sale));
    }
    for offset in [0, 1] {
        let page = f
            .app
            .query(Query::Report {
                report: Report::DiscountsGranted,
                from: DAY.into(),
                to: DAY.into(),
                party_id: Some(Int(f.party)),
                limit: 1,
                offset,
            })
            .unwrap();
        assert_eq!(page["rows"].as_array().unwrap().len(), 1);
        assert_eq!(metric(&page, "discountsGranted"), 30);
    }
    let path = f.temp.path().join("all-pages.csv");
    let result = f
        .app
        .export_report(&path, &filter(Report::DiscountsGranted, DAY, DAY), false)
        .unwrap();
    assert_eq!(result["rows"], "2");
    let snapshot = f
        .app
        .query(Query::PrintSnapshot {
            source: PrintSource::Report {
                report: Report::DiscountsGranted,
                from: DAY.into(),
                to: DAY.into(),
                party_id: Some(Int(f.party)),
            },
        })
        .unwrap();
    assert_eq!(snapshot["rows"].as_array().unwrap().len(), 2);
    assert_eq!(metric(&snapshot, "discountsGranted"), 30);
}
