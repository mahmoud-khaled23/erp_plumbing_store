use super::*;
use plumbing_erp::db;

fn number(v: &Value, key: &str) -> i64 {
    v[key].as_str().unwrap().parse().unwrap()
}
fn state(f: &Fixture) -> Value {
    let d = f.db();
    json!({"stock":db::rows(&d,"SELECT * FROM stock_movements ORDER BY id",[]).unwrap(),"balances":db::rows(&d,"SELECT * FROM stock_balances ORDER BY product_id,warehouse_id",[]).unwrap(),"journals":db::rows(&d,"SELECT * FROM journal_lines ORDER BY id",[]).unwrap(),"audit":db::rows(&d,"SELECT * FROM audit_log ORDER BY id",[]).unwrap(),"receipts":db::rows(&d,"SELECT * FROM command_receipts ORDER BY request_id",[]).unwrap()})
}
fn set_warranty(f: &Fixture, days: Option<u16>) {
    let revision = f
        .db()
        .query_row(
            "SELECT revision FROM products WHERE id=?1",
            [f.product],
            |r| r.get(0),
        )
        .unwrap();
    f.app
        .mutate(request(Mutation::SetWarranty {
            product_id: Int(f.product),
            revision: Int(revision),
            days,
            reason: "شروط ضمان المورد".into(),
        }))
        .unwrap();
}
fn variants() -> Vec<VariantInput> {
    vec![
        VariantInput {
            code: "VAR-20".into(),
            size: "20 مم".into(),
            material: "نحاس".into(),
            barcode: Some("VAR-20-CODE".into()),
        },
        VariantInput {
            code: "VAR-25".into(),
            size: "25 مم".into(),
            material: "PPR".into(),
            barcode: None,
        },
    ]
}
fn variant_request(f: &Fixture, vs: Vec<VariantInput>) -> Request {
    let p = f
        .app
        .query(Query::VariantPreview {
            template_id: Int(f.product),
            variants: vs.clone(),
        })
        .unwrap();
    request(Mutation::CreateVariants {
        template_id: Int(f.product),
        variants: vs,
        preview_hash: p["preview_hash"].as_str().unwrap().into(),
        reason: "مقاسات القالب".into(),
    })
}
fn bundle(f: &Fixture, items: Vec<BundleItem>) -> i64 {
    id(&f
        .app
        .mutate(request(Mutation::SaveBundle {
            bundle: SalesBundle {
                id: None,
                revision: Int(0),
                code: "KIT".into(),
                name: "طقم تركيب".into(),
                active: true,
                items,
            },
            reason: "مكونات التركيب".into(),
        }))
        .unwrap())
}
fn item(unit: i64, qty: i64) -> BundleItem {
    BundleItem {
        unit_id: Int(unit),
        quantity: Int(qty),
    }
}
fn expanded_draft(f: &Fixture, b: i64, qty: i64) -> Draft {
    let p = f
        .app
        .query(Query::ExpandBundle {
            id: Int(b),
            quantity: Int(qty),
            wholesale: false,
        })
        .unwrap();
    let mut d = f.draft(DocumentKind::Sale, SCALE, 0, 0);
    let template = d.lines[0].clone();
    d.lines = p["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| DraftLine {
            unit_id: Int(number(r, "unit_id")),
            quantity: Int(number(r, "quantity_micros")),
            price: Int(number(r, "unit_price_minor")),
            notes: format!("مكون طقم: {}", r["bundle_name"].as_str().unwrap()),
            ..template.clone()
        })
        .collect();
    d
}
fn post_request(f: &Fixture, doc: i64) -> Request {
    let rev = f
        .db()
        .query_row("SELECT revision FROM documents WHERE id=?1", [doc], |r| {
            r.get(0)
        })
        .unwrap();
    request(Mutation::DocumentAction {
        id: Int(doc),
        revision: Int(rev),
        action: DocumentAction::Post,
        reason: String::new(),
        acknowledge_credit: false,
    })
}
fn reverse(f: &Fixture, doc: i64) -> Value {
    f.app
        .mutate(request(Mutation::Reverse {
            id: Int(doc),
            date: DAY.into(),
            reason: "تصحيح".into(),
        }))
        .unwrap()
}
fn return_lines(f: &Fixture, doc: i64, lines: Vec<ReturnLine>) -> Value {
    f.app
        .mutate(request(Mutation::Return {
            source_id: Int(doc),
            date: DAY.into(),
            warehouse_id: Some(Int(1)),
            lines,
            payments: vec![],
            reason: "رد مكونات".into(),
        }))
        .unwrap()
}
fn quarantine(f: &Fixture) -> i64 {
    let w = id(&f
        .app
        .mutate(request(Mutation::SaveMaster {
            master: Master::Warehouse {
                id: None,
                code: "Q".into(),
                name: "عزل وفحص".into(),
                address: String::new(),
                active: true,
            },
        }))
        .unwrap());
    f.app
        .mutate(request(Mutation::SetQuarantineWarehouse {
            warehouse_id: Int(w),
            revision: Int(0),
            quarantine: true,
            reason: "مخزن فحص التالف".into(),
        }))
        .unwrap();
    w
}
fn transfer(f: &Fixture, from: i64, to: i64, qty: i64) -> Value {
    f.app
        .mutate(request(Mutation::QuarantineTransfer {
            date: DAY.into(),
            from_id: Int(from),
            to_id: Int(to),
            lines: vec![item(f.unit, qty)],
            reason: "فحص البضاعة".into(),
        }))
        .unwrap()
}

