use super::*;
use plumbing_erp::{
    db,
    services::{
        documents,
        finance::{Entry, Journal},
        inventory,
    },
    state::Actor,
};
use rusqlite::params;
use std::time::Instant;

pub(super) fn assert_balances(db: &Connection) {
    let ledger = db::rows(db, "SELECT product_id,warehouse_id,SUM(quantity_micros) quantity_micros,SUM(value_minor) value_minor,MAX(id) last_movement_id FROM stock_movements GROUP BY product_id,warehouse_id ORDER BY product_id,warehouse_id", []).unwrap();
    let cache = db::rows(
        db,
        "SELECT * FROM stock_balances ORDER BY product_id,warehouse_id",
        [],
    )
    .unwrap();
    assert_eq!(cache, ledger);
    db::verify_stock_balances(db).unwrap();
}

#[test]
fn negative_receipt_adjustments_and_stocktake_keep_balances_exact() {
    let f = Fixture::new();
    f.db()
        .execute("UPDATE store_settings SET allow_negative_stock=1", [])
        .unwrap();
    let sale = f.save(f.draft(DocumentKind::Sale, 2 * SCALE, 1500, 3000));
    f.post(sale);
    assert_eq!(f.quantity_value(), (-2 * SCALE, -2000));
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 5 * SCALE, 1200, 0));
    f.post(receipt);
    assert_eq!(f.quantity_value(), (3 * SCALE, 3600));
    f.app
        .mutate(request(Mutation::Adjustment {
            date: DAY.into(),
            warehouse_id: Int(1),
            product_id: Int(f.product),
            quantity: Int(SCALE),
            unit_cost: Int(1200),
            reason: "زيادة فعلية".into(),
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (4 * SCALE, 4800));
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
                quantity: Int(3 * SCALE),
                reason: "عجز".into(),
            }],
        }))
        .unwrap();
    f.post(count);
    assert_eq!(f.quantity_value(), (3 * SCALE, 3600));
    f.reconcile();
}

#[test]
fn failed_second_issue_rolls_back_cache_ledgers_audit_and_receipt() {
    let f = Fixture::new();
    f.opening();
    let mut sale = f.draft(DocumentKind::Sale, 8 * SCALE, 1500, 0);
    sale.lines.push(sale.lines[0].clone());
    let doc = f.save(sale);
    let db = f.db();
    let before = db::rows(&db, "SELECT * FROM stock_balances", []).unwrap();
    let counts = || {
        db::rows(&db, "SELECT (SELECT COUNT(*) FROM stock_movements) movements,(SELECT COUNT(*) FROM journal_lines) journal,(SELECT COUNT(*) FROM audit_log) audit,(SELECT COUNT(*) FROM command_receipts) receipts", []).unwrap()
    };
    let before_counts = counts();
    let request = request(Mutation::DocumentAction {
        id: Int(doc),
        revision: Int(1),
        action: DocumentAction::Post,
        reason: String::new(),
        acknowledge_credit: false,
    });
    assert!(f.app.mutate(request).is_err());
    assert_eq!(
        db::rows(&db, "SELECT * FROM stock_balances", []).unwrap(),
        before
    );
    assert_eq!(counts(), before_counts);
    assert_balances(&db);
}

#[test]
fn landed_preview_revaluation_and_its_reversal_preserve_balances_and_stocktake_guard() {
    let f = Fixture::new();
    let receipt = f.save(f.draft(DocumentKind::GoodsReceipt, 10 * SCALE, 1000, 0));
    f.post(receipt);
    let line = f
        .db()
        .query_row(
            "SELECT id FROM document_lines WHERE document_id=?1",
            [receipt],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
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
                quantity: Int(10 * SCALE),
                reason: String::new(),
            }],
        }))
        .unwrap();
    let charge = id(&f
        .app
        .mutate(request(Mutation::LandedCost {
            date: DAY.into(),
            supplier_id: Int(f.party),
            description: "شحن".into(),
            shares: vec![CostShare {
                receipt_line_id: Int(line),
                amount: Int(1000),
            }],
        }))
        .unwrap());
    assert_eq!(f.quantity_value(), (10 * SCALE, 11000));
    assert_eq!(db::verify_stock_balances(&f.db()).unwrap(), (1, 2));
    f.app
        .mutate(request(Mutation::Reverse {
            id: Int(charge),
            date: DAY.into(),
            reason: "تصحيح الشحن".into(),
        }))
        .unwrap();
    assert_eq!(f.quantity_value(), (10 * SCALE, 10000));
    assert_eq!(db::verify_stock_balances(&f.db()).unwrap(), (1, 3));
    // Identical final quantity/value do not make the old count snapshot valid again.
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
    f.reconcile();
}

