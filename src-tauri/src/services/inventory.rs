use super::{
    costing::Pool,
    documents::{self, Line},
    finance::{self, Entry, Journal},
};
use crate::{
    error::{Error, Result, ensure},
    models::arithmetic::{SCALE, allocate, prorate, sum},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn pool(db: &Connection, product: i64, warehouse: i64) -> Result<Pool> {
    Ok(db.query_row("SELECT COALESCE(SUM(quantity_micros),0),COALESCE(SUM(value_minor),0) FROM stock_movements WHERE product_id=?1 AND warehouse_id=?2",params![product,warehouse],|r|Ok(Pool{quantity:r.get(0)?,value:r.get(1)?}))?)
}
pub fn movement(
    db: &Connection,
    a: &Actor,
    doc: i64,
    line: Option<i64>,
    product: i64,
    warehouse: i64,
    qty: i64,
    value: i64,
    kind: &str,
    reversal: Option<i64>,
) -> Result<i64> {
    let cost = if qty == 0 {
        0
    } else {
        prorate(value.abs(), 1_000_000_000_000, qty.abs())?
    };
    db.execute("INSERT INTO stock_movements(document_id,document_line_id,product_id,warehouse_id,movement_type,quantity_micros,value_minor,unit_cost_micropiastres,reverses_movement_id,reason,created_by) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,(SELECT reason FROM documents WHERE id=?1),?10)",params![doc,line,product,warehouse,kind,qty,value,cost,reversal,a.user_id])?;
    Ok(db.last_insert_rowid())
}
pub fn issue(
    db: &Connection,
    a: &Actor,
    doc: i64,
    line: Option<i64>,
    product: i64,
    warehouse: i64,
    qty: i64,
    kind: &str,
) -> Result<i64> {
    let mut pool = pool(db, product, warehouse)?;
    let fallback: i64 = db.query_row(
        "SELECT standard_cost_minor FROM products WHERE id=?1",
        [product],
        |r| r.get(0),
    )?;
    let negative: bool =
        db.query_row("SELECT allow_negative_stock FROM store_settings", [], |r| {
            r.get(0)
        })?;
    let cost = pool.issue(qty, fallback, negative)?;
    movement(
        db, a, doc, line, product, warehouse, -qty, -cost, kind, None,
    )?;
    Ok(cost)
}
pub fn receive(
    db: &Connection,
    a: &Actor,
    doc: i64,
    line: Option<i64>,
    product: i64,
    warehouse: i64,
    qty: i64,
    cost: i64,
    kind: &str,
) -> Result<(i64, i64)> {
    let mut pool = pool(db, product, warehouse)?;
    let (stock, correction) = pool.receive(qty, cost)?;
    movement(db, a, doc, line, product, warehouse, qty, stock, kind, None)?;
    Ok((stock, correction))
}
pub fn adjust(
    db: &Connection,
    a: &Actor,
    day: &str,
    warehouse: i64,
    product: i64,
    qty: i64,
    cost: i64,
    reason: &str,
) -> Result<Value> {
    ensure(
        qty != 0
            && qty.unsigned_abs() <= 1_000_000_000_000_000
            && cost >= 0
            && !reason.trim().is_empty(),
        "كمية التسوية وتكلفتها وسببها مطلوبة",
    )?;
    let doc = documents::header(db, a, "adjustment", day, None, Some(warehouse), reason)?;
    documents::begin(db, doc)?;
    let mut j = Journal::default();
    adjust_line(db, a, doc, warehouse, product, qty, cost, &mut j)?;
    j.write(db, doc)?;
    documents::finish(db, a, doc)
}
fn adjust_line(
    db: &Connection,
    a: &Actor,
    doc: i64,
    warehouse: i64,
    product: i64,
    qty: i64,
    cost: i64,
    j: &mut Journal,
) -> Result<()> {
    if qty == 0 {
        return Ok(());
    }
    let (value, correction) = if qty > 0 {
        receive(
            db,
            a,
            doc,
            None,
            product,
            warehouse,
            qty,
            prorate(cost, qty, SCALE)?,
            "adjustment",
        )?
    } else {
        (
            -issue(db, a, doc, None, product, warehouse, -qty, "adjustment")?,
            0,
        )
    };
    let mut inv = Entry::new(finance::account(db, "inventory")?, value);
    inv.product = Some(product);
    j.add(inv);
    let mut cogs = Entry::new(finance::account(db, "cogs")?, correction);
    cogs.product = Some(product);
    j.add(cogs);
    j.add(Entry::new(
        finance::account(db, "stock_variance")?,
        -sum([value, correction])?,
    ));
    Ok(())
}
pub fn start_count(
    db: &Connection,
    a: &Actor,
    day: &str,
    warehouse: i64,
    products: &[Int],
    reason: &str,
) -> Result<Value> {
    ensure(
        !products.is_empty() && products.len() <= 10000 && !reason.trim().is_empty(),
        "أصناف وسبب الجرد مطلوبة",
    )?;
    let doc = documents::header(db, a, "stocktake", day, None, Some(warehouse), reason)?;
    for product in products {
        let p = pool(db, product.0, warehouse)?;
        let last: Option<i64> = db.query_row(
            "SELECT MAX(id) FROM stock_movements WHERE product_id=?1 AND warehouse_id=?2",
            params![product, warehouse],
            |r| r.get(0),
        )?;
        db.execute("INSERT INTO stocktake_lines(document_id,product_id,expected_quantity_micros,snapshot_movement_id) VALUES (?1,?2,?3,?4)",params![doc,product,p.quantity,last])?;
    }
    Ok(json!({"id":Int(doc),"revision":"0"}))
}
pub fn count(
    db: &Connection,
    doc: i64,
    revision: i64,
    user: i64,
    lines: &[CountLine],
) -> Result<Value> {
    ensure(db.execute("UPDATE documents SET revision=revision+1 WHERE id=?1 AND kind='stocktake' AND status='draft' AND revision=?2",params![doc,revision])?==1,"ورقة الجرد تغيرت أو أغلقت")?;
    for l in lines {
        ensure(l.quantity.0 >= 0, "الكمية المعدودة سالبة")?;
        ensure(db.execute("UPDATE stocktake_lines SET counted_quantity_micros=?1,counted_by=?2,counted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),reason=?3 WHERE document_id=?4 AND product_id=?5",params![l.quantity,user,l.reason,doc,l.product_id])?==1,"الصنف غير موجود بورقة الجرد")?;
    }
    Ok(json!({"id":Int(doc),"revision":Int(revision+1)}))
}
pub fn post_count(db: &Connection, a: &Actor, doc: i64) -> Result<Value> {
    let warehouse: i64 = db.query_row(
        "SELECT warehouse_id FROM documents WHERE id=?1",
        [doc],
        |r| r.get(0),
    )?;
    let rows:Vec<(i64,i64,Option<i64>,i64)>=db.prepare("SELECT s.product_id,s.expected_quantity_micros,s.counted_quantity_micros,p.standard_cost_minor FROM stocktake_lines s JOIN products p ON p.id=s.product_id WHERE s.document_id=?1")?.query_map([doc],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    for (product, expected, counted, default) in rows {
        let counted = counted.ok_or_else(|| Error::validation("استكمل عد جميع الأصناف"))?;
        let p = pool(db, product, warehouse)?;
        ensure(
            p.quantity == expected,
            "تغير رصيد الصنف؛ أعد إعداد ورقة الجرد",
        )?;
        let cost = if p.quantity > 0 {
            prorate(p.value, SCALE, p.quantity)?
        } else {
            default
        };
        adjust_line(
            db,
            a,
            doc,
            warehouse,
            product,
            counted - expected,
            cost,
            &mut journal,
        )?;
    }
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}

#[derive(Clone)]
struct Move {
    id: i64,
    _doc: i64,
    line: Option<i64>,
    product: i64,
    warehouse: i64,
    kind: String,
    qty: i64,
    value: i64,
    reverse: Option<i64>,
    party: Option<i64>,
    category: Option<i64>,
}
/// Replay a marginal receipt cost change through moving-average pools, transfers and returns.
/// Original rows remain immutable. Inventory and expense differences post on the current date.
fn revalue(
    db: &Connection,
    a: &Actor,
    doc: i64,
    receipt_line: i64,
    delta: i64,
    journal: &mut Journal,
) -> Result<i64> {
    if delta == 0 {
        return Ok(0);
    }
    let product: i64 = db.query_row(
        "SELECT product_id FROM document_lines WHERE id=?1",
        [receipt_line],
        |r| r.get(0),
    )?;
    let mut statement=db.prepare("SELECT s.id,s.document_id,s.document_line_id,s.product_id,s.warehouse_id,s.movement_type,s.quantity_micros,s.value_minor,s.reverses_movement_id,d.party_id,l.category_id_snapshot FROM stock_movements s JOIN documents d ON d.id=s.document_id LEFT JOIN document_lines l ON l.id=s.document_line_id WHERE d.status IN ('posted','posting') AND s.product_id=?1 ORDER BY s.id")?;
    let movements = statement.query_map([product], |r| {
        Ok(Move {
            id: r.get(0)?,
            _doc: r.get(1)?,
            line: r.get(2)?,
            product: r.get(3)?,
            warehouse: r.get(4)?,
            kind: r.get(5)?,
            qty: r.get(6)?,
            value: r.get(7)?,
            reverse: r.get(8)?,
            party: r.get(9)?,
            category: r.get(10)?,
        })
    })?;
    let mut pools: BTreeMap<(i64, i64), (Pool, Pool)> = BTreeMap::new();
    let mut deltas: BTreeMap<i64, i64> = BTreeMap::new();
    let mut expenses: BTreeMap<i64, (String, i64)> = BTreeMap::new();
    let mut transfer_delta: BTreeMap<i64, i64> = BTreeMap::new();
    let mut sale_delta: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    let mut returned: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    let mut found = false;
    for m in movements {
        let m = m?;
        let (base, hyp) = pools.entry((m.product, m.warehouse)).or_default();
        let mut change = 0;
        let mut expense: Option<(&str, i64)> = None;
        match m.kind.as_str() {
            "receipt" | "opening" => {
                let addition = if m.line == Some(receipt_line) && m.kind == "receipt" {
                    ensure(!found, "مرجع الاستلام يحتوي على حركات مكررة")?;
                    found = true;
                    delta
                } else {
                    0
                };
                let actual = if let Some(line) = m.line {
                    db.query_row("SELECT subtotal_minor-line_discount_minor-invoice_discount_share_minor FROM document_lines WHERE id=?1",[line],|r|r.get::<_,i64>(0))?
                } else {
                    m.value
                };
                let mut b = *base;
                let mut h = *hyp;
                let (bv, _) = b.receive(m.qty, actual)?;
                let (hv, _) = h.receive(m.qty, sum([actual, addition])?)?;
                change = hv - bv;
                expense = Some(("cogs", addition - change));
            }
            "sale" | "purchase_return" | "transfer_out" => {
                let mut b = *base;
                let mut h = *hyp;
                let fallback = prorate(-m.value, SCALE, -m.qty)?;
                change = -(h.issue(-m.qty, fallback, true)? - b.issue(-m.qty, fallback, true)?);
                if m.kind == "transfer_out" {
                    if let Some(line) = m.line {
                        transfer_delta.insert(line, -change);
                    }
                } else {
                    expense = Some((
                        if m.kind == "sale" {
                            "cogs"
                        } else {
                            "purchase_variance"
                        },
                        -change,
                    ));
                    if m.kind == "sale" {
                        if let Some(line) = m.line {
                            sale_delta.insert(line, (-m.qty, -change));
                        }
                    }
                }
            }
            "transfer_in" => {
                change = m
                    .line
                    .and_then(|l| transfer_delta.get(&l).copied())
                    .unwrap_or(0);
            }
            "sale_return" => {
                if let Some(line) = m.line {
                    let source: Option<i64> = db.query_row(
                        "SELECT source_line_id FROM document_lines WHERE id=?1",
                        [line],
                        |r| r.get(0),
                    )?;
                    if let Some(source) = source {
                        if let Some((quantity, total_delta)) = sale_delta.get(&source) {
                            let used = returned.entry(source).or_default();
                            change = if used.0 + m.qty == *quantity {
                                *total_delta - used.1
                            } else {
                                prorate(*total_delta, m.qty, *quantity)?
                            };
                            used.0 += m.qty;
                            used.1 += change;
                        }
                    }
                }
                expense = Some(("cogs", -change));
            }
            "adjustment" if m.qty < 0 => {
                let mut b = *base;
                let mut h = *hyp;
                let fallback = prorate(-m.value, SCALE, -m.qty)?;
                change = -(h.issue(-m.qty, fallback, true)? - b.issue(-m.qty, fallback, true)?);
                expense = Some(("stock_variance", -change));
            }
            "reversal" => {
                if let Some(reverse) = m.reverse {
                    change = -deltas.get(&reverse).copied().unwrap_or(0);
                    if let Some((key, value)) = expenses.get(&reverse) {
                        expense = Some((key.as_str(), -*value));
                    }
                }
            }
            _ => {}
        }
        if let Some((key, value)) = expense {
            let key = key.to_owned();
            let account = finance::account(db, &key)?;
            let mut e = Entry::new(account, value);
            e.product = Some(m.product);
            e.category = m.category;
            e.party = m.party;
            journal.add(e);
            if value != 0 {
                db.execute("INSERT INTO cost_attributions(document_id,source_movement_id,account_id,delta_minor) VALUES (?1,?2,?3,?4)",params![doc,m.id,account,value])?;
            }
            expenses.insert(m.id, (key, value));
        }
        base.quantity = sum([base.quantity, m.qty])?;
        base.value = sum([base.value, m.value])?;
        hyp.quantity = sum([hyp.quantity, m.qty])?;
        hyp.value = sum([hyp.value, m.value, change])?;
        deltas.insert(m.id, change);
    }
    ensure(found, "مرجع الاستلام غير موجود أو لم يرحل")?;
    let mut inventory_total = 0;
    for ((product, warehouse), (base, hyp)) in pools {
        let change = hyp.value - base.value;
        if change != 0 {
            movement(
                db,
                a,
                doc,
                None,
                product,
                warehouse,
                0,
                change,
                "revaluation",
                None,
            )?;
            let mut e = Entry::new(finance::account(db, "inventory")?, change);
            e.product = Some(product);
            journal.add(e);
            inventory_total = sum([inventory_total, change])?;
        }
    }
    Ok(inventory_total)
}
pub fn invoice_cost_delta(
    db: &Connection,
    a: &Actor,
    doc: i64,
    line: &Line,
    delta: i64,
    journal: &mut Journal,
) -> Result<()> {
    let matches:Vec<(i64,i64)>=db.prepare("SELECT receipt_line_id,base_quantity_micros FROM purchase_matches WHERE invoice_line_id=?1 ORDER BY id")?.query_map([line.id],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure(!matches.is_empty(), "يجب مطابقة فاتورة المورد مع الاستلام")?;
    let shares = allocate(
        delta.abs(),
        &matches.iter().map(|r| r.1).collect::<Vec<_>>(),
    )?;
    for ((receipt, _), share) in matches.iter().zip(shares) {
        revalue(db, a, doc, *receipt, share * delta.signum(), journal)?;
    }
    Ok(())
}
pub fn landed(
    db: &Connection,
    a: &Actor,
    day: &str,
    supplier: i64,
    description: &str,
    shares: &[CostShare],
) -> Result<Value> {
    ensure(
        !shares.is_empty() && shares.len() <= 1000 && !description.trim().is_empty(),
        "تفاصيل التكلفة الإضافية مطلوبة",
    )?;
    ensure(
        shares.iter().all(|s| s.amount.0 > 0),
        "التكلفة الإضافية يجب أن تكون موجبة",
    )?;
    let mut unique = BTreeSet::new();
    for s in shares {
        ensure(unique.insert(s.receipt_line_id.0), "سطر الاستلام مكرر")?;
    }
    let total = sum(shares.iter().map(|s| s.amount.0))?;
    let doc = documents::header(db, a, "landed_cost", day, Some(supplier), None, description)?;
    finance::set_total(db, doc, total, 0)?;
    db.execute("INSERT INTO landed_charges(document_id,supplier_id,description,amount_minor,allocation_basis) VALUES (?1,?2,?3,?4,'manual')",params![doc,supplier,description,total])?;
    let charge = db.last_insert_rowid();
    // Draft allocation shares are fixed before the immutable posting phase.
    // Replay in a savepoint to calculate shares; rollback provisional rows and recalculate at posting.
    db.execute_batch("SAVEPOINT landed_preview")?;
    documents::begin(db, doc)?;
    let mut preview = Journal::default();
    let mut portions = Vec::new();
    for s in shares {
        portions.push(revalue(
            db,
            a,
            doc,
            s.receipt_line_id.0,
            s.amount.0,
            &mut preview,
        )?);
    }
    db.execute_batch("ROLLBACK TO landed_preview; RELEASE landed_preview")?;
    for (s, inventory) in shares.iter().zip(&portions) {
        ensure(
            *inventory >= 0 && *inventory <= s.amount.0,
            "توزيع التكلفة يحتاج إلى مراجعة حركة الصنف",
        )?;
        db.execute("INSERT INTO landed_allocations(charge_id,receipt_line_id,amount_minor,inventory_share_minor,cogs_share_minor) VALUES (?1,?2,?3,?4,?5)",params![charge,s.receipt_line_id,s.amount,inventory,s.amount.0-inventory])?;
    }
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    for s in shares {
        revalue(db, a, doc, s.receipt_line_id.0, s.amount.0, &mut journal)?;
    }
    journal.add(Entry::party(
        finance::account(db, "payable")?,
        -total,
        supplier,
        day,
    ));
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}
