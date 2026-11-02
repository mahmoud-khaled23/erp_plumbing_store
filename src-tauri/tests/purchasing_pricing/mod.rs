use super::*;
use plumbing_erp::services::{pricing, purchasing};

#[test]
fn margin_rechecks_receipt_cost_change_after_draft_without_partial_posting() {
    let f = Fixture::new();
    f.opening();
    let mut d = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    d.reason.clear();
    let sale = f.save(d);
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 3000, 0));
    f.post(receipt);
    let before = f.quantity_value();
    assert_eq!(before, (20 * SCALE, 40000));
    assert!(f.app.mutate(post_request(&f, sale, "")).is_err());
    assert_eq!(f.quantity_value(), before);
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM sale_margin_checks WHERE document_id=?1",
                [sale],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    f.app
        .mutate(post_request(&f, sale, "اعتماد بيع بخسارة"))
        .unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT cost_minor FROM sale_margin_checks WHERE document_id=?1",
                [sale],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2000
    );
    assert_eq!(f.quantity_value(), (19 * SCALE, 38000));
    f.reconcile();
}

#[test]
fn weight_allocation_requires_weights_and_rejects_changed_preview() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, SCALE, 1000, 0));
    f.post(receipt);
    let line: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get(0),
        )
        .unwrap();
    let ids = vec![Int(line)];
    assert!(
        f.app
            .query(Query::LandedPreview {
                basis: LandedBasis::Weight,
                amount: Int(100),
                receipt_line_ids: ids.clone()
            })
            .is_err()
    );
    plan(&f, f.product, f.party, 100);
    let p = f
        .app
        .query(Query::LandedPreview {
            basis: LandedBasis::Weight,
            amount: Int(100),
            receipt_line_ids: ids.clone(),
        })
        .unwrap();
    f.app
        .mutate(request(Mutation::SaveProductPlanning {
            planning: ProductPlanning {
                product_id: Int(f.product),
                revision: Int(1),
                supplier_id: Some(Int(f.party)),
                reorder_point: Int(10 * SCALE),
                max_level: Int(20 * SCALE),
                lead_time_days: 10,
                weight_grams: Int(200),
            },
            reason: "وزن مصحح".into(),
        }))
        .unwrap();
    assert!(
        f.app
            .mutate(request(Mutation::AllocateLandedCost {
                date: DAY.into(),
                supplier_id: Int(f.party),
                description: "شحن".into(),
                basis: LandedBasis::Weight,
                amount: Int(100),
                receipt_line_ids: ids,
                preview_hash: hash(&p)
            }))
            .is_err()
    );
    assert_eq!(f.quantity_value(), (SCALE, 1000));
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM landed_calculations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    f.reconcile();
}

fn add_product(f: &Fixture, code: &str) -> (i64, i64) {
    let product = id(&f
        .app
        .mutate(request(Mutation::SaveProduct {
            product: Product {
                id: None,
                revision: Int(0),
                code: code.into(),
                name: format!("محبس {code}"),
                category_id: Int(3),
                brand: String::new(),
                material: "نحاس".into(),
                size: "١ بوصة".into(),
                base_unit_id: Int(1),
                standard_cost: Int(1000),
                min_stock: Int(0),
                active: true,
                units: vec![ProductUnit {
                    id: None,
                    unit_id: Int(1),
                    numerator: Int(1),
                    denominator: Int(1),
                    barcode: None,
                    retail_price: Int(1500),
                    wholesale_price: Int(1400),
                    active: true,
                }],
            },
        }))
        .unwrap());
    let unit: i64 = f
        .db()
        .query_row(
            "SELECT id FROM product_units WHERE product_id=?1",
            [product],
            |r| r.get(0),
        )
        .unwrap();
    (product, unit)
}
fn plan(f: &Fixture, product: i64, supplier: i64, weight: i64) {
    f.app
        .mutate(request(Mutation::SaveProductPlanning {
            planning: ProductPlanning {
                product_id: Int(product),
                revision: Int(0),
                supplier_id: Some(Int(supplier)),
                reorder_point: Int(10 * SCALE),
                max_level: Int(20 * SCALE),
                lead_time_days: 10,
                weight_grams: Int(weight),
            },
            reason: "تخطيط المخزون".into(),
        }))
        .unwrap();
}
fn filter() -> ReorderFilter {
    ReorderFilter {
        warehouse_id: Int(1),
        through: DAY.into(),
        lookback_days: 10,
        coverage_days: 14,
    }
}
fn post_request(f: &Fixture, id: i64, reason: &str) -> Request {
    let revision = f
        .db()
        .query_row("SELECT revision FROM documents WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    request(Mutation::DocumentAction {
        id: Int(id),
        revision: Int(revision),
        action: DocumentAction::Post,
        reason: reason.into(),
        acknowledge_credit: false,
    })
}
fn hash(v: &Value) -> String {
    v["preview_hash"].as_str().unwrap().into()
}