#[test]
fn stock_check_requires_permission_and_does_not_write() {
    let f = Fixture::new();
    f.opening();
    let audit = || {
        f.db()
            .query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get::<_, i64>(0))
            .unwrap()
    };
    let before = audit();
    assert_eq!(
        f.app.query(Query::StockIntegrity).unwrap(),
        json!({"pools":"1","movements":"1"})
    );
    assert_eq!(audit(), before);
    for (role, name, permitted) in [
        (2, "cashier", false),
        (3, "storekeeper", true),
        (4, "accountant", true),
    ] {
        f.app
            .create_user(name, name, "TestPassword12!", &[Int(role)])
            .unwrap();
        f.app.login(name, "TestPassword12!").unwrap();
        let result = f.app.query(Query::StockIntegrity);
        if permitted {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().code, "forbidden");
        }
        f.app.login("admin", "CorrectHorse12!").unwrap();
    }
}

#[test]
fn mismatch_blocks_startup_backup_and_maintenance_without_repair() {
    let f = Fixture::new();
    f.opening();
    f.db()
        .execute("UPDATE stock_balances SET value_minor=value_minor+1", [])
        .unwrap();
    assert!(db::integrity(&f.db(), true).is_err());
    assert!(f.app.backup(&f.temp.path().join("bad.erpbackup")).is_err());
    let error = f.app.query(Query::StockIntegrity).unwrap_err();
    assert_eq!(error.code, "stock_balance_mismatch");
    assert_eq!(
        f.app.query(Query::Bootstrap).unwrap()["recovery_required"],
        true
    );
    assert!(f.app.login("admin", "CorrectHorse12!").is_err());
    assert!(App::open(f.temp.path().join("store")).is_err());
    let recovery = App::open_with_recovery(f.temp.path().join("store")).unwrap();
    assert_eq!(
        recovery.query(Query::Bootstrap).unwrap()["recovery_required"],
        true
    );
    assert_eq!(
        f.db()
            .query_row("SELECT value_minor FROM stock_balances", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        10001
    );
    assert_eq!(
        f.db()
            .query_row("SELECT SUM(value_minor) FROM stock_movements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        10000
    );
}

#[test]
fn verifier_detects_missing_extra_zero_quantity_and_high_water_rows() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.post(sale);
    let db = f.db();
    db.execute(
        "INSERT INTO warehouses(id,code,name_ar) VALUES (2,'SECOND','مخزن الاختبار')",
        [],
    )
    .unwrap();
    for corruption in [
        "DELETE FROM stock_balances",
        "UPDATE stock_balances SET quantity_micros=quantity_micros+1",
        "UPDATE stock_balances SET last_movement_id=1",
        "INSERT INTO stock_balances SELECT product_id,2,0,0,last_movement_id FROM stock_balances",
        "DROP TABLE stock_balances",
    ] {
        // Deliberately simulate external corruption; application code has no cache-write API.
        db.execute_batch("SAVEPOINT corrupt").unwrap();
        db.execute_batch(corruption).unwrap();
        assert_eq!(
            db::verify_stock_balances(&db).unwrap_err().code,
            "stock_balance_mismatch"
        );
        db.execute_batch("ROLLBACK TO corrupt; RELEASE corrupt")
            .unwrap();
    }
    assert_balances(&db);
}

/// Real guarded movement inserts; no triggers or indexes are disabled. A single
/// synthetic adjustment holds the historical fixture, with its balanced journal.
fn history(db: &Connection, actor: &Actor, product: i64, movements: i64) {
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let doc = documents::header(
        db,
        actor,
        "adjustment",
        DAY,
        None,
        Some(1),
        "بيانات اختبار الأداء",
    )
    .unwrap();
    documents::begin(db, doc).unwrap();
    db.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<?1) INSERT INTO stock_movements(document_id,product_id,warehouse_id,movement_type,quantity_micros,value_minor,unit_cost_micropiastres,created_by) SELECT ?2,?3,1,'adjustment',1000000,1000,1000000000,1 FROM n",params![movements,doc,product]).unwrap();
    let value = narrow(i128::from(movements) * 1000).unwrap();
    let mut journal = Journal::default();
    journal.add(Entry::new(120, value));
    journal.add(Entry::new(503, -value));
    journal.write(db, doc).unwrap();
    documents::finish(db, actor, doc).unwrap();
    db::audit(db, actor.user_id, "benchmark_fixture", "document", doc, "").unwrap();
    db.execute("INSERT INTO command_receipts(request_id,command,payload_sha256,user_id,document_id,result_json) VALUES (?1,'benchmark_fixture',?2,?3,?4,'{}')",params![uuid::Uuid::new_v4().to_string(),db::digest(b"benchmark_fixture"),actor.user_id,doc]).unwrap();
    db.execute_batch("COMMIT").unwrap();
}

