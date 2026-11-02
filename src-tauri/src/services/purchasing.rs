use super::{documents, inventory};
use crate::{
    db,
    error::{Result, ensure},
    models::{
        arithmetic::{MONEY_MAX, SCALE, allocate, narrow, prorate, sum},
        *,
    },
    state::Actor,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn unique(ids: &[Int], max: usize) -> Result<()> {
    ensure(
        !ids.is_empty()
            && ids.len() <= max
            && ids.iter().all(|id| id.0 > 0)
            && ids.iter().map(|i| i.0).collect::<BTreeSet<_>>().len() == ids.len(),
        "اختر مراجع صالحة دون تكرار ضمن الحد المسموح",
    )
}
pub fn number(row: &Value, key: &str) -> i64 {
    row[key].as_str().unwrap().parse().unwrap()
}
pub fn reason(why: &str) -> Result<()> {
    ensure(
        !why.trim().is_empty() && why.len() <= 2000,
        "سبب العملية مطلوب ولا يتجاوز ألفي حرف",
    )
}
pub fn planning(db: &Connection, a: &Actor, product: i64) -> Result<Value> {
    a.require("products.read")?;
    let mut rows = db::rows(
        db,
        "SELECT p.id AS product_id,p.name_ar,COALESCE(q.revision,0) revision,q.supplier_id,s.name_ar AS supplier_name,COALESCE(q.reorder_point_micros,0) reorder_point_micros,COALESCE(q.max_level_micros,0) max_level_micros,COALESCE(q.lead_time_days,0) lead_time_days,COALESCE(q.weight_grams,0) weight_grams FROM products p LEFT JOIN product_planning q ON q.product_id=p.id LEFT JOIN parties s ON s.id=q.supplier_id WHERE p.id=?1",
        [product],
    )?;
    if let Some(row) = rows.first_mut() {
        row["last_purchase_cost_minor"] = json!(last_cost(db, product, None)?.map(Int));
    }
    Ok(json!(rows))
}
pub fn save_planning(db: &Connection, a: &Actor, p: &ProductPlanning, why: &str) -> Result<Value> {
    a.require("purchases.plan")?;
    reason(why)?;
    ensure(
        p.reorder_point.0 >= 0
            && p.max_level.0 >= p.reorder_point.0
            && p.max_level.0 <= 1_000_000_000_000_000
            && p.lead_time_days <= 3650
            && (0..=1_000_000).contains(&p.weight_grams.0),
        "حدود إعادة الطلب أو المهلة أو الوزن غير صالحة",
    )?;
    if let Some(s) = p.supplier_id {
        ensure(
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM parties WHERE id=?1 AND is_supplier=1 AND active=1)",
                [s],
                |r| r.get::<_, bool>(0),
            )?,
            "المورد الافتراضي غير نشط أو غير صالح",
        )?;
    }
    let old: Option<i64> = db
        .query_row(
            "SELECT revision FROM product_planning WHERE product_id=?1",
            [p.product_id],
            |r| r.get(0),
        )
        .optional()?;
    ensure(
        old.unwrap_or(0) == p.revision.0,
        "تغيرت بيانات التخطيط؛ أعد تحميل الصنف",
    )?;
    db.execute("INSERT INTO product_planning(product_id,supplier_id,reorder_point_micros,max_level_micros,lead_time_days,weight_grams) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(product_id) DO UPDATE SET supplier_id=excluded.supplier_id,reorder_point_micros=excluded.reorder_point_micros,max_level_micros=excluded.max_level_micros,lead_time_days=excluded.lead_time_days,weight_grams=excluded.weight_grams,revision=product_planning.revision+1",params![p.product_id,p.supplier_id,p.reorder_point,p.max_level,p.lead_time_days,p.weight_grams])?;
    Ok(json!({"id":p.product_id}))
}
pub fn prices(db: &Connection, a: &Actor, product: i64, after: i64, limit: u16) -> Result<Value> {
    a.require("purchases.read")?;
    let mut rows = db::rows(
        db,
        "SELECT h.*,p.name_ar AS supplier_name,d.number AS document_number,CASE WHEN EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=h.document_id AND r.status='posted') THEN 'reversed' ELSE 'posted' END status FROM supplier_price_history h JOIN parties p ON p.id=h.supplier_id JOIN documents d ON d.id=h.document_id WHERE h.product_id=?1 AND (?2=0 OR h.purchase_line_id<?2) ORDER BY h.purchase_line_id DESC LIMIT ?3",
        params![product, after, i64::from(limit.clamp(1, 200))],
    )?;
    for row in &mut rows {
        row["base_unit_cost_minor"] = json!(Int(prorate(
            number(row, "net_minor"),
            SCALE,
            number(row, "base_quantity_micros")
        )?));
        row["purchase_unit_cost_minor"] = json!(Int(prorate(
            number(row, "net_minor"),
            SCALE,
            number(row, "quantity_micros")
        )?));
    }
    Ok(json!(rows))
}
pub fn last_cost(db: &Connection, product: i64, supplier: Option<i64>) -> Result<Option<i64>> {
    let row:Option<(i64,i64)>=db.query_row("SELECT h.net_minor,h.base_quantity_micros FROM supplier_price_history h WHERE h.product_id=?1 AND (?2 IS NULL OR h.supplier_id=?2) AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=h.document_id AND r.status='posted') ORDER BY h.document_date DESC,h.purchase_line_id DESC LIMIT 1",params![product,supplier],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    match row {
        Some((net, qty)) => Ok(Some(prorate(net, SCALE, qty)?)),
        None => Ok(None),
    }
}
fn reorder_rows(
    db: &Connection,
    f: &ReorderFilter,
    after: i64,
    limit: i64,
    ids: Option<&[Int]>,
) -> Result<Vec<Value>> {
    documents::date(&f.through)?;
    ensure(
        (1..=365).contains(&f.lookback_days) && (1..=365).contains(&f.coverage_days),
        "فترة تحليل المبيعات أو التغطية غير صالحة",
    )?;
    ensure(
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM warehouses WHERE id=?1 AND active=1)",
            [f.warehouse_id],
            |r| r.get::<_, bool>(0),
        )?,
        "المخزن غير صالح",
    )?;
    let mut rows = db::rows(
        db,
        "SELECT p.id AS product_id,p.code,p.name_ar,p.revision AS product_revision,COALESCE(q.revision,0) planning_revision,q.supplier_id,s.name_ar AS supplier_name,COALESCE(q.reorder_point_micros,0) reorder_point_micros,COALESCE(q.max_level_micros,0) max_level_micros,COALESCE(q.lead_time_days,0) lead_time_days,COALESCE(b.quantity_micros,0) quantity_micros,(SELECT pu.id FROM product_units pu WHERE pu.product_id=p.id AND pu.unit_id=p.base_unit_id AND pu.factor_numerator=pu.factor_denominator AND pu.active=1) AS unit_id FROM products p LEFT JOIN product_planning q ON q.product_id=p.id LEFT JOIN parties s ON s.id=q.supplier_id AND s.active=1 AND s.is_supplier=1 LEFT JOIN stock_balances b ON b.product_id=p.id AND b.warehouse_id=?1 WHERE p.active=1 AND p.id>?2 AND (?3 IS NULL OR p.id IN(SELECT CAST(value AS INTEGER) FROM json_each(?3))) ORDER BY p.id LIMIT ?4",
        params![
            f.warehouse_id,
            after,
            ids.map(serde_json::to_string).transpose()?,
            limit
        ],
    )?;
    for row in &mut rows {
        let product = number(row, "product_id");
        let velocity:i64=db.query_row("SELECT COALESCE(SUM(-m.quantity_micros),0) FROM stock_movements m JOIN documents d ON d.id=m.document_id LEFT JOIN stock_movements original ON original.id=m.reverses_movement_id WHERE m.product_id=?1 AND m.warehouse_id=?2 AND d.status='posted' AND d.document_date BETWEEN date(?3,?4) AND ?3 AND (m.movement_type IN ('sale','sale_return') OR original.movement_type IN ('sale','sale_return'))",params![product,f.warehouse_id,f.through,format!("-{} days",f.lookback_days-1)],|r|r.get(0))?;
        let ordered:i64=db.query_row("SELECT COALESCE(SUM(MAX(0,l.base_quantity_micros-COALESCE((SELECT SUM(rl.base_quantity_micros) FROM receipt_lines r JOIN document_lines rl ON rl.id=r.document_line_id JOIN documents rd ON rd.id=rl.document_id WHERE r.purchase_order_line_id=l.id AND rd.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=rd.id AND rev.status='posted')),0))),0) FROM document_lines l JOIN documents d ON d.id=l.document_id WHERE l.product_id=?1 AND d.warehouse_id=?2 AND d.kind='purchase_order' AND d.status IN ('draft','approved')",params![product,f.warehouse_id],|r|r.get(0))?;
        let lead = number(row, "lead_time_days");
        let ceil_velocity = |days: i64| {
            narrow(
                (i128::from(velocity.max(0)) * i128::from(days) + i128::from(f.lookback_days) - 1)
                    / i128::from(f.lookback_days),
            )
        };
        let threshold = number(row, "reorder_point_micros").max(ceil_velocity(lead)?);
        let target =
            number(row, "max_level_micros").max(ceil_velocity(lead + i64::from(f.coverage_days))?);
        let position = sum([number(row, "quantity_micros"), ordered])?;
        let suggested = if position <= threshold {
            narrow((i128::from(target) - i128::from(position)).max(0))?
        } else {
            0
        };
        ensure(
            suggested <= 1_000_000_000_000_000,
            "الكمية المقترحة تتجاوز الحد المسموح",
        )?;
        let supplier = row["supplier_id"]
            .as_str()
            .map(|v| v.parse::<i64>().unwrap());
        row["net_sales_micros"] = json!(Int(velocity));
        row["on_order_micros"] = json!(Int(ordered));
        row["trigger_micros"] = json!(Int(threshold));
        row["target_micros"] = json!(Int(target));
        row["suggested_micros"] = json!(Int(suggested));
        let last = last_cost(db, product, supplier)?;
        row["last_purchase_cost_minor"] = json!(last.map(Int));
        let suggested_cost = match last {
            Some(cost) => cost,
            None => db.query_row(
                "SELECT standard_cost_minor FROM products WHERE id=?1",
                [product],
                |r| r.get(0),
            )?,
        };
        row["suggested_unit_cost_minor"] = json!(Int(suggested_cost));
    }
    Ok(rows)
}
pub fn reorder(
    db: &Connection,
    a: &Actor,
    f: &ReorderFilter,
    after: i64,
    limit: u16,
) -> Result<Value> {
    a.require("purchases.plan")?;
    Ok(json!(reorder_rows(
        db,
        f,
        after,
        i64::from(limit.clamp(1, 200)),
        None
    )?))
}
fn order_preview(db: &Connection, f: &ReorderFilter, ids: &[Int]) -> Result<(Vec<Value>, String)> {
    unique(ids, 200)?;
    let rows = reorder_rows(db, f, 0, 200, Some(ids))?;
    ensure(rows.len() == ids.len(), "يوجد صنف غير نشط أو غير موجود")?;
    ensure(
        rows.iter().all(|r| {
            number(r, "suggested_micros") > 0
                && r["supplier_name"].is_string()
                && r["unit_id"].is_string()
        }),
        "تحتاج الأصناف المحددة إلى كمية مقترحة ومورد نشط ووحدة أساسية",
    )?;
    let hash = db::digest(serde_json::to_string(&json!([f, rows]))?.as_bytes());
    Ok((rows, hash))
}
pub fn preview_orders(db: &Connection, a: &Actor, f: &ReorderFilter, ids: &[Int]) -> Result<Value> {
    a.require("purchases.plan")?;
    let (rows, hash) = order_preview(db, f, ids)?;
    Ok(json!({"rows":rows,"preview_hash":hash}))
}
pub fn create_orders(
    db: &Connection,
    a: &Actor,
    f: &ReorderFilter,
    ids: &[Int],
    hash: &str,
    why: &str,
) -> Result<Value> {
    a.require("purchases.plan")?;
    a.require("purchases.write")?;
    reason(why)?;
    let (rows, current) = order_preview(db, f, ids)?;
    ensure(hash == current, "تغيرت اقتراحات الشراء؛ أعد المعاينة")?;
    let mut grouped: BTreeMap<i64, Vec<DraftLine>> = BTreeMap::new();
    for row in rows {
        let cost = number(&row, "suggested_unit_cost_minor");
        ensure(
            (0..=MONEY_MAX).contains(&cost),
            "التكلفة الأخيرة تتجاوز حد سعر أمر الشراء",
        )?;
        grouped
            .entry(number(&row, "supplier_id"))
            .or_default()
            .push(DraftLine {
                unit_id: Int(number(&row, "unit_id")),
                quantity: Int(number(&row, "suggested_micros")),
                price: Int(cost),
                discount: Int(0),
                tax_bps: 0,
                order_line_id: None,
                damaged: Int(0),
                short: Int(0),
                matches: vec![],
                notes: String::new(),
            });
    }
    let mut documents = Vec::new();
    for (supplier, lines) in grouped {
        let result = documents::save(
            db,
            a,
            &Draft {
                id: None,
                revision: Int(0),
                kind: DocumentKind::PurchaseOrder,
                date: f.through.clone(),
                due_date: None,
                party_id: Some(Int(supplier)),
                warehouse_id: Some(f.warehouse_id),
                destination_id: None,
                source_id: None,
                wholesale: false,
                discount: Int(0),
                lines,
                payments: vec![],
                notes: "أمر شراء من اقتراح إعادة الطلب؛ راجع السعر والضريبة قبل الاعتماد".into(),
                reason: why.into(),
            },
        )?;
        documents.push(result["id"].clone());
    }
    Ok(json!({"document_ids":documents}))
}