#[test]
fn posted_purchase_history_is_append_only_and_reversed_cost_is_excluded() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 900, 0));
    f.post(receipt);
    let line: i64 = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get(0),
        )
        .unwrap();
    let mut invoice = f.draft(DocumentKind::Purchase, 10 * SCALE, 1000, 0);
    invoice.lines[0].matches.push(PurchaseMatch {
        receipt_line_id: Int(line),
        base_quantity: Int(10 * SCALE),
    });
    let invoice = f.save(invoice);
    assert_eq!(
        purchasing::last_cost(&f.db(), f.product, None).unwrap(),
        None
    );
    let req = post_request(&f, invoice, "");
    let posted = f.app.mutate(req.clone()).unwrap();
    assert_eq!(posted, f.app.mutate(req).unwrap());
    assert_eq!(
        purchasing::last_cost(&f.db(), f.product, None).unwrap(),
        Some(1000)
    );
    let rows = f
        .app
        .query(Query::SupplierPrices {
            product_id: Int(f.product),
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(rows[0]["net_minor"], "10000");
    assert_eq!(rows[0]["base_unit_cost_minor"], "1000");
    assert!(
        f.db()
            .execute("UPDATE supplier_price_history SET net_minor=1", [])
            .is_err()
    );
    assert!(
        f.db()
            .execute("DELETE FROM supplier_price_history", [])
            .is_err()
    );
    f.app
        .mutate(request(Mutation::Reverse {
            id: Int(invoice),
            date: DAY.into(),
            reason: "عكس فاتورة".into(),
        }))
        .unwrap();
    assert_eq!(
        purchasing::last_cost(&f.db(), f.product, None).unwrap(),
        None
    );
    let rows = f
        .app
        .query(Query::SupplierPrices {
            product_id: Int(f.product),
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["status"], "reversed");
    f.reconcile();
}

#[test]
fn reorder_uses_velocity_and_open_orders_groups_suppliers_and_rejects_stale_preview() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 0));
    f.post(sale);
    let (second, _) = add_product(&f, "SECOND");
    let supplier = id(&f
        .app
        .mutate(request(Mutation::SaveParty {
            party: Party {
                id: None,
                code: "S2".into(),
                name: "مورد ثان".into(),
                customer: false,
                supplier: true,
                phone: String::new(),
                address: String::new(),
                tax_number: String::new(),
                credit_limit: None,
                terms_days: 0,
                active: true,
                notes: String::new(),
            },
        }))
        .unwrap());
    plan(&f, f.product, f.party, 100);
    plan(&f, second, supplier, 200);
    let ids = vec![Int(second), Int(f.product)];
    let preview = f
        .app
        .query(Query::SuggestedOrdersPreview {
            filter: filter(),
            product_ids: ids.clone(),
        })
        .unwrap();
    assert_eq!(preview["rows"][0]["net_sales_micros"], "2000000");
    assert_eq!(preview["rows"][0]["suggested_micros"], "12000000");
    assert_eq!(preview["rows"][0]["suggested_unit_cost_minor"], "1000");
    assert!(preview["rows"][0]["last_purchase_cost_minor"].is_null());
    let req = request(Mutation::CreateSuggestedOrders {
        filter: filter(),
        product_ids: ids.clone(),
        preview_hash: hash(&preview),
        reason: "إعادة طلب".into(),
    });
    let created = f.app.mutate(req.clone()).unwrap();
    assert_eq!(created, f.app.mutate(req).unwrap());
    assert_eq!(created["document_ids"].as_array().unwrap().len(), 2);
    assert_eq!(f.db().query_row("SELECT COUNT(DISTINCT party_id) FROM documents WHERE kind='purchase_order' AND status='draft'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert!(
        f.app
            .mutate(request(Mutation::CreateSuggestedOrders {
                filter: filter(),
                product_ids: ids,
                preview_hash: hash(&preview),
                reason: "مكرر".into()
            }))
            .is_err()
    );
    let rows = f
        .app
        .query(Query::ReorderSuggestions {
            filter: filter(),
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(rows[0]["on_order_micros"], "12000000");
    assert_eq!(rows[0]["suggested_micros"], "0");
    assert_eq!(f.quantity_value(), (8 * SCALE, 8000));
    f.reconcile();
}

#[test]
fn reorder_velocity_rounds_up_and_return_quantities_reduce_demand() {
    let f = Fixture::new();
    f.opening();
    plan(&f, f.product, f.party, 100);
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 0));
    f.post(sale);
    let line: i64 = f
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
            warehouse_id: Some(Int(1)),
            lines: vec![ReturnLine {
                source_line_id: Int(line),
                quantity: Int(SCALE / 2),
            }],
            payments: vec![],
            reason: "مرتجع".into(),
        }))
        .unwrap();
    let mut ftr = filter();
    ftr.lookback_days = 7;
    ftr.coverage_days = 100;
    let rows = f
        .app
        .query(Query::ReorderSuggestions {
            filter: ftr,
            after_id: Int(0),
            limit: 50,
        })
        .unwrap();
    assert_eq!(rows[0]["net_sales_micros"], "1500000");
    assert_eq!(rows[0]["target_micros"], "23571429");
    assert_eq!(rows[0]["suggested_micros"], "15071429");
    f.reconcile();
}