#[test]
fn variants_clone_units_warranty_and_planning_atomically_without_stock() {
    let f = Fixture::new();
    set_warranty(&f, Some(365));
    f.db().execute("INSERT INTO product_units(product_id,unit_id,factor_numerator,barcode,retail_price_minor,wholesale_price_minor) VALUES (?1,2,12,'CARTON',18000,16800)",[f.product]).unwrap();
    f.db()
        .execute(
            "INSERT INTO product_planning(product_id,supplier_id,weight_grams) VALUES (?1,?2,250)",
            rusqlite::params![f.product, f.party],
        )
        .unwrap();
    let req = variant_request(&f, variants());
    let first = f.app.mutate(req.clone()).unwrap();
    assert_eq!(first, f.app.mutate(req).unwrap());
    let ids = first["product_ids"].as_array().unwrap();
    assert_eq!(ids.len(), 2);
    for v in ids {
        let d = f
            .app
            .query(Query::Product {
                id: Int(v.as_str().unwrap().parse().unwrap()),
            })
            .unwrap();
        assert_eq!(d["units"].as_array().unwrap().len(), 2);
        assert_eq!(d["product"][0]["warranty_days"], "365");
        assert_eq!(
            d["product"][0]["template_product_id"],
            f.product.to_string()
        );
        assert_eq!(d["units"][1]["barcode"], Value::Null);
    }
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM stock_movements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        f.db()
            .execute("UPDATE products SET size='30' WHERE code='VAR-20'", [])
            .is_err()
    );
    assert!(f.db().execute("DELETE FROM product_variants", []).is_err());
    let q = f
        .app
        .query(Query::ProductVariants {
            template_id: Int(f.product),
            after_id: Int(0),
            limit: 1,
        })
        .unwrap();
    assert_eq!(q.as_array().unwrap().len(), 1);
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM product_planning WHERE weight_grams=250",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        3
    );
}

