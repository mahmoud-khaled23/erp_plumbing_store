//! Read one consistent, permission-checked print snapshot in the caller's transaction.
use super::reporting;
use crate::{
    db,
    error::{Result, ensure},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub const MAX_PRINT_ROWS: usize = 10_000;

pub fn snapshot(db: &Connection, actor: &Actor, source: &PrintSource) -> Result<Value> {
    let mut result = json!({"source":source,"document":null,"rows":[],"opening":[],"context":[],"references":{},"store":{},"preferences":{}});
    match source {
        PrintSource::Document { id } => {
            let doc = reporting::query(db, actor, &Query::Document { id: *id })?;
            let header = &doc["header"][0];
            let stored: Value =
                serde_json::from_str(header["store_snapshot_json"].as_str().unwrap_or("{}"))?;
            result["store"] = stored;
            result["context"] = json!(db::rows(
                db,
                "SELECT d.id,w.name_ar warehouse_name,dw.name_ar destination_name,src.number source_number,src.kind source_kind,rev.number reversal_number,u.display_name operator_name FROM documents d LEFT JOIN warehouses w ON w.id=d.warehouse_id LEFT JOIN warehouses dw ON dw.id=d.destination_warehouse_id LEFT JOIN documents src ON src.id=COALESCE(d.source_document_id,d.reversal_of_id) LEFT JOIN documents rev ON rev.reversal_of_id=d.id AND rev.status='posted' LEFT JOIN users u ON u.id=COALESCE(d.posted_by,d.created_by) WHERE d.id=?1",
                [id]
            )?);
            result["document"] = doc;
        }
        PrintSource::Report {
            report,
            from,
            to,
            party_id,
        } => {
            actor.require("reports.read")?;
            ensure(
                !matches!(report, Report::PartyStatement) || party_id.is_some(),
                "اختر الطرف قبل طباعة كشف الحساب",
            )?;
            let value = reporting::report(
                db,
                *report,
                from,
                to,
                party_id.map(|i| i.0),
                (MAX_PRINT_ROWS + 1) as i64,
                0,
            )?;
            result["rows"] = if value.is_array() {
                value
            } else {
                result["opening"] = value["opening"].clone();
                value["rows"].clone()
            };
            if let Some(id) = party_id {
                result["context"] = json!(db::rows(
                    db,
                    "SELECT name_ar,code,address,tax_number,phone FROM parties WHERE id=?1",
                    [id]
                )?);
            }
        }
        PrintSource::Labels { unit_ids } => {
            actor.require("products.read")?;
            ensure(
                !unit_ids.is_empty() && unit_ids.len() <= 100,
                "اختر من وحدة واحدة إلى مائة وحدة للملصقات",
            )?;
            let mut seen = std::collections::BTreeSet::new();
            let mut rows = Vec::new();
            for id in unit_ids {
                ensure(seen.insert(id.0), "الوحدة مكررة في طلب الملصقات")?;
                let mut unit = db::rows(
                    db,
                    "SELECT pu.id,pu.barcode,pu.retail_price_minor,pu.wholesale_price_minor,p.code,p.name_ar,u.name_ar unit_name FROM product_units pu JOIN products p ON p.id=pu.product_id JOIN units u ON u.id=pu.unit_id WHERE pu.id=?1 AND pu.active=1 AND p.active=1 AND pu.barcode IS NOT NULL",
                    [id],
                )?;
                ensure(unit.len() == 1, "وحدة الملصق غير نشطة أو بلا باركود")?;
                rows.append(&mut unit);
            }
            result["rows"] = json!(rows);
        }
        PrintSource::StockCard {
            product_id,
            warehouse_id,
        } => {
            actor.require("inventory.read")?;
            result["context"] = json!(db::rows(
                db,
                "SELECT p.name_ar,p.code,w.name_ar warehouse_name FROM products p CROSS JOIN warehouses w WHERE p.id=?1 AND w.id=?2",
                params![product_id, warehouse_id]
            )?);
            ensure(
                result["context"].as_array().is_some_and(|v| !v.is_empty()),
                "الصنف أو المخزن غير موجود",
            )?;
            result["rows"] = json!(db::rows(
                db,
                "SELECT * FROM v_stock_card WHERE product_id=?1 AND warehouse_id=?2 ORDER BY id LIMIT ?3",
                params![product_id, warehouse_id, (MAX_PRINT_ROWS + 1) as i64]
            )?);
        }
    }
    ensure(
        result["rows"]
            .as_array()
            .is_some_and(|rows| rows.len() <= MAX_PRINT_ROWS),
        "يتجاوز التقرير عشرة آلاف صف؛ ضيق نطاق التقرير قبل الطباعة",
    )?;
    let settings = db::rows(
        db,
        "SELECT store_name,logo_path,tax_number,address,phones,currency_code,currency_symbol,currency_minor_digits,digit_style,timezone,receipt_footer,print_copies,default_print_layout FROM store_settings WHERE id=1",
        [],
    )?;
    result["preferences"] = json!(settings[0]);
    if !matches!(source, PrintSource::Document { .. }) {
        result["store"] = json!(settings[0]);
    }
    for table in ["warehouses", "cash_accounts", "units"] {
        result["references"][table] = json!(db::rows(
            db,
            &format!("SELECT id,name_ar FROM {table}"),
            []
        )?);
    }
    if actor.permissions.contains("finance.read") || actor.permissions.contains("reports.read") {
        result["references"]["accounts"] =
            json!(db::rows(db, "SELECT id,name_ar,code FROM accounts", [])?);
    }
    // Enrich just the parties present in this snapshot, rather than loading a large directory.
    let parties = result["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["party_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut party_rows = Vec::new();
    for id in parties {
        party_rows.extend(db::rows(
            db,
            "SELECT id,name_ar FROM parties WHERE id=?1",
            [id],
        )?);
    }
    result["references"]["parties"] = json!(party_rows);
    result["generated_at"] = json!(chrono::Utc::now().to_rfc3339());
    Ok(result)
}
