use super::*;
use crate::{
    db,
    models::arithmetic::{narrow, prorate, sum},
};
use rusqlite::params;
use std::collections::VecDeque;

const POOLS:&str="WITH pools AS (
 SELECT s.product_id,s.warehouse_id,SUM(s.quantity_micros) quantity_micros,SUM(s.value_minor) value_minor
 FROM stock_movements s JOIN documents d ON d.id=s.document_id
 WHERE d.status='posted' AND d.document_date<=?2 GROUP BY s.product_id,s.warehouse_id
)";

pub(super) fn run(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    match f.report {
        Report::StockAbc => abc(db, f, limit, offset),
        Report::StockActivity => activity(db, f, limit, offset),
        Report::StockAging => aging(db, f, limit, offset),
        Report::StockTurnover => turnover(db, f, limit, offset),
        _ => Err(Error::validation("نوع تحليل المخزون غير صالح")),
    }
}
fn abc(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    let mut rows=db::rows(db,"WITH sales AS (
 SELECT product_id,SUM(net_sales_minor) net_sales_minor FROM v_product_profit WHERE document_date BETWEEN ?1 AND ?2 GROUP BY product_id
), ranked AS (
 SELECT p.id product_id,p.code,p.name_ar,COALESCE(s.net_sales_minor,0) net_sales_minor,
 SUM(MAX(COALESCE(s.net_sales_minor,0),0)) OVER () total_sales_minor,
 COALESCE(SUM(MAX(COALESCE(s.net_sales_minor,0),0)) OVER (ORDER BY COALESCE(s.net_sales_minor,0) DESC,p.id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING),0) preceding_sales_minor
 FROM products p LEFT JOIN sales s ON s.product_id=p.id WHERE p.active=1 OR COALESCE(s.net_sales_minor,0)<>0
) SELECT *,'product:'||product_id drill_key FROM ranked ORDER BY net_sales_minor DESC,product_id LIMIT ?3 OFFSET ?4",params![f.from,f.to,limit,offset])?;
    for row in &mut rows {
        let (net, total, before) = (
            integer(row, "net_sales_minor")?,
            integer(row, "total_sales_minor")?,
            integer(row, "preceding_sales_minor")?,
        );
        let class = if net <= 0 || total <= 0 {
            "abc_c"
        } else if i128::from(before) * 100 < i128::from(total) * 80 {
            "abc_a"
        } else if i128::from(before) * 100 < i128::from(total) * 95 {
            "abc_b"
        } else {
            "abc_c"
        };
        row["stock_class"] = json!(class);
        row["share_bps"] = json!(Int(if total > 0 {
            prorate(net.max(0), 10000, total)?
        } else {
            0
        }));
        row["cumulative_bps"] = json!(Int(if total > 0 {
            prorate(sum([before, net.max(0)])?, 10000, total)?
        } else {
            0
        }));
    }
    Ok(output(rows, vec![]))
}
fn activity(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    let mut rows=db::rows(db,&format!("{POOLS}, last_sales AS (
 SELECT s.product_id,s.warehouse_id,MAX(d.document_date) last_sale_date FROM stock_movements s JOIN documents d ON d.id=s.document_id
 WHERE d.status='posted' AND s.movement_type='sale' AND d.document_date<=?2
 AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted' AND r.document_date<=?2)
 GROUP BY s.product_id,s.warehouse_id
) SELECT b.*,p.code,p.name_ar,w.name_ar warehouse_name,l.last_sale_date,
 CAST(julianday(?2)-julianday(l.last_sale_date) AS INTEGER) days_since_sale,
 'pool:'||b.product_id||':'||b.warehouse_id drill_key
 FROM pools b JOIN products p ON p.id=b.product_id JOIN warehouses w ON w.id=b.warehouse_id
 LEFT JOIN last_sales l ON l.product_id=b.product_id AND l.warehouse_id=b.warehouse_id
 WHERE (b.quantity_micros<>0 OR b.value_minor<>0) AND ?1 IS NOT NULL
 ORDER BY l.last_sale_date IS NOT NULL,l.last_sale_date,b.product_id,b.warehouse_id LIMIT ?3 OFFSET ?4"),params![f.from,f.to,limit,offset])?;
    for row in &mut rows {
        row["stock_activity"] = json!(if integer(row, "quantity_micros")? < 0 {
            "negative_stock"
        } else if row["days_since_sale"].is_null() {
            "never_sold"
        } else {
            let days = integer(row, "days_since_sale")?;
            if days >= 180 {
                "dead_stock"
            } else if days >= 90 {
                "slow_stock"
            } else {
                "moving_stock"
            }
        });
    }
    Ok(output(rows, vec![]))
}

fn aging(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    let mut rows = db::rows(
        db,
        &format!(
            "{POOLS} SELECT b.*,p.code,p.name_ar,w.name_ar warehouse_name,'pool:'||b.product_id||':'||b.warehouse_id drill_key FROM pools b JOIN products p ON p.id=b.product_id JOIN warehouses w ON w.id=b.warehouse_id WHERE (b.quantity_micros<>0 OR b.value_minor<>0) AND ?1 IS NOT NULL ORDER BY b.product_id,b.warehouse_id LIMIT ?3 OFFSET ?4"
        ),
        params![f.from, f.to, limit, offset],
    )?;
    let as_of = super::super::documents::date(&f.to)?;
    for row in &mut rows {
        let mut lots = VecDeque::<(i64, String)>::new();
        let mut deficit = 0i64;
        let mut stmt=db.prepare_cached("SELECT s.quantity_micros,d.document_date FROM stock_movements s JOIN documents d ON d.id=s.document_id WHERE s.product_id=?1 AND s.warehouse_id=?2 AND d.status='posted' AND d.document_date<=?3 ORDER BY s.id")?;
        let mut movements = stmt.query(params![
            integer(row, "product_id")?,
            integer(row, "warehouse_id")?,
            f.to
        ])?;
        while let Some(m) = movements.next()? {
            let q: i64 = m.get(0)?;
            if q > 0 {
                let covered = q.min(narrow(-i128::from(deficit))?);
                deficit = sum([deficit, covered])?;
                if q > covered {
                    lots.push_back((q - covered, m.get(1)?));
                }
            } else if q < 0 {
                let mut remaining = narrow(-i128::from(q))?;
                while remaining > 0 {
                    let Some((quantity, _)) = lots.front_mut() else {
                        break;
                    };
                    let used = remaining.min(*quantity);
                    *quantity -= used;
                    remaining -= used;
                    if *quantity == 0 {
                        lots.pop_front();
                    }
                }
                deficit = sum([deficit, -remaining])?;
            }
        }
        let mut buckets = [0i64; 4];
        for (quantity, day) in lots {
            let days = (as_of - super::super::documents::date(&day)?).num_days();
            let bucket = if days <= 30 {
                0
            } else if days <= 60 {
                1
            } else if days <= 90 {
                2
            } else {
                3
            };
            buckets[bucket] = sum([buckets[bucket], quantity])?;
        }
        ensure(
            sum(buckets.into_iter().chain([deficit]))? == integer(row, "quantity_micros")?,
            "تعذر مطابقة أعمار المخزون مع الكمية",
        )?;
        for (key, value) in [
            "age_0_30_micros",
            "age_31_60_micros",
            "age_61_90_micros",
            "age_over_90_micros",
        ]
        .into_iter()
        .zip(buckets)
        {
            row[key] = json!(Int(value));
        }
        row["negative_quantity_micros"] = json!(Int(deficit));
    }
    Ok(output(rows, vec![]))
}

fn turnover(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    let mut rows=db::rows(db,"WITH valuation AS (
 SELECT s.product_id,SUM(CASE WHEN d.document_date<?1 THEN s.value_minor ELSE 0 END) opening_value_minor,SUM(s.value_minor) closing_value_minor
 FROM stock_movements s JOIN documents d ON d.id=s.document_id WHERE d.status='posted' AND d.document_date<=?2 GROUP BY s.product_id
), costs AS (SELECT product_id,SUM(cogs_minor) cogs_minor FROM v_product_profit WHERE document_date BETWEEN ?1 AND ?2 GROUP BY product_id)
 SELECT p.id product_id,p.code,p.name_ar,COALESCE(v.opening_value_minor,0) opening_value_minor,COALESCE(v.closing_value_minor,0) closing_value_minor,COALESCE(c.cogs_minor,0) cogs_minor,'product:'||p.id drill_key
 FROM products p LEFT JOIN valuation v ON v.product_id=p.id LEFT JOIN costs c ON c.product_id=p.id
 WHERE v.product_id IS NOT NULL OR c.product_id IS NOT NULL ORDER BY p.id LIMIT ?3 OFFSET ?4",params![f.from,f.to,limit,offset])?;
    let days = (super::super::documents::date(&f.to)? - super::super::documents::date(&f.from)?)
        .num_days()
        + 1;
    for row in &mut rows {
        let (opening, closing, cogs) = (
            integer(row, "opening_value_minor")?,
            integer(row, "closing_value_minor")?,
            integer(row, "cogs_minor")?,
        );
        let combined = sum([opening, closing])?;
        row["average_value_minor"] = json!(Int(prorate(combined, 1, 2)?));
        row["turnover_milli"] = if opening >= 0 && closing >= 0 && combined > 0 && cogs >= 0 {
            json!(Int(prorate(cogs, 2000, combined)?))
        } else {
            Value::Null
        };
        row["inventory_days_milli"] = if opening >= 0 && closing >= 0 && combined > 0 && cogs > 0 {
            json!(Int(prorate(combined, days * 500, cogs)?))
        } else {
            Value::Null
        };
    }
    Ok(output(rows, vec![]))
}