#[test]
fn variant_stale_preview_duplicate_codes_and_mid_batch_failure_leave_no_receipt() {
    let f = Fixture::new();
    let req = variant_request(&f, variants());
    set_warranty(&f, Some(90));
    let before = state(&f);
    assert!(f.app.mutate(req).unwrap_err().message.contains("تغير"));
    assert_eq!(before, state(&f));
    let mut duplicate = variants();
    duplicate[1].code = duplicate[0].code.to_lowercase();
    assert!(
        f.app
            .query(Query::VariantPreview {
                template_id: Int(f.product),
                variants: duplicate
            })
            .is_err()
    );
    let req = variant_request(&f, variants());
    f.db().execute_batch("CREATE TRIGGER fail_variant BEFORE INSERT ON products WHEN NEW.code='VAR-25' BEGIN SELECT RAISE(ABORT,'فشل تجريبي'); END;").unwrap();
    let before = state(&f);
    assert!(f.app.mutate(req).is_err());
    assert_eq!(before, state(&f));
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM product_variants", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM products", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn bundle_expansion_is_exact_and_permissions_and_inactive_components_are_checked() {
    let f = Fixture::new();
    let b = bundle(&f, vec![item(f.unit, 1_250_000)]);
    let before = state(&f);
    let p = f
        .app
        .query(Query::ExpandBundle {
            id: Int(b),
            quantity: Int(2 * SCALE),
            wholesale: true,
        })
        .unwrap();
    assert_eq!(p["rows"][0]["quantity_micros"], "2500000");
    assert_eq!(p["total_minor"], "3500");
    assert_eq!(before, state(&f));
    assert!(
        f.app
            .query(Query::ExpandBundle {
                id: Int(b),
                quantity: Int(1),
                wholesale: false
            })
            .is_err()
    );
    f.app
        .create_user("cashier", "أمين", "CashierPassword12", &[Int(2)])
        .unwrap();
    f.app.login("cashier", "CashierPassword12").unwrap();
    assert!(
        f.app
            .query(Query::ExpandBundle {
                id: Int(b),
                quantity: Int(SCALE),
                wholesale: false
            })
            .is_ok()
    );
    assert_eq!(
        f.app
            .mutate(request(Mutation::SaveBundle {
                bundle: SalesBundle {
                    id: Some(Int(b)),
                    revision: Int(0),
                    code: "KIT".into(),
                    name: "تغيير".into(),
                    active: true,
                    items: vec![item(f.unit, SCALE)]
                },
                reason: "تعديل".into()
            }))
            .unwrap_err()
            .code,
        "forbidden"
    );
    f.db()
        .execute("UPDATE product_units SET active=0 WHERE id=?1", [f.unit])
        .unwrap();
    assert!(
        f.app
            .query(Query::ExpandBundle {
                id: Int(b),
                quantity: Int(SCALE),
                wholesale: false
            })
            .is_err()
    );
    f.app.login("admin", "CorrectHorse12!").unwrap();
    f.app
        .mutate(request(Mutation::SaveBundle {
            bundle: SalesBundle {
                id: Some(Int(b)),
                revision: Int(0),
                code: "KIT".into(),
                name: "طقم متوقف".into(),
                active: false,
                items: vec![item(f.unit, 1_250_000)],
            },
            reason: "إيقاف طقم بمكون غير نشط".into(),
        }))
        .unwrap();
    assert_eq!(
        f.app.query(Query::Bundle { id: Int(b) }).unwrap()["header"][0]["active"],
        "0"
    );
}

#[test]
fn bundle_sale_uses_frozen_components_and_original_revalued_return_costs() {
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
    let b = bundle(&f, vec![item(f.unit, 2 * SCALE)]);
    let sale = f.save(expanded_draft(&f, b, 2 * SCALE));
    f.post(sale);
    f.app
        .mutate(request(Mutation::SaveBundle {
            bundle: SalesBundle {
                id: Some(Int(b)),
                revision: Int(0),
                code: "KIT".into(),
                name: "طقم جديد".into(),
                active: true,
                items: vec![item(f.unit, 3 * SCALE)],
            },
            reason: "تعديل الكميات المستقبلية".into(),
        }))
        .unwrap();
    f.app
        .mutate(request(Mutation::LandedCost {
            date: DAY.into(),
            supplier_id: Int(f.party),
            description: "شحن لاحق".into(),
            shares: vec![CostShare {
                receipt_line_id: Int(received),
                amount: Int(1000),
            }],
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (6 * SCALE, 6600));
    let line: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    return_lines(
        &f,
        sale,
        vec![ReturnLine {
            source_line_id: Int(line),
            quantity: Int(1_500_000),
        }],
    );
    assert_eq!(f.quantity_value(), (7_500_000, 8250));
    return_lines(
        &f,
        sale,
        vec![ReturnLine {
            source_line_id: Int(line),
            quantity: Int(2_500_000),
        }],
    );
    assert_eq!(f.quantity_value(), (10 * SCALE, 11000));
    let old = f.app.query(Query::Document { id: Int(sale) }).unwrap();
    assert_eq!(old["lines"][0]["quantity_micros"], "4000000");
    assert!(
        old["lines"][0]["notes"]
            .as_str()
            .unwrap()
            .contains("طقم تركيب")
    );
    f.reconcile();
}

#[test]
fn bundle_second_component_failure_rolls_back_then_sale_reversal_restores_all() {
    let f = Fixture::new();
    f.opening();
    let created = f
        .app
        .mutate(variant_request(&f, vec![variants()[0].clone()]))
        .unwrap();
    let product = created["product_ids"][0]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let unit = f
        .db()
        .query_row(
            "SELECT id FROM product_units WHERE product_id=?1",
            [product],
            |r| r.get(0),
        )
        .unwrap();
    let b = bundle(&f, vec![item(f.unit, SCALE), item(unit, SCALE)]);
    let sale = f.save(expanded_draft(&f, b, SCALE));
    let before = state(&f);
    assert!(f.app.mutate(post_request(&f, sale)).is_err());
    assert_eq!(before, state(&f));
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    let mut d = f.draft(DocumentKind::OpeningStock, 2 * SCALE, 1000, 0);
    d.lines[0].unit_id = Int(unit);
    let opening = f.save(d);
    f.post(opening);
    let req = post_request(&f, sale);
    let result = f.app.mutate(req.clone()).unwrap();
    assert_eq!(result, f.app.mutate(req).unwrap());
    reverse(&f, sale);
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT quantity_micros FROM stock_balances WHERE product_id=?1 AND warehouse_id=1",
                [product],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2 * SCALE
    );
    f.reconcile();
}

#[test]
fn warranty_snapshots_survive_catalog_changes_returns_and_quote_conversion() {
    let f = Fixture::new();
    f.opening();
    set_warranty(&f, Some(365));
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 0));
    f.post(sale);
    let quote = f.save(f.draft(DocumentKind::Quotation, SCALE, 1500, 0));
    let rev = f
        .db()
        .query_row("SELECT revision FROM documents WHERE id=?1", [quote], |r| {
            r.get(0)
        })
        .unwrap();
    f.app
        .mutate(request(Mutation::DocumentAction {
            id: Int(quote),
            revision: Int(rev),
            action: DocumentAction::Approve,
            reason: String::new(),
            acknowledge_credit: false,
        }))
        .unwrap();
    set_warranty(&f, Some(730));
    let cloned = f
        .app
        .mutate(request(Mutation::ConvertQuotation {
            id: Int(quote),
            date: DAY.into(),
        }))
        .unwrap();
    assert_eq!(
        f.app
            .query(Query::Document {
                id: Int(id(&cloned))
            })
            .unwrap()["lines"][0]["warranty_days"],
        "365"
    );
    let line = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    let returned = return_lines(
        &f,
        sale,
        vec![ReturnLine {
            source_line_id: Int(line),
            quantity: Int(SCALE),
        }],
    );
    for doc in [sale, id(&returned)] {
        let snapshot = f
            .app
            .query(Query::PrintSnapshot {
                source: PrintSource::Document { id: Int(doc) },
            })
            .unwrap();
        assert_eq!(snapshot["document"]["lines"][0]["warranty_days"], "365");
        assert!(
            f.db()
                .execute(
                    "UPDATE document_lines SET warranty_days=1 WHERE document_id=?1",
                    [doc]
                )
                .is_err()
        );
    }
    set_warranty(&f, None);
    let q = f.save(f.draft(DocumentKind::Quotation, SCALE, 1500, 0));
    let rev = f
        .db()
        .query_row("SELECT revision FROM documents WHERE id=?1", [q], |r| {
            r.get(0)
        })
        .unwrap();
    f.app
        .mutate(request(Mutation::DocumentAction {
            id: Int(q),
            revision: Int(rev),
            action: DocumentAction::Approve,
            reason: String::new(),
            acknowledge_credit: false,
        }))
        .unwrap();
    set_warranty(&f, Some(730));
    let clone = f
        .app
        .mutate(request(Mutation::ConvertQuotation {
            id: Int(q),
            date: DAY.into(),
        }))
        .unwrap();
    assert_eq!(
        f.app
            .query(Query::Document {
                id: Int(id(&clone))
            })
            .unwrap()["lines"][0]["warranty_days"],
        Value::Null
    );
    f.reconcile();
}