#[test]
fn bulk_prices_round_exactly_are_idempotent_and_stale_preview_changes_nothing() {
    let f = Fixture::new();
    f.opening();
    let ids = vec![Int(f.unit)];
    let rule = PriceRule {
        basis: PriceBasis::Percentage,
        price_list: PriceList::Both,
        rate_bps: Int(125),
    };
    let preview = f
        .app
        .query(Query::PricePreview {
            unit_ids: ids.clone(),
            rule: rule.clone(),
        })
        .unwrap();
    assert_eq!(preview["rows"][0]["new_retail_minor"], "1519");
    assert_eq!(preview["rows"][0]["new_wholesale_minor"], "1418");
    let req = request(Mutation::ApplyPrices {
        unit_ids: ids.clone(),
        rule: rule.clone(),
        preview_hash: hash(&preview),
        reason: "زيادة".into(),
    });
    let result = f.app.mutate(req.clone()).unwrap();
    assert_eq!(result, f.app.mutate(req).unwrap());
    assert!(
        f.app
            .mutate(request(Mutation::ApplyPrices {
                unit_ids: ids.clone(),
                rule,
                preview_hash: hash(&preview),
                reason: "قديم".into()
            }))
            .is_err()
    );
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM price_update_batches", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        f.db()
            .execute("DELETE FROM price_update_lines", [])
            .is_err()
    );
    let markup = PriceRule {
        basis: PriceBasis::CostMarkup,
        price_list: PriceList::Retail,
        rate_bps: Int(2500),
    };
    let p = f
        .app
        .query(Query::PricePreview {
            unit_ids: ids,
            rule: markup,
        })
        .unwrap();
    assert_eq!(p["rows"][0]["new_retail_minor"], "1250");
    assert_eq!(p["rows"][0]["new_wholesale_minor"], "1418");
    f.reconcile();
}

