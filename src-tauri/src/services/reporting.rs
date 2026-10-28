use super::{catalog::normalize, documents};
use crate::{
    db,
    error::{Error, Result, ensure},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

fn read_permission(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "sale" | "sale_return" | "quotation" => "sales.read",
        "purchase" | "purchase_order" | "purchase_return" | "landed_cost" => "purchases.read",
        "goods_receipt" | "transfer" | "adjustment" | "stocktake" | "opening_stock" => {
            "inventory.read"
        }
        "receipt_voucher" | "payment_voucher" | "manual_journal" | "opening_balance"
        | "cash_transfer" | "expense" | "settlement" | "cheque_clear" | "cheque_bounce"
        | "cash_close" | "cost_adjustment" | "reversal" => "finance.read",
        _ => return Err(Error::validation("نوع المستند غير صالح")),
    })
}
fn limit(value: u16) -> i64 {
    i64::from(value.clamp(1, 200))
}
pub fn query(db: &Connection, a: &Actor, q: &Query) -> Result<Value> {
    match q {
        Query::PrintSnapshot { source } => super::printing::snapshot(db, a, source),
        Query::Bootstrap => {
            let mut value = json!({"authenticated":true,"setup_required":false,"permissions":a.permissions,"user_id":Int(a.user_id)});
            for (table, key) in [
                ("store_settings", "settings"),
                ("categories", "categories"),
                ("units", "units"),
                ("warehouses", "warehouses"),
                ("cash_accounts", "cash_accounts"),
                ("expense_categories", "expense_categories"),
            ] {
                value[key] = json!(db::rows(db, &format!("SELECT * FROM {table}"), [])?);
            }
            if a.permissions.contains("finance.read") {
                value["accounts"] =
                    json!(db::rows(db, "SELECT * FROM accounts ORDER BY code", [])?);
            }
            value["display_name"] = json!(db.query_row(
                "SELECT display_name FROM users WHERE id=?1",
                [a.user_id],
                |r| r.get::<_, String>(0)
            )?);
            if a.permissions.contains("periods.lock") || a.permissions.contains("periods.unlock") {
                value["fiscal_periods"] = json!(db::rows(
                    db,
                    "SELECT * FROM fiscal_periods ORDER BY starts_on",
                    []
                )?);
            }
            if a.permissions.contains("settings.write") {
                value["sequences"] = json!(db::rows(
                    db,
                    "SELECT * FROM number_sequences ORDER BY document_type,fiscal_year",
                    []
                )?);
                value["backup_runs"] = json!(db::rows(
                    db,
                    "SELECT id,kind,path,status,size_bytes,error_message,completed_at FROM backup_runs ORDER BY id DESC LIMIT 30",
                    []
                )?);
            }
            value["must_change_password"] = json!(db.query_row(
                "SELECT must_change_password FROM users WHERE id=?1",
                [a.user_id],
                |r| r.get::<_, bool>(0)
            )?);
            Ok(value)
        }
        Query::CashDesk => {
            a.require("cash.shift")?;
            Ok(
                json!({"shifts":db::rows(db,"SELECT * FROM cash_shifts WHERE closed_at IS NULL ORDER BY id",[])?,"balances":db::rows(db,"SELECT * FROM v_cash_balances",[])?}),
            )
        }
        Query::Products {
            search,
            after_id,
            limit: l,
        } => {
            a.require("products.read")?;
            ensure(search.len() <= 256, "نص البحث طويل")?;
            let norm = normalize(search);
            let fts = if norm.trim().is_empty() {
                "\"__empty__\"".into()
            } else {
                norm.split_whitespace()
                    .map(|s| format!("\"{}\"*", s.replace('"', "\"\"")))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            };
            Ok(json!(db::rows(
                db,
                "SELECT p.*,(SELECT id FROM product_units WHERE product_id=p.id AND barcode=?1 AND active=1 LIMIT 1) AS matched_unit_id FROM products p WHERE p.id>?2 AND (?1='' OR p.code LIKE ?3 ESCAPE '\\' OR p.id IN(SELECT product_id FROM product_units WHERE barcode=?1 AND active=1) OR p.id IN(SELECT rowid FROM product_search WHERE product_search MATCH ?4)) ORDER BY p.id LIMIT ?5",
                params![
                    search.trim(),
                    after_id,
                    format!("{}%", escape_like(search)),
                    fts,
                    limit(*l)
                ]
            )?))
        }
        Query::Product { id } => {
            a.require("products.read")?;
            Ok(
                json!({"product":db::rows(db,"SELECT * FROM products WHERE id=?1",[id])?,"warehouse_settings":db::rows(db,"SELECT * FROM warehouse_product_settings WHERE product_id=?1",[id])?,"units":db::rows(db,"SELECT pu.*,u.name_ar FROM product_units pu JOIN units u ON u.id=pu.unit_id WHERE product_id=?1 ORDER BY pu.id",[id])?}),
            )
        }
        Query::Parties {
            search,
            after_id,
            limit: l,
        } => {
            a.require("parties.read")?;
            ensure(search.len() <= 256, "نص البحث طويل")?;
            Ok(json!(db::rows(
                db,
                "SELECT * FROM parties WHERE id>?1 AND (normalized_name LIKE ?2 ESCAPE '\\' OR code LIKE ?3 ESCAPE '\\' OR phone LIKE ?3 ESCAPE '\\') ORDER BY id LIMIT ?4",
                params![
                    after_id,
                    format!("%{}%", escape_like(&normalize(search))),
                    format!("%{}%", escape_like(search)),
                    limit(*l)
                ]
            )?))
        }
        Query::Documents {
            kind,
            status,
            after_id,
            limit: l,
        } => {
            a.require(read_permission(kind)?)?;
            Ok(json!(db::rows(
                db,
                "SELECT d.*,(SELECT progress FROM v_purchase_order_progress WHERE document_id=d.id) AS progress,(SELECT number FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted') AS reversal_number FROM documents d WHERE d.kind=?1 AND (?2 IS NULL OR d.status=?2) AND d.id>?3 ORDER BY d.id LIMIT ?4",
                params![kind, status, after_id, limit(*l)]
            )?))
        }
        Query::Document { id } => {
            let kind: String =
                db.query_row("SELECT kind FROM documents WHERE id=?1", [id], |r| r.get(0))?;
            a.require(read_permission(&kind)?)?;
            let mut value = json!({"header":db::rows(db,"SELECT * FROM documents WHERE id=?1",[id])?,"lines":db::rows(db,"SELECT * FROM document_lines WHERE document_id=?1 ORDER BY line_number",[id])?});
            for (table, key) in [
                ("payment_legs", "payments"),
                ("stocktake_lines", "counts"),
                ("expense_lines", "expenses"),
                ("document_events", "events"),
            ] {
                value[key] = json!(db::rows(
                    db,
                    &format!("SELECT * FROM {table} WHERE document_id=?1 ORDER BY id"),
                    [id]
                )?);
            }
            value["returned"] = json!(db::rows(
                db,
                "SELECT r.source_line_id,SUM(r.subtotal_minor) subtotal_minor,SUM(r.line_discount_minor) line_discount_minor,SUM(r.invoice_discount_share_minor) invoice_discount_share_minor,SUM(r.tax_minor) tax_minor FROM document_lines r JOIN documents d ON d.id=r.document_id WHERE r.source_line_id IN(SELECT id FROM document_lines WHERE document_id=?1) AND d.status='posted' AND d.kind IN ('sale_return','purchase_return') AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted') GROUP BY r.source_line_id",
                [id]
            )?);
            value["counts"] = json!(db::rows(
                db,
                "SELECT c.*,p.code,p.name_ar FROM stocktake_lines c JOIN products p ON p.id=c.product_id WHERE c.document_id=?1 ORDER BY c.id",
                [id]
            )?);
            value["receiving"] = json!(db::rows(
                db,
                "SELECT r.* FROM receipt_lines r JOIN document_lines l ON l.id=r.document_line_id WHERE l.document_id=?1",
                [id]
            )?);
            value["matches"] = json!(db::rows(
                db,
                "SELECT m.* FROM purchase_matches m JOIN document_lines l ON l.id=m.invoice_line_id WHERE l.document_id=?1",
                [id]
            )?);
            value["cheques"] = json!(db::rows(
                db,
                "SELECT c.* FROM cheques c JOIN payment_legs p ON p.id=c.payment_leg_id WHERE p.document_id=?1",
                [id]
            )?);
            value["availability"] = json!(db::rows(
                db,
                "SELECT l.id,l.product_id,l.product_unit_id,MAX(0,l.base_quantity_micros-COALESCE((SELECT SUM(rl.base_quantity_micros) FROM receipt_lines r JOIN document_lines rl ON rl.id=r.document_line_id JOIN documents rd ON rd.id=rl.document_id WHERE r.purchase_order_line_id=l.id AND rd.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=rd.id AND rev.status='posted')),0)) AS unreceived_micros,MAX(0,l.base_quantity_micros-COALESCE((SELECT SUM(m.base_quantity_micros) FROM purchase_matches m JOIN document_lines il ON il.id=m.invoice_line_id JOIN documents iv ON iv.id=il.document_id WHERE m.receipt_line_id=l.id AND iv.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=iv.id AND rev.status='posted')),0)) AS unbilled_micros,MAX(0,l.quantity_micros-COALESCE((SELECT SUM(rl.quantity_micros) FROM document_lines rl JOIN documents rd ON rd.id=rl.document_id WHERE rl.source_line_id=l.id AND rd.status='posted' AND rd.kind IN ('sale_return','purchase_return') AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=rd.id AND rev.status='posted')),0)) AS returnable_micros FROM document_lines l WHERE l.document_id=?1 ORDER BY l.line_number",
                [id]
            )?);
            if a.permissions.contains("finance.read") {
                value["journal"] = json!(db::rows(
                    db,
                    "SELECT j.*,ac.name_ar AS account_name,ac.code AS account_code,p.name_ar AS party_name FROM journal_lines j JOIN accounts ac ON ac.id=j.account_id LEFT JOIN parties p ON p.id=j.party_id WHERE j.document_id=?1 ORDER BY j.id",
                    [id]
                )?);
                value["settlements"] = json!(db::rows(
                    db,
                    "SELECT * FROM settlements WHERE document_id=?1 ORDER BY id",
                    [id]
                )?);
            }
            Ok(value)
        }
        Query::Inventory {
            warehouse_id,
            after_id,
            limit: l,
        } => {
            a.require("inventory.read")?;
            Ok(json!(db::rows(
                db,
                "SELECT p.id AS product_id,p.code,p.name_ar,?2 AS warehouse_id,CASE WHEN ?2 IS NULL THEN 'كل المخازن' ELSE MAX(w.name_ar) END AS warehouse_name,COALESCE(SUM(s.quantity_micros),0) AS quantity_micros,COALESCE(SUM(s.value_minor),0) AS value_minor FROM products p CROSS JOIN warehouses w LEFT JOIN v_stock_on_hand s ON s.product_id=p.id AND s.warehouse_id=w.id WHERE p.id>?1 AND (?2 IS NULL OR w.id=?2) GROUP BY p.id ORDER BY p.id LIMIT ?3",
                params![after_id, warehouse_id, limit(*l)]
            )?))
        }
        Query::StockCard {
            product_id,
            warehouse_id,
            after_id,
            limit: l,
        } => {
            a.require("inventory.read")?;
            Ok(json!(db::rows(
                db,
                "SELECT * FROM v_stock_card WHERE product_id=?1 AND warehouse_id=?2 AND id>?3 ORDER BY id LIMIT ?4",
                params![product_id, warehouse_id, after_id, limit(*l)]
            )?))
        }
        Query::Report {
            report: r,
            from,
            to,
            party_id,
            limit: l,
            offset,
        } => {
            a.require("reports.read")?;
            report(
                db,
                *r,
                from,
                to,
                party_id.map(|v| v.0),
                i64::from((*l).clamp(1, 2000)),
                i64::from(*offset),
            )
        }
        Query::Dashboard { date } => dashboard(db, a, date),
        Query::Audit { after_id, limit: l } => {
            a.require("audit.read")?;
            Ok(json!(db::rows(
                db,
                "SELECT * FROM audit_log WHERE id>?1 ORDER BY id LIMIT ?2",
                params![after_id, limit(*l)]
            )?))
        }
        Query::Users => {
            a.require("users.manage")?;
            Ok(
                json!({"users":db::rows(db,"SELECT id,username,display_name,active,must_change_password,failed_login_count,locked_until,created_at FROM users ORDER BY id",[])?,"roles":db::rows(db,"SELECT * FROM roles ORDER BY id",[])?,"user_roles":db::rows(db,"SELECT * FROM user_roles ORDER BY user_id,role_id",[])?}),
            )
        }
    }
}
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
pub(crate) fn report(
    db: &Connection,
    r: Report,
    from: &str,
    to: &str,
    party: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    documents::date(from)?;
    documents::date(to)?;
    ensure(from <= to, "نطاق التاريخ غير صالح")?;
    if matches!(r, Report::Aging) {
        let sql = format!(
            "SELECT * FROM ({}) WHERE (?2 IS NULL OR party_id=?2) LIMIT ?3 OFFSET ?4",
            include_str!("../../queries/aging.sql")
                .trim()
                .trim_end_matches(';')
        );
        return Ok(json!(db::rows(
            db,
            &sql,
            params![to, party, limit, offset]
        )?));
    }
    let sql = match r {
        Report::TrialBalance => {
            "SELECT account_id,account_code,account_name,SUM(CASE WHEN document_date<?1 THEN debit_minor-credit_minor ELSE 0 END) opening_minor,SUM(CASE WHEN document_date>=?1 THEN debit_minor ELSE 0 END) debit_minor,SUM(CASE WHEN document_date>=?1 THEN credit_minor ELSE 0 END) credit_minor,SUM(debit_minor-credit_minor) closing_minor FROM v_journal WHERE document_date<=?2 GROUP BY account_id ORDER BY account_code"
        }
        Report::IncomeStatement => {
            "SELECT account_id,account_code,account_name,account_kind,SUM(credit_minor-debit_minor) profit_signed_minor FROM v_journal WHERE document_date BETWEEN ?1 AND ?2 AND account_kind IN ('income','expense') GROUP BY account_id ORDER BY account_code"
        }
        Report::CashFlow => {
            "SELECT document_date,document_kind,cash_account_id,SUM(cash_in_minor) cash_in_minor,SUM(cash_out_minor) cash_out_minor FROM v_cash_flow WHERE document_date BETWEEN ?1 AND ?2 GROUP BY document_date,document_kind,cash_account_id ORDER BY document_date"
        }
        Report::ProductProfit => {
            "SELECT p.product_id,x.name_ar,SUM(p.net_sales_minor) net_sales_minor,SUM(p.cogs_minor) cogs_minor,SUM(p.net_sales_minor-p.cogs_minor) gross_profit_minor FROM v_product_profit p JOIN products x ON x.id=p.product_id WHERE p.document_date BETWEEN ?1 AND ?2 GROUP BY p.product_id ORDER BY gross_profit_minor DESC"
        }
        Report::CategoryProfit => {
            "SELECT p.category_id,x.name_ar,SUM(p.net_sales_minor) net_sales_minor,SUM(p.cogs_minor) cogs_minor,SUM(p.net_sales_minor-p.cogs_minor) gross_profit_minor FROM v_product_profit p LEFT JOIN categories x ON x.id=p.category_id WHERE p.document_date BETWEEN ?1 AND ?2 GROUP BY p.category_id ORDER BY gross_profit_minor DESC"
        }
        Report::CustomerProfit => {
            "SELECT p.party_id,COALESCE(x.name_ar,'عميل نقدي') name_ar,SUM(p.net_sales_minor) net_sales_minor,SUM(p.cogs_minor) cogs_minor,SUM(p.net_sales_minor-p.cogs_minor) gross_profit_minor FROM v_product_profit p LEFT JOIN parties x ON x.id=p.party_id WHERE p.document_date BETWEEN ?1 AND ?2 GROUP BY p.party_id ORDER BY gross_profit_minor DESC"
        }
        Report::PartyStatement => {
            "SELECT *,SUM(debit_minor-credit_minor) OVER(PARTITION BY account_id ORDER BY document_date,id ROWS UNBOUNDED PRECEDING) running_balance_minor FROM v_journal WHERE party_id=?3 AND control_kind IN ('receivable','payable') AND document_date<=?2 ORDER BY document_date,id"
        }
        Report::Receivables => {
            "SELECT j.party_id,p.name_ar,SUM(j.debit_minor-j.credit_minor) balance_minor FROM v_journal j JOIN parties p ON p.id=j.party_id WHERE j.document_date<=?2 AND j.control_kind='receivable' GROUP BY j.party_id ORDER BY balance_minor DESC"
        }
        Report::Payables => {
            "SELECT j.party_id,p.name_ar,SUM(j.credit_minor-j.debit_minor) balance_minor FROM v_journal j JOIN parties p ON p.id=j.party_id WHERE j.document_date<=?2 AND j.control_kind='payable' GROUP BY j.party_id ORDER BY balance_minor DESC"
        }
        Report::Vat => {
            "SELECT COALESCE(SUM(input_vat_minor),0) input_vat_minor,COALESCE(SUM(output_vat_minor),0) output_vat_minor,COALESCE(SUM(output_vat_minor-input_vat_minor),0) net_vat_minor FROM v_vat_summary WHERE document_date BETWEEN ?1 AND ?2"
        }
        Report::LowStock => "SELECT * FROM v_low_stock ORDER BY product_id,warehouse_id",
        Report::Valuation => {
            "SELECT s.product_id,p.name_ar,s.warehouse_id,SUM(s.quantity_micros) quantity_micros,SUM(s.value_minor) value_minor FROM stock_movements s JOIN documents d ON d.id=s.document_id JOIN products p ON p.id=s.product_id WHERE d.status='posted' AND d.document_date<=?2 GROUP BY s.product_id,s.warehouse_id ORDER BY s.product_id,s.warehouse_id"
        }
        Report::Cheques => {
            "SELECT * FROM v_cheque_register WHERE due_date BETWEEN ?1 AND ?2 ORDER BY due_date,id"
        }
        Report::PurchaseOrders => {
            "SELECT * FROM v_purchase_order_progress WHERE document_date BETWEEN ?1 AND ?2 ORDER BY document_id"
        }
        Report::OpenItems => {
            "SELECT * FROM v_party_open_items WHERE (?3 IS NULL OR party_id=?3) AND outstanding_signed_minor<>0 ORDER BY due_date,journal_line_id"
        }
        Report::CashShifts => {
            "SELECT * FROM cash_shifts WHERE substr(opened_at,1,10) BETWEEN ?1 AND ?2 ORDER BY id"
        }
        Report::Aging => unreachable!(),
    };
    // Keep all five positions present even for reports that do not use a date/party filter.
    let wrapped = if matches!(r, Report::PartyStatement) {
        format!(
            "SELECT * FROM ({sql}) WHERE document_date>=?1 AND (?3 IS NULL OR party_id=?3) AND ?2 IS NOT NULL LIMIT ?4 OFFSET ?5"
        )
    } else {
        format!(
            "SELECT * FROM ({sql}) WHERE ?1 IS NOT NULL AND ?2 IS NOT NULL AND (?3 IS NULL OR ?3 IS NOT NULL) LIMIT ?4 OFFSET ?5"
        )
    };
    let rows = db::rows(db, &wrapped, params![from, to, party, limit, offset])?;
    if matches!(r, Report::PartyStatement) {
        return Ok(
            json!({"rows":rows,"opening":db::rows(db,"SELECT account_id,COALESCE(SUM(debit_minor-credit_minor),0) balance_minor FROM v_journal WHERE party_id=?1 AND document_date<?2 AND control_kind IN ('receivable','payable') GROUP BY account_id",params![party,from])?}),
        );
    }
    Ok(json!(rows))
}
fn dashboard(db: &Connection, a: &Actor, day: &str) -> Result<Value> {
    a.require("dashboard.read")?;
    let date = documents::date(day)?;
    let from = (date - chrono::Duration::days(29)).to_string();
    let month = format!("{}-01", &day[..7]);
    let sales = db::rows(
        db,
        "SELECT COALESCE(SUM(CASE WHEN document_date=?1 THEN net_sales_minor ELSE 0 END),0) today_sales_minor,COALESCE(SUM(net_sales_minor),0) month_sales_minor,COALESCE(SUM(net_sales_minor-cogs_minor),0) gross_profit_minor FROM v_product_profit WHERE document_date BETWEEN ?2 AND ?1",
        params![day, month],
    )?;
    let mut result = json!({"sales":sales,"low_stock_count":Int(db.query_row("SELECT COUNT(*) FROM v_low_stock",[],|r|r.get(0))?),"low_stock":db::rows(db,"SELECT * FROM v_low_stock ORDER BY quantity_micros LIMIT 10",[])?,"sales_trend":db::rows(db,"SELECT document_date,SUM(net_sales_minor) sales_minor FROM v_product_profit WHERE document_date BETWEEN ?1 AND ?2 GROUP BY document_date ORDER BY document_date",params![from,day])?,"top_products":report(db,Report::ProductProfit,&from,day,None,10,0)?,"categories":report(db,Report::CategoryProfit,&from,day,None,2000,0)?});
    result["top_products"] = json!(db::rows(
        db,
        "SELECT p.product_id,x.name_ar,SUM(p.net_sales_minor) net_sales_minor,SUM(p.cogs_minor) cogs_minor FROM v_product_profit p JOIN products x ON x.id=p.product_id WHERE p.document_date BETWEEN ?1 AND ?2 GROUP BY p.product_id ORDER BY net_sales_minor DESC LIMIT 10",
        params![from, day]
    )?);
    if a.permissions.contains("finance.read") {
        result["balances"] = json!(db::rows(
            db,
            "SELECT COALESCE(SUM(CASE WHEN control_kind='receivable' THEN debit_minor-credit_minor ELSE 0 END),0) receivable_minor,COALESCE(SUM(CASE WHEN control_kind='payable' THEN credit_minor-debit_minor ELSE 0 END),0) payable_minor FROM v_journal WHERE document_date<=?1",
            [day]
        )?);
        result["cash"] = json!(db::rows(db, "SELECT * FROM v_cash_balances", [])?);
        result["receivables"] = report(db, Report::Receivables, &from, day, None, 10, 0)?;
        result["payables"] = report(db, Report::Payables, &from, day, None, 10, 0)?;
        result["cash_flow"] = json!(db::rows(
            db,
            "SELECT document_date,SUM(cash_in_minor) cash_in_minor,SUM(cash_out_minor) cash_out_minor FROM v_cash_flow WHERE document_date BETWEEN ?1 AND ?2 GROUP BY document_date ORDER BY document_date",
            params![from, day]
        )?);
        result["cheques_due"] = json!(db::rows(
            db,
            "SELECT * FROM v_cheque_register WHERE status='pending' AND due_date<=date(?1,'+7 days') ORDER BY due_date LIMIT 10",
            [day]
        )?);
        result["overdue"] = json!(db::rows(
            db,
            "SELECT i.party_id,p.name_ar,SUM(i.outstanding_signed_minor) overdue_minor FROM v_party_open_items i JOIN parties p ON p.id=i.party_id WHERE i.control_kind='receivable' AND i.due_date<?1 AND i.outstanding_signed_minor>0 GROUP BY i.party_id ORDER BY overdue_minor DESC LIMIT 10",
            [day]
        )?);
    }
    if a.permissions.contains("sales.read") {
        result["recent_invoices"] = json!(db::rows(
            db,
            "SELECT id,number,document_date,party_name_snapshot,total_minor,status FROM documents WHERE kind='sale' ORDER BY id DESC LIMIT 10",
            []
        )?);
    }
    Ok(result)
}