#[test]
fn quarantine_transfer_blocks_sale_and_preserves_value_locations_and_reversal() {
    let f = Fixture::new();
    f.opening();
    let q = quarantine(&f);
    f.app
        .mutate(request(Mutation::SaveMaster {
            master: Master::WarehouseProduct {
                warehouse_id: Int(q),
                product_id: Int(f.product),
                min_stock: Int(0),
                bin: "ب-٣".into(),
                shelf: Some("رف الفحص".into()),
            },
        }))
        .unwrap();
    let movement = transfer(&f, 1, q, 3 * SCALE);
    assert_eq!(f.quantity_value(), (7 * SCALE, 7000));
    let held = f
        .app
        .query(Query::Inventory {
            warehouse_id: Some(Int(q)),
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(held[0]["available_quantity_micros"], "0");
    assert_eq!(held[0]["quarantine_quantity_micros"], "3000000");
    assert_eq!(held[0]["shelf_location"], "رف الفحص");
    let mut d = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    d.warehouse_id = Some(Int(q));
    let sale = f.save(d);
    let before = state(&f);
    assert!(
        f.app
            .mutate(post_request(&f, sale))
            .unwrap_err()
            .message
            .contains("العزل")
    );
    assert_eq!(before, state(&f));
    assert!(
        f.db()
            .execute(
                "UPDATE documents SET number='Q-FAIL',status='posting' WHERE id=?1",
                [sale]
            )
            .is_err()
    );
    assert!(
        f.app
            .mutate(request(Mutation::SetQuarantineWarehouse {
                warehouse_id: Int(q),
                revision: Int(1),
                quarantine: false,
                reason: "تجاوز العزل".into()
            }))
            .is_err()
    );
    assert!(
        f.db()
            .execute(
                "DELETE FROM quarantine_transfers WHERE document_id=?1",
                [id(&movement)]
            )
            .is_err()
    );
    let release = transfer(&f, q, 1, SCALE);
    assert_eq!(f.quantity_value(), (8 * SCALE, 8000));
    let print = f
        .app
        .query(Query::PrintSnapshot {
            source: PrintSource::Document {
                id: Int(id(&release)),
            },
        })
        .unwrap();
    assert_eq!(print["document"]["header"][0]["from_quarantine"], "1");
    reverse(&f, id(&release));
    assert_eq!(f.quantity_value(), (7 * SCALE, 7000));
    f.reconcile();
}

#[test]
fn quarantine_generic_transfer_requires_reason_and_accountant_cannot_release_by_reversal() {
    let f = Fixture::new();
    f.opening();
    let q = quarantine(&f);
    let mut draft = f.draft(DocumentKind::Transfer, SCALE, 0, 0);
    draft.destination_id = Some(Int(q));
    draft.reason.clear();
    let doc = f.save(draft);
    let before = state(&f);
    assert!(f.app.mutate(post_request(&f, doc)).is_err());
    assert_eq!(before, state(&f));
    let original = transfer(&f, 1, q, SCALE);
    f.app
        .create_user("accountant", "محاسب", "AccountantPass12", &[Int(4)])
        .unwrap();
    f.app.login("accountant", "AccountantPass12").unwrap();
    let before = state(&f);
    assert_eq!(
        f.app
            .mutate(request(Mutation::Reverse {
                id: Int(id(&original)),
                date: DAY.into(),
                reason: "عكس".into()
            }))
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(before, state(&f));
    assert_eq!(
        f.app
            .mutate(request(Mutation::SetQuarantineWarehouse {
                warehouse_id: Int(q),
                revision: Int(1),
                quarantine: false,
                reason: "تعديل".into()
            }))
            .unwrap_err()
            .code,
        "forbidden"
    );
    f.app.login("admin", "CorrectHorse12!").unwrap();
    transfer(&f, q, 1, SCALE);
    f.app
        .mutate(request(Mutation::SetQuarantineWarehouse {
            warehouse_id: Int(q),
            revision: Int(1),
            quarantine: false,
            reason: "المخزن فارغ".into(),
        }))
        .unwrap();
    f.reconcile();
}