#[test]
fn failed_second_price_write_rolls_back_entire_audited_batch() {
    let f = Fixture::new();
    let (_, unit) = add_product(&f, "FAIL");
    let ids = vec![Int(f.unit), Int(unit)];
    let rule = PriceRule {
        basis: PriceBasis::Percentage,
        price_list: PriceList::Retail,
        rate_bps: Int(1000),
    };
    let p = f
        .app
        .query(Query::PricePreview {
            unit_ids: ids.clone(),
            rule: rule.clone(),
        })
        .unwrap();
    f.db().execute_batch(&format!("CREATE TRIGGER fail_price BEFORE UPDATE OF retail_price_minor ON product_units WHEN OLD.id={unit} BEGIN SELECT RAISE(ABORT,'رفض اختبار'); END;")).unwrap();
    let req = request(Mutation::ApplyPrices {
        unit_ids: ids,
        rule,
        preview_hash: hash(&p),
        reason: "اختبار".into(),
    });
    let uuid = req.request_id.clone();
    assert!(f.app.mutate(req).is_err());
    assert_eq!(
        f.db()
            .query_row(
                "SELECT retail_price_minor FROM product_units WHERE id=?1",
                [f.unit],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1500
    );
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM price_update_batches", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM command_receipts WHERE request_id=?1",
                [uuid],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn margin_guard_rechecks_posted_cost_and_policy_and_requires_permission_and_reason() {
    let f = Fixture::new();
    f.opening();
    let mut draft = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    draft.reason.clear();
    let sale = f.save(draft.clone());
    let margins = f
        .app
        .query(Query::SaleMargins {
            warehouse_id: Int(1),
            lines: draft.lines.clone(),
            discount: Int(0),
        })
        .unwrap();
    assert_eq!(margins[0]["margin_minor"], "500");
    assert_eq!(margins[0]["override_required"], "0");
    f.app
        .mutate(request(Mutation::SavePricingPolicy {
            revision: Int(0),
            minimum_margin_bps: 4000,
            reason: "حماية الهامش".into(),
        }))
        .unwrap();
    let original = f.quantity_value();
    let error = f.app.mutate(post_request(&f, sale, "")).unwrap_err();
    assert!(
        error.code == "validation" && error.message.contains("تجاوز السعر"),
        "{error:?}"
    );
    assert_eq!(f.quantity_value(), original);
    f.app
        .create_user("cashier", "أمين الصندوق", "CashierPassword12", &[Int(2)])
        .unwrap();
    f.app.login("cashier", "CashierPassword12").unwrap();
    assert_eq!(
        f.app
            .mutate(post_request(&f, sale, "طلب العميل"))
            .unwrap_err()
            .code,
        "forbidden"
    );
    f.app.login("admin", "CorrectHorse12!").unwrap();
    f.app
        .mutate(post_request(&f, sale, "موافقة المدير على الهامش"))
        .unwrap();
    let checked:(i64,i64,Option<i64>,String)=f.db().query_row("SELECT cost_minor,minimum_margin_bps,override_user_id,reason FROM sale_margin_checks WHERE document_id=?1",[sale],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(checked.0, 1000);
    assert_eq!(checked.1, 4000);
    assert!(checked.2.is_some());
    assert!(checked.3.contains("المدير"));
    assert!(!pricing::below_margin(1250, 1000, 2000));
    assert!(pricing::below_margin(1249, 1000, 2000));
    assert!(pricing::below_margin(999, 1000, 0));
    f.reconcile();
}

#[test]
fn duplicate_product_margin_preview_matches_sequential_weighted_issues_and_discounts() {
    let f = Fixture::new();
    let open = f.save(f.draft(DocumentKind::OpeningStock, 3 * SCALE, 1, 0));
    f.post(open);
    let mut d = f.draft(DocumentKind::Sale, SCALE, 2, 0);
    d.lines.push(d.lines[0].clone());
    d.lines[1].quantity = Int(2 * SCALE);
    d.discount = Int(1);
    let p = f
        .app
        .query(Query::SaleMargins {
            warehouse_id: Int(1),
            lines: d.lines.clone(),
            discount: d.discount,
        })
        .unwrap();
    let sale = f.save(d);
    f.post(sale);
    let rows:Vec<(i64,i64)>=f.db().prepare("SELECT cost_minor,net_minor FROM sale_margin_checks WHERE document_id=?1 ORDER BY document_line_id").unwrap().query_map([sale],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    for (i, (cost, net)) in rows.iter().enumerate() {
        assert_eq!(p[i]["cost_minor"], cost.to_string());
        assert_eq!(p[i]["net_minor"], net.to_string());
    }
    assert_eq!(rows.iter().map(|x| x.0).sum::<i64>(), 3);
    f.reconcile();
}

#[test]
fn automatic_landed_allocations_conserve_piastres_and_revalue_sold_stock() {
    let f = Fixture::new();
    let (second, unit) = add_product(&f, "WEIGHT");
    plan(&f, f.product, f.party, 100);
    plan(&f, second, f.party, 300);
    let mut d = f.draft(DocumentKind::GoodsReceipt, 2 * SCALE, 1000, 0);
    let mut line = d.lines[0].clone();
    line.unit_id = Int(unit);
    line.quantity = Int(SCALE);
    line.price = Int(2000);
    d.lines.push(line);
    let receipt = f.save(d);
    f.post(receipt);
    let ids: Vec<Int> = f
        .db()
        .prepare("SELECT id FROM document_lines WHERE document_id=?1 ORDER BY id")
        .unwrap()
        .query_map([receipt], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    for (basis, expected) in [
        (LandedBasis::Value, [51, 50]),
        (LandedBasis::Quantity, [67, 34]),
        (LandedBasis::Weight, [40, 61]),
    ] {
        let p = f
            .app
            .query(Query::LandedPreview {
                basis,
                amount: Int(101),
                receipt_line_ids: ids.clone(),
            })
            .unwrap();
        assert_eq!(p["rows"][0]["amount_minor"], expected[0].to_string());
        assert_eq!(p["rows"][1]["amount_minor"], expected[1].to_string());
    }
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 0));
    f.post(sale);
    let p = f
        .app
        .query(Query::LandedPreview {
            basis: LandedBasis::Weight,
            amount: Int(101),
            receipt_line_ids: ids.clone(),
        })
        .unwrap();
    let req = request(Mutation::AllocateLandedCost {
        date: DAY.into(),
        supplier_id: Int(f.party),
        description: "شحن بالوزن".into(),
        basis: LandedBasis::Weight,
        amount: Int(101),
        receipt_line_ids: ids,
        preview_hash: hash(&p),
    });
    let doc = f.app.mutate(req.clone()).unwrap();
    assert_eq!(doc, f.app.mutate(req).unwrap());
    assert_eq!(f.quantity_value(), (SCALE, 1020));
    let totals: (i64, i64) = f
        .db()
        .query_row(
            "SELECT SUM(inventory_share_minor),SUM(cogs_share_minor) FROM landed_allocations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(totals, (81, 20));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT basis FROM landed_calculations WHERE document_id=?1",
                [id(&doc)],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "weight"
    );
    f.reconcile();
}