fn latency(db: &Connection, actor: &Actor, product: i64) -> (u128, u128) {
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let doc = documents::header(db, actor, "adjustment", DAY, None, Some(1), "قياس الأداء").unwrap();
    documents::begin(db, doc).unwrap();
    let (mut receipts, mut issues) = (Vec::new(), Vec::new());
    // Nine batches reduce scheduling noise. First batch warms prepared/page caches.
    for batch in 0..10 {
        let (mut received, mut issued) = (0, 0);
        for _ in 0..50 {
            let start = Instant::now();
            inventory::receive(db, actor, doc, None, product, 1, SCALE, 1000, "adjustment")
                .unwrap();
            received += start.elapsed().as_nanos();
            let start = Instant::now();
            inventory::issue(db, actor, doc, None, product, 1, SCALE, "adjustment").unwrap();
            issued += start.elapsed().as_nanos();
        }
        if batch > 0 {
            receipts.push(received / 50);
            issues.push(issued / 50);
        }
    }
    db.execute_batch("ROLLBACK").unwrap();
    receipts.sort_unstable();
    issues.sort_unstable();
    (receipts[4], issues[4])
}

#[test]
fn issue_receive_latency_does_not_scale_with_200000_movements() {
    let f = Fixture::new();
    let db = f.db();
    db.execute_batch(include_str!("../../db/connection.sql"))
        .unwrap();
    let actor = Actor {
        user_id: 1,
        permissions: Default::default(),
        must_change_password: false,
    };
    history(&db, &actor, f.product, 1000);
    let small = latency(&db, &actor, f.product);
    history(&db, &actor, f.product, 199000);
    let count = db
        .query_row("SELECT COUNT(*) FROM stock_movements", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    assert_eq!(count, 200000);
    let large = latency(&db, &actor, f.product);
    eprintln!(
        "stock benchmark (debug, transaction-local service calls, median ns/op): movements=1000 receive={} issue={}; movements=200000 receive={} issue={}",
        small.0, small.1, large.0, large.1
    );
    // 200x history must not produce linear latency growth; generous noise allowance.
    // This is a regression gate, not a Windows performance SLA or an fsync benchmark.
    assert!(
        large.0 <= small.0 * 5 + 100_000,
        "receive latency grew with history"
    );
    assert!(
        large.1 <= small.1 * 5 + 100_000,
        "issue latency grew with history"
    );
    assert_eq!(
        inventory::pool(&db, f.product, 1).unwrap(),
        Pool {
            quantity: 200000 * SCALE,
            value: 200000000
        }
    );
    assert_balances(&db);
    f.reconcile();
}
