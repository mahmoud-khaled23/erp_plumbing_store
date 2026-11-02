use super::{
    costing::Pool,
    inventory,
    purchasing::{number, reason, unique},
};
use crate::{
    db,
    error::{Result, ensure},
    models::{
        arithmetic::{MONEY_MAX, SCALE, allocate, convert, narrow, prorate, sum},
        *,
    },
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn policy(db: &Connection, a: &Actor) -> Result<Value> {
    a.require("products.read")?;
    Ok(json!(db::rows(db, "SELECT * FROM pricing_policy", [])?))
}
pub fn save_policy(
    db: &Connection,
    a: &Actor,
    revision: i64,
    margin: u16,
    why: &str,
) -> Result<Value> {
    a.require("pricing.manage")?;
    reason(why)?;
    ensure(margin <= 10000, "الهامش يجب أن يكون بين صفر ومائة بالمائة")?;
    ensure(db.execute("UPDATE pricing_policy SET minimum_margin_bps=?1,revision=revision+1 WHERE id=1 AND revision=?2",params![margin,revision])?==1,"تغيرت سياسة الهامش؛ أعد تحميلها")?;
    Ok(json!({"id":"1"}))
}
pub fn units(db: &Connection, a: &Actor, search: &str, after: i64, limit: u16) -> Result<Value> {
    a.require("products.price_update")?;
    ensure(search.len() <= 256, "نص البحث طويل")?;
    Ok(json!(db::rows(
        db,
        "SELECT u.id AS unit_id,p.id AS product_id,p.code,p.name_ar,n.name_ar AS unit_name,u.retail_price_minor,u.wholesale_price_minor FROM product_units u JOIN products p ON p.id=u.product_id JOIN units n ON n.id=u.unit_id WHERE u.active=1 AND p.active=1 AND u.id>?1 AND (?2='' OR instr(p.normalized_name,?2)>0 OR instr(p.code,?3)>0 OR instr(COALESCE(u.barcode,''),?3)>0) ORDER BY u.id LIMIT ?4",
        params![
            after,
            super::catalog::normalize(search),
            search,
            i64::from(limit.clamp(1, 200))
        ]
    )?))
}
fn price_preview(db: &Connection, ids: &[Int], rule: &PriceRule) -> Result<(Vec<Value>, String)> {
    unique(ids, 500)?;
    ensure(
        (-10000..=1_000_000).contains(&rule.rate_bps.0)
            && (rule.basis != PriceBasis::CostMarkup || rule.rate_bps.0 >= 0),
        "نسبة تعديل السعر غير صالحة",
    )?;
    let mut rows = db::rows(
        db,
        "SELECT u.id AS unit_id,u.product_id,p.code,p.name_ar,p.revision,u.factor_numerator,u.factor_denominator,p.standard_cost_minor,n.name_ar AS unit_name,u.retail_price_minor AS old_retail_minor,u.wholesale_price_minor AS old_wholesale_minor FROM product_units u JOIN products p ON p.id=u.product_id JOIN units n ON n.id=u.unit_id WHERE u.id IN(SELECT CAST(value AS INTEGER) FROM json_each(?1)) AND u.active=1 AND p.active=1 ORDER BY u.id",
        [serde_json::to_string(ids)?],
    )?;
    ensure(rows.len() == ids.len(), "يوجد صنف أو وحدة غير نشطة")?;
    for row in &mut rows {
        let qty = convert(
            SCALE,
            number(row, "factor_numerator"),
            number(row, "factor_denominator"),
        )?;
        let (stock_qty,stock_value):(i64,i64)=db.query_row("SELECT COALESCE(SUM(quantity_micros),0),COALESCE(SUM(value_minor),0) FROM stock_balances WHERE product_id=?1",[number(row,"product_id")],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let cost = if stock_qty > 0 && stock_value >= 0 {
            prorate(stock_value, qty, stock_qty)?
        } else {
            prorate(number(row, "standard_cost_minor"), qty, SCALE)?
        };
        let updated = |old: i64| {
            prorate(
                if rule.basis == PriceBasis::CostMarkup {
                    cost
                } else {
                    old
                },
                10000 + rule.rate_bps.0,
                10000,
            )
        };
        let retail = if rule.price_list == PriceList::Wholesale {
            number(row, "old_retail_minor")
        } else {
            updated(number(row, "old_retail_minor"))?
        };
        let wholesale = if rule.price_list == PriceList::Retail {
            number(row, "old_wholesale_minor")
        } else {
            updated(number(row, "old_wholesale_minor"))?
        };
        ensure(
            (0..=MONEY_MAX).contains(&retail) && (0..=MONEY_MAX).contains(&wholesale),
            "السعر الجديد يتجاوز الحد المسموح",
        )?;
        row["unit_cost_minor"] = json!(Int(cost));
        row["new_retail_minor"] = json!(Int(retail));
        row["new_wholesale_minor"] = json!(Int(wholesale));
    }
    let hash = db::digest(serde_json::to_string(&json!([rule, rows]))?.as_bytes());
    Ok((rows, hash))
}
pub fn preview(db: &Connection, a: &Actor, ids: &[Int], rule: &PriceRule) -> Result<Value> {
    a.require("products.price_update")?;
    let (rows, hash) = price_preview(db, ids, rule)?;
    Ok(json!({"rows":rows,"preview_hash":hash}))
}
pub fn apply(
    db: &Connection,
    a: &Actor,
    ids: &[Int],
    rule: &PriceRule,
    hash: &str,
    why: &str,
) -> Result<Value> {
    a.require("products.price_update")?;
    reason(why)?;
    let (rows, current) = price_preview(db, ids, rule)?;
    ensure(hash == current, "تغيرت الأسعار أو التكلفة؛ أعد المعاينة")?;
    db.execute("INSERT INTO price_update_batches(rule_json,preview_hash,reason,created_by) VALUES (?1,?2,?3,?4)",params![serde_json::to_string(rule)?,current,why,a.user_id])?;
    let batch = db.last_insert_rowid();
    let mut products = BTreeSet::new();
    for row in rows {
        let (unit, retail, wholesale) = (
            number(&row, "unit_id"),
            number(&row, "new_retail_minor"),
            number(&row, "new_wholesale_minor"),
        );
        db.execute("INSERT INTO price_update_lines(batch_id,product_unit_id,old_retail_minor,new_retail_minor,old_wholesale_minor,new_wholesale_minor) VALUES (?1,?2,?3,?4,?5,?6)",params![batch,unit,number(&row,"old_retail_minor"),retail,number(&row,"old_wholesale_minor"),wholesale])?;
        db.execute(
            "UPDATE product_units SET retail_price_minor=?1,wholesale_price_minor=?2 WHERE id=?3",
            params![retail, wholesale, unit],
        )?;
        products.insert(number(&row, "product_id"));
    }
    for product in products {
        db.execute(
            "UPDATE products SET revision=revision+1 WHERE id=?1",
            [product],
        )?;
    }
    Ok(json!({"id":Int(batch),"count":Int(ids.len() as i64)}))
}
pub fn below_margin(net: i64, cost: i64, minimum: i64) -> bool {
    net < cost
        || (net == 0 && minimum > 0)
        || (i128::from(net) - i128::from(cost)) * 10000 < i128::from(net) * i128::from(minimum)
}
pub fn enforce(
    db: &Connection,
    a: &Actor,
    doc: i64,
    line: &super::documents::Line,
    cost: i64,
) -> Result<()> {
    let minimum: i64 = db.query_row(
        "SELECT minimum_margin_bps FROM pricing_policy WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    let net = sum([line.gross, -line.discount, -line.share])?;
    let override_needed = below_margin(net, cost, minimum);
    let why: String = db.query_row("SELECT reason FROM documents WHERE id=?1", [doc], |r| {
        r.get(0)
    })?;
    if override_needed {
        a.require("sales.discount")?;
        ensure(
            !why.trim().is_empty() && why.len() <= 2000,
            "البيع دون التكلفة أو الحد الأدنى للهامش يتطلب صلاحية تجاوز السعر وسبباً",
        )?;
    }
    db.execute("INSERT INTO sale_margin_checks(document_line_id,document_id,net_minor,cost_minor,minimum_margin_bps,override_user_id,reason) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![line.id,doc,net,cost,minimum,override_needed.then_some(a.user_id),if override_needed{why.as_str()}else{""}])?;
    Ok(())
}
pub fn margins(
    db: &Connection,
    a: &Actor,
    warehouse: i64,
    lines: &[DraftLine],
    discount: i64,
) -> Result<Value> {
    a.require("sales.write")?;
    super::quarantine::sale_warehouse(db, warehouse)?;
    ensure(
        !lines.is_empty() && lines.len() <= 1000 && discount >= 0,
        "بيانات معاينة الهامش غير صالحة",
    )?;
    ensure(
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM warehouses WHERE id=?1 AND active=1)",
            [warehouse],
            |r| r.get::<_, bool>(0),
        )?,
        "المخزن غير صالح",
    )?;
    let mut nets = Vec::new();
    for l in lines {
        ensure(
            (1..=1_000_000_000_000_000).contains(&l.quantity.0)
                && (0..=MONEY_MAX).contains(&l.price.0)
                && l.discount.0 >= 0,
            "كمية أو سعر أو خصم غير صالح",
        )?;
        let gross = prorate(l.price.0, l.quantity.0, SCALE)?;
        ensure(
            gross <= MONEY_MAX && l.discount.0 <= gross,
            "صافي سطر البيع غير صالح",
        )?;
        nets.push(gross - l.discount.0);
    }
    ensure(
        discount <= sum(nets.iter().copied())?,
        "الخصم يتجاوز الإجمالي",
    )?;
    let shares = allocate(discount, &nets)?;
    let minimum: i64 = db.query_row(
        "SELECT minimum_margin_bps FROM pricing_policy WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    let mut pools: BTreeMap<i64, Pool> = BTreeMap::new();
    let mut rows = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let (product,num,den,fallback):(i64,i64,i64,i64)=db.query_row("SELECT p.id,u.factor_numerator,u.factor_denominator,p.standard_cost_minor FROM product_units u JOIN products p ON p.id=u.product_id WHERE u.id=?1 AND u.active=1 AND p.active=1",[l.unit_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let pool = match pools.entry(product) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(inventory::pool(db, product, warehouse)?)
            }
        };
        let qty = convert(l.quantity.0, num, den)?;
        let sufficient = pool.quantity >= qty;
        // Preview can show provisional cost despite insufficient stock. Posting still
        // independently enforces the store's negative-stock setting and actual issue cost.
        let cost = pool.issue(qty, fallback, true)?;
        let net = nets[i] - shares[i];
        let profit = narrow(i128::from(net) - i128::from(cost))?;
        let bps = if net > 0 {
            prorate(profit, 10000, net).ok().map(Int)
        } else {
            None
        };
        rows.push(json!({"line_number":Int((i+1)as i64),"cost_minor":Int(cost),"net_minor":Int(net),"margin_minor":Int(profit),"margin_bps":bps,"minimum_margin_bps":Int(minimum),"override_required":if below_margin(net,cost,minimum){"1"}else{"0"},"sufficient_stock":if sufficient{"1"}else{"0"}}));
    }
    Ok(json!(rows))
}