fn landed_preview(
    db: &Connection,
    basis: LandedBasis,
    amount: i64,
    ids: &[Int],
) -> Result<(Vec<Value>, String)> {
    unique(ids, 1000)?;
    ensure(
        (1..=1_000_000_000_000_000).contains(&amount),
        "مبلغ التوزيع غير صالح",
    )?;
    let mut rows = db::rows(
        db,
        "SELECT l.id AS receipt_line_id,l.product_name_snapshot,l.base_quantity_micros,l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor AS receipt_value_minor,COALESCE(p.weight_grams,0) weight_grams FROM document_lines l JOIN documents d ON d.id=l.document_id LEFT JOIN product_planning p ON p.product_id=l.product_id WHERE l.id IN(SELECT CAST(value AS INTEGER) FROM json_each(?1)) AND d.kind='goods_receipt' AND d.status='posted' AND l.base_quantity_micros>0 AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted') ORDER BY l.id",
        [serde_json::to_string(ids)?],
    )?;
    ensure(
        rows.len() == ids.len(),
        "اختر سطور استلام موجبة ومرحلة وغير معكوسة",
    )?;
    let weights = rows
        .iter()
        .map(|r| match basis {
            LandedBasis::Value => Ok(number(r, "receipt_value_minor")),
            LandedBasis::Quantity => Ok(number(r, "base_quantity_micros")),
            LandedBasis::Weight => {
                ensure(
                    number(r, "weight_grams") > 0,
                    "أدخل وزن الوحدة الأساسية لكل صنف قبل التوزيع بالوزن",
                )?;
                narrow(
                    i128::from(number(r, "base_quantity_micros"))
                        * i128::from(number(r, "weight_grams")),
                )
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let shares = allocate(amount, &weights)?;
    for ((row, share), weight) in rows.iter_mut().zip(shares).zip(weights) {
        row["amount_minor"] = json!(Int(share));
        row["allocation_weight"] = json!(Int(weight));
    }
    let hash = db::digest(serde_json::to_string(&json!([basis, amount, rows]))?.as_bytes());
    Ok((rows, hash))
}
pub fn preview_landed(
    db: &Connection,
    a: &Actor,
    basis: LandedBasis,
    amount: i64,
    ids: &[Int],
) -> Result<Value> {
    a.require("purchases.write")?;
    let (rows, hash) = landed_preview(db, basis, amount, ids)?;
    Ok(json!({"rows":rows,"preview_hash":hash}))
}
pub fn post_landed(
    db: &Connection,
    a: &Actor,
    day: &str,
    supplier: i64,
    description: &str,
    basis: LandedBasis,
    amount: i64,
    ids: &[Int],
    hash: &str,
) -> Result<Value> {
    a.require("purchases.write")?;
    let (rows, current) = landed_preview(db, basis, amount, ids)?;
    ensure(hash == current, "تغيرت أسس توزيع التكلفة؛ أعد المعاينة")?;
    let shares = rows
        .iter()
        .filter(|r| number(r, "amount_minor") > 0)
        .map(|r| CostShare {
            receipt_line_id: Int(number(r, "receipt_line_id")),
            amount: Int(number(r, "amount_minor")),
        })
        .collect::<Vec<_>>();
    // Existing revalue() replay and exact COGS/inventory split remain the sole posting path.
    let result = inventory::landed(db, a, day, supplier, description, &shares)?;
    db.execute("INSERT INTO landed_calculations(document_id,basis,amount_minor,inputs_json,preview_hash) VALUES (?1,?2,?3,?4,?5)",params![result["id"].as_str().unwrap().parse::<i64>().unwrap(),basis.code(),amount,serde_json::to_string(&rows)?,current])?;
    Ok(result)
}
