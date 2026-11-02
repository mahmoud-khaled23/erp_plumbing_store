use super::*;
use crate::{db, models::arithmetic::narrow, state::Actor};
use rusqlite::params;

const JOURNAL: &str = "SELECT document_id,document_number,document_kind,document_date,account_code,account_name,debit_minor,credit_minor,due_date FROM v_journal";
const JOURNAL_COLUMNS: &[&str] = &[
    "document_date",
    "document_number",
    "document_kind",
    "account_code",
    "account_name",
    "debit_minor",
    "credit_minor",
    "due_date",
];
fn id(value: Option<&&str>) -> Result<i64> {
    let number = value.and_then(|v| v.parse::<i64>().ok()).filter(|v| *v > 0);
    number.ok_or_else(|| Error::validation("مرجع تفاصيل التقرير غير صالح"))
}
fn result(rows: Vec<Value>, columns: &[&str]) -> Value {
    json!({"rows":rows,"columns":columns})
}

pub fn drilldown(
    db: &Connection,
    actor: &Actor,
    f: &ReportFilter,
    key: &str,
    limit: u16,
    offset: u32,
) -> Result<Value> {
    actor.require("reports.read")?;
    actor.require("reports.drilldown")?;
    validate(f)?;
    ensure(key.len() <= 128, "مرجع التقرير طويل")?;
    let parts = key.split(':').collect::<Vec<_>>();
    let limit = i64::from(limit.clamp(1, 200));
    let offset = i64::from(offset);
    match f.report {
        Report::BalanceSheet => {
            actor.require("finance.read")?;
            let rows = match parts.as_slice() {
                ["account", _] => db::rows(
                    db,
                    &format!(
                        "{JOURNAL} WHERE account_id=?1 AND document_date<=?2 ORDER BY document_date,id LIMIT ?3 OFFSET ?4"
                    ),
                    params![id(parts.get(1))?, f.to, limit, offset],
                )?,
                ["result", period @ ("prior" | "current")] => {
                    let start = financial::period_start(db, &f.to)?;
                    db::rows(
                        db,
                        &format!(
                            "{} SELECT document_id,document_number,document_kind,document_date,account_code,account_name,debit_minor,credit_minor,due_date FROM result_journal WHERE account_kind IN ('income','expense') AND document_date<=?1 AND ((?2='prior' AND result_date<?3) OR (?2='current' AND result_date>=?3)) ORDER BY document_date,id LIMIT ?4 OFFSET ?5",
                            financial::RESULT_JOURNAL
                        ),
                        params![f.to, period, start, limit, offset],
                    )?
                }
                _ => return Err(Error::validation("مرجع الميزانية غير صالح")),
            };
            Ok(result(rows, JOURNAL_COLUMNS))
        }
        Report::InventoryReconciliation => {
            actor.require("finance.read")?;
            ensure(key == "reconciliation:all", "مرجع المطابقة غير صالح")?;
            let mut rows=db::rows(db,"WITH stock AS (SELECT document_id,SUM(value_minor) stock_value_minor FROM stock_movements GROUP BY document_id),ledger AS (SELECT j.document_id,SUM(j.debit_minor-j.credit_minor) ledger_value_minor FROM journal_lines j JOIN accounts a ON a.id=j.account_id WHERE a.control_kind='inventory' GROUP BY j.document_id)
 SELECT d.id document_id,d.number document_number,d.kind document_kind,d.document_date,COALESCE(s.stock_value_minor,0) stock_value_minor,COALESCE(l.ledger_value_minor,0) ledger_value_minor
 FROM documents d LEFT JOIN stock s ON s.document_id=d.id LEFT JOIN ledger l ON l.document_id=d.id
 WHERE d.status='posted' AND d.document_date<=?1 AND (s.document_id IS NOT NULL OR l.document_id IS NOT NULL)
 ORDER BY (COALESCE(s.stock_value_minor,0)<>COALESCE(l.ledger_value_minor,0)) DESC,d.document_date,d.id LIMIT ?2 OFFSET ?3",params![f.to,limit,offset])?;
            for row in &mut rows {
                row["difference_minor"] = json!(Int(narrow(
                    i128::from(integer(row, "stock_value_minor")?)
                        - i128::from(integer(row, "ledger_value_minor")?)
                )?));
            }
            Ok(result(
                rows,
                &[
                    "document_date",
                    "document_number",
                    "document_kind",
                    "stock_value_minor",
                    "ledger_value_minor",
                    "difference_minor",
                ],
            ))
        }
        Report::DiscountsGranted => {
            actor.require("finance.read")?;
            ensure(
                parts.len() == 2 && parts[0] == "document",
                "مرجع الخصم غير صالح",
            )?;
            let rows = db::rows(
                db,
                &format!(
                    "{JOURNAL} WHERE document_id=?1 AND document_date BETWEEN ?2 AND ?3 AND (?4 IS NULL OR document_id IN (SELECT id FROM documents WHERE party_id=?4)) ORDER BY id LIMIT ?5 OFFSET ?6"
                ),
                params![id(parts.get(1))?, f.from, f.to, f.party_id, limit, offset],
            )?;
            Ok(result(rows, JOURNAL_COLUMNS))
        }
        Report::VatReturn => {
            actor.require("finance.read")?;
            ensure(
                parts.len() == 3 && parts[0] == "vat" && ["input", "output"].contains(&parts[1]),
                "مرجع الضريبة غير صالح",
            )?;
            let rate = if parts[2] == "unclassified" {
                None
            } else {
                Some(
                    parts[2]
                        .parse::<u16>()
                        .ok()
                        .filter(|v| *v <= 10000)
                        .ok_or_else(|| Error::validation("نسبة الضريبة غير صالحة"))?,
                )
            };
            let rows = db::rows(
                db,
                &format!(
                    "{} SELECT document_id,document_number,document_kind,document_date,taxable_minor,tax_minor FROM items WHERE vat_side=?3 AND tax_rate_bps IS ?4 ORDER BY document_date,document_id LIMIT ?5 OFFSET ?6",
                    financial::VAT_ITEMS
                ),
                params![f.from, f.to, parts[1], rate, limit, offset],
            )?;
            Ok(result(
                rows,
                &[
                    "document_date",
                    "document_number",
                    "document_kind",
                    "taxable_minor",
                    "tax_minor",
                ],
            ))
        }
        Report::Collections => {
            actor.require("finance.read")?;
            ensure(
                parts.len() == 2 && parts[0] == "party",
                "مرجع العميل غير صالح",
            )?;
            let party = id(parts.get(1))?;
            ensure(
                f.party_id.is_none_or(|p| p.0 == party),
                "العميل لا يطابق مرشح التقرير",
            )?;
            let rows=db::rows(db,"SELECT * FROM (SELECT j.document_id,j.document_number,j.document_kind,j.document_date,j.due_date,j.debit_minor,j.credit_minor,
 CASE WHEN j.debit_minor>0 THEN j.debit_minor-COALESCE((SELECT SUM(s.amount_minor) FROM settlements s JOIN documents d ON d.id=s.document_id WHERE s.debit_line_id=j.id AND d.status='posted' AND d.document_date<=?2),0)
 ELSE -j.credit_minor+COALESCE((SELECT SUM(s.amount_minor) FROM settlements s JOIN documents d ON d.id=s.document_id WHERE s.credit_line_id=j.id AND d.status='posted' AND d.document_date<=?2),0) END outstanding_signed_minor
 FROM v_journal j WHERE j.party_id=?1 AND j.control_kind='receivable' AND j.document_date<=?2 ORDER BY j.due_date,j.id)
 WHERE outstanding_signed_minor<>0 LIMIT ?3 OFFSET ?4",params![party,f.to,limit,offset])?;
            Ok(result(
                rows,
                &[
                    "document_date",
                    "document_number",
                    "document_kind",
                    "due_date",
                    "debit_minor",
                    "credit_minor",
                    "outstanding_signed_minor",
                ],
            ))
        }
        Report::StockAbc => {
            actor.require("finance.read")?;
            ensure(
                parts.len() == 2 && parts[0] == "product",
                "مرجع الصنف غير صالح",
            )?;
            let rows = db::rows(
                db,
                &format!(
                    "{JOURNAL} WHERE product_id=?1 AND account_kind='income' AND document_date BETWEEN ?2 AND ?3 ORDER BY document_date,id LIMIT ?4 OFFSET ?5"
                ),
                params![id(parts.get(1))?, f.from, f.to, limit, offset],
            )?;
            Ok(result(rows, JOURNAL_COLUMNS))
        }
        Report::StockTurnover | Report::StockActivity | Report::StockAging => {
            actor.require("inventory.read")?;
            let product = id(parts.get(1))?;
            let warehouse = if matches!(f.report, Report::StockTurnover) {
                ensure(
                    parts.len() == 2 && parts[0] == "product",
                    "مرجع الصنف غير صالح",
                )?;
                None
            } else {
                ensure(
                    parts.len() == 3 && parts[0] == "pool",
                    "مرجع المخزون غير صالح",
                )?;
                Some(id(parts.get(2))?)
            };
            let rows = db::rows(
                db,
                "SELECT s.document_id,d.number document_number,d.kind document_kind,d.document_date,s.warehouse_id,s.movement_type,s.quantity_micros,s.value_minor FROM stock_movements s JOIN documents d ON d.id=s.document_id WHERE s.product_id=?1 AND (?2 IS NULL OR s.warehouse_id=?2) AND d.status='posted' AND d.document_date<=?3 ORDER BY s.id LIMIT ?4 OFFSET ?5",
                params![product, warehouse, f.to, limit, offset],
            )?;
            Ok(result(
                rows,
                &[
                    "document_date",
                    "document_number",
                    "document_kind",
                    "warehouse_id",
                    "movement_type",
                    "quantity_micros",
                    "value_minor",
                ],
            ))
        }
        _ => Err(Error::validation("تفاصيل هذا التقرير غير متاحة")),
    }
}
