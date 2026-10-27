use super::{
    documents,
    finance::{self, Entry, Journal},
    inventory,
};
use crate::{
    error::{Error, Result, ensure},
    models::arithmetic::{allocate, convert, prorate, sum},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn post_return(
    db: &Connection,
    a: &Actor,
    source: i64,
    day: &str,
    requested_warehouse: Option<i64>,
    lines: &[ReturnLine],
    payments: &[Payment],
    reason: &str,
) -> Result<Value> {
    ensure(
        !lines.is_empty() && lines.len() <= 1000 && !reason.trim().is_empty(),
        "أصناف وسبب المرتجع مطلوبة",
    )?;
    let(kind,party,warehouse,original_date):(String,Option<i64>,Option<i64>,String)=db.query_row("SELECT kind,party_id,warehouse_id,document_date FROM documents WHERE id=?1 AND status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=?1 AND rev.status='posted')",[source],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    ensure(
        matches!(kind.as_str(), "sale" | "purchase") && day >= original_date.as_str(),
        "الفاتورة الأصلية أو تاريخ المرتجع غير صالح",
    )?;
    let sale = kind == "sale";
    a.require(if sale {
        "sales.return"
    } else {
        "purchases.write"
    })?;
    let warehouse = requested_warehouse
        .or(warehouse)
        .ok_or_else(|| Error::validation("حدد مخزن المرتجع"))?;
    let doc = documents::header(
        db,
        a,
        if sale {
            "sale_return"
        } else {
            "purchase_return"
        },
        day,
        party,
        Some(warehouse),
        reason,
    )?;
    db.execute(
        "UPDATE documents SET source_document_id=?1 WHERE id=?2",
        params![source, doc],
    )?;
    let originals = documents::lines(db, source)?;
    let mut unique = BTreeSet::new();
    let mut original_costs = Vec::new();
    for (index, input) in lines.iter().enumerate() {
        ensure(
            unique.insert(input.source_line_id.0) && input.quantity.0 > 0,
            "سطر المرتجع مكرر أو كميته غير صالحة",
        )?;
        let original = originals
            .iter()
            .find(|l| l.id == input.source_line_id.0)
            .ok_or_else(|| Error::validation("سطر المرتجع لا ينتمي للفاتورة"))?;
        let (num, den): (i64, i64) = db.query_row(
            "SELECT factor_numerator,factor_denominator FROM document_lines WHERE id=?1",
            [original.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let qty = convert(input.quantity.0, num, den)?;
        let used:(i64,i64,i64,i64,i64)=db.query_row("SELECT COALESCE(SUM(l.base_quantity_micros),0),COALESCE(SUM(l.subtotal_minor),0),COALESCE(SUM(l.line_discount_minor),0),COALESCE(SUM(l.invoice_discount_share_minor),0),COALESCE(SUM(l.tax_minor),0) FROM document_lines l JOIN documents d ON d.id=l.document_id WHERE l.source_line_id=?1 AND d.kind=?2 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",params![original.id,if sale{"sale_return"}else{"purchase_return"}],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        ensure(
            qty > 0 && qty <= original.qty - used.0,
            "كمية المرتجع تتجاوز المتاح",
        )?;
        let last = qty == original.qty - used.0;
        let gross = if last {
            original.gross - used.1
        } else {
            prorate(original.gross, qty, original.qty)?.min(original.gross - used.1)
        };
        let components = [
            original.discount - used.2,
            original.share - used.3,
            original.gross - used.1 - (original.discount - used.2) - (original.share - used.3),
        ];
        let portions = allocate(gross, &components)?;
        let tax = if last {
            original.tax - used.4
        } else {
            prorate(original.tax, qty, original.qty)?.min(original.tax - used.4)
        };
        let total = sum([gross, -portions[0], -portions[1], tax])?;
        db.execute("INSERT INTO document_lines(document_id,line_number,product_id,product_unit_id,source_line_id,product_code_snapshot,product_name_snapshot,unit_name_snapshot,category_id_snapshot,factor_numerator,factor_denominator,quantity_micros,base_quantity_micros,unit_price_minor,subtotal_minor,line_discount_minor,invoice_discount_share_minor,tax_rate_bps,tax_minor,total_minor) SELECT ?1,?2,product_id,product_unit_id,id,product_code_snapshot,product_name_snapshot,unit_name_snapshot,category_id_snapshot,factor_numerator,factor_denominator,?3,?4,unit_price_minor,?5,?6,?7,tax_rate_bps,?8,?9 FROM document_lines WHERE id=?10",params![doc,(index+1)as i64,input.quantity,qty,gross,portions[0],portions[1],tax,total,original.id])?;
        if sale {
            let base_cost:i64=db.query_row("SELECT -COALESCE(SUM(value_minor),0) FROM stock_movements WHERE document_line_id=?1 AND movement_type='sale'",[original.id],|r|r.get(0))?;
            let corrections:i64=db.query_row("SELECT COALESCE(SUM(c.delta_minor),0) FROM cost_attributions c JOIN stock_movements s ON s.id=c.source_movement_id JOIN documents d ON d.id=c.document_id WHERE s.document_line_id=?1 AND c.account_id=?2 AND d.status='posted'",params![original.id,finance::account(db,"cogs")?],|r|r.get(0))?;
            let cost = sum([base_cost, corrections])?;
            let returned:i64=db.query_row("SELECT COALESCE(SUM(j.credit_minor-j.debit_minor),0) FROM journal_lines j JOIN document_lines l ON l.id=j.document_line_id JOIN documents d ON d.id=j.document_id WHERE l.source_line_id=?1 AND j.account_id=?2 AND d.status='posted' AND d.kind='sale_return' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",params![original.id,finance::account(db,"cogs")?],|r|r.get(0))?;
            let returned_correction:i64=db.query_row("SELECT COALESCE(SUM(c.delta_minor),0) FROM cost_attributions c JOIN stock_movements s ON s.id=c.source_movement_id JOIN document_lines l ON l.id=s.document_line_id JOIN documents rd ON rd.id=l.document_id JOIN documents d ON d.id=c.document_id WHERE l.source_line_id=?1 AND c.account_id=?2 AND d.status='posted' AND rd.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=rd.id AND rev.status='posted')",params![original.id,finance::account(db,"cogs")?],|r|r.get(0))?;
            let returned = sum([returned, -returned_correction])?;
            original_costs.push(if last {
                cost - returned
            } else {
                prorate(cost, qty, original.qty)?.min(cost - returned)
            });
        }
    }
    documents::totals(db, doc)?;
    let total: i64 = db.query_row(
        "SELECT total_minor FROM documents WHERE id=?1",
        [doc],
        |r| r.get(0),
    )?;
    let paid = sum(payments.iter().map(|p| p.amount.0))?;
    ensure(
        paid <= total && (party.is_some() || paid == total),
        "المبلغ المسترد غير صالح؛ المرتجع النقدي يحتاج إلى رد كامل",
    )?;
    finance::save_payments(db, doc, payments, sale, party)?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    for (index, l) in documents::lines(db, doc)?.iter().enumerate() {
        let net = l.gross - l.discount - l.share;
        if sale {
            journal.add(Entry::item(
                finance::account(db, "sales_returns")?,
                net,
                party,
                l,
            ));
            journal.add(Entry::new(finance::account(db, "vat_output")?, l.tax));
            let (stock, correction) = inventory::receive(
                db,
                a,
                doc,
                Some(l.id),
                l.product,
                warehouse,
                l.qty,
                original_costs[index],
                "sale_return",
            )?;
            journal.add(Entry::item(
                finance::account(db, "inventory")?,
                stock,
                party,
                l,
            ));
            journal.add(Entry::item(
                finance::account(db, "cogs")?,
                -original_costs[index],
                party,
                l,
            ));
            if correction != 0 {
                let mut e = Entry::new(finance::account(db, "cogs")?, correction);
                e.product = Some(l.product);
                e.category = Some(l.category);
                e.party = party;
                journal.add(e);
            }
        } else {
            let cost = inventory::issue(
                db,
                a,
                doc,
                Some(l.id),
                l.product,
                warehouse,
                l.qty,
                "purchase_return",
            )?;
            journal.add(Entry::item(
                finance::account(db, "inventory")?,
                -cost,
                party,
                l,
            ));
            journal.add(Entry::new(finance::account(db, "vat_input")?, -l.tax));
            journal.add(Entry::item(
                finance::account(db, "purchase_variance")?,
                cost - net,
                party,
                l,
            ));
        }
    }
    if let Some(p) = party {
        journal.add(Entry::party(
            finance::account(db, if sale { "receivable" } else { "payable" })?,
            if sale { -total } else { total },
            p,
            day,
        ));
    }
    finance::post_payments(db, a, doc, party, !sale, &mut journal)?;
    journal.write(db, doc)?;
    finance::match_document_payments(db, doc)?;
    match_source(db, doc, source)?;
    documents::finish(db, a, doc)
}
fn match_source(db: &Connection, doc: i64, source: i64) -> Result<()> {
    let pairs:Vec<(i64,i64)>=db.prepare("SELECT CASE WHEN a.debit_minor>0 THEN a.id ELSE b.id END,CASE WHEN a.credit_minor>0 THEN a.id ELSE b.id END FROM journal_lines a JOIN journal_lines b ON a.account_id=b.account_id AND a.party_id=b.party_id JOIN accounts c ON c.id=a.account_id WHERE a.document_id=?1 AND b.document_id=?2 AND c.control_kind IN ('receivable','payable') AND ((a.debit_minor>0 AND b.credit_minor>0) OR (a.credit_minor>0 AND b.debit_minor>0)) ORDER BY a.id,b.id")?.query_map(params![doc,source],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    for (d, c) in pairs {
        let debit:i64=db.query_row("SELECT debit_minor-COALESCE((SELECT SUM(amount_minor) FROM settlements WHERE debit_line_id=?1),0) FROM journal_lines WHERE id=?1",[d],|r|r.get(0))?;
        let credit:i64=db.query_row("SELECT credit_minor-COALESCE((SELECT SUM(amount_minor) FROM settlements WHERE credit_line_id=?1),0) FROM journal_lines WHERE id=?1",[c],|r|r.get(0))?;
        let amount = debit.min(credit);
        if amount > 0 {
            db.execute("INSERT INTO settlements(document_id,debit_line_id,credit_line_id,amount_minor) VALUES (?1,?2,?3,?4)",params![doc,d,c,amount])?;
        }
    }
    Ok(())
}
pub fn reverse(db: &Connection, a: &Actor, source: i64, day: &str, reason: &str) -> Result<Value> {
    ensure(!reason.trim().is_empty(), "سبب العكس مطلوب")?;
    let(kind,party,warehouse,original_day):(String,Option<i64>,Option<i64>,String)=db.query_row("SELECT kind,party_id,warehouse_id,document_date FROM documents WHERE id=?1 AND status='posted'",[source],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    ensure(
        kind != "reversal" && day >= original_day.as_str(),
        "المستند أو تاريخ العكس غير صالح",
    )?;
    ensure(
        !matches!(
            kind.as_str(),
            "cheque_clear" | "cheque_bounce" | "cash_close"
        ),
        "استخدم حركة شيك مقابلة أو تسوية خزينة بدلاً من عكس هذا المستند",
    )?;
    let dependents:i64=db.query_row("SELECT COUNT(*) FROM documents d WHERE d.status='posted' AND d.source_document_id=?1 AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",[source],|r|r.get(0))?;
    ensure(dependents == 0, "اعكس المستندات التابعة أولاً")?;
    let matched:i64=db.query_row("SELECT COUNT(*) FROM purchase_matches m JOIN document_lines r ON r.id=m.receipt_line_id JOIN document_lines i ON i.id=m.invoice_line_id JOIN documents d ON d.id=i.document_id WHERE r.document_id=?1 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",[source],|r|r.get(0))?;
    ensure(matched == 0, "اعكس فواتير المورد المرتبطة بالاستلام أولاً")?;
    let costs:i64=db.query_row("SELECT COUNT(*) FROM landed_allocations a JOIN landed_charges c ON c.id=a.charge_id JOIN document_lines l ON l.id=a.receipt_line_id JOIN documents d ON d.id=c.document_id WHERE l.document_id=?1 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",[source],|r|r.get(0))?;
    ensure(costs == 0, "اعكس التكاليف الإضافية المرتبطة أولاً")?;
    if matches!(
        kind.as_str(),
        "goods_receipt"
            | "opening_stock"
            | "landed_cost"
            | "purchase"
            | "transfer"
            | "adjustment"
            | "stocktake"
    ) {
        let later:i64=db.query_row("SELECT COUNT(*) FROM stock_movements s JOIN stock_movements n ON n.product_id=s.product_id AND n.warehouse_id=s.warehouse_id AND n.id>s.id AND n.document_id<>s.document_id WHERE s.document_id=?1",[source],|r|r.get(0))?;
        ensure(
            later == 0,
            "توجد حركات مخزون لاحقة؛ استخدم مرتجعاً أو تسوية حالية",
        )?;
    }
    let cheques: Vec<(i64, String)> = db
        .prepare("SELECT id,status FROM v_cheque_register WHERE document_id=?1")?
        .query_map([source], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    ensure(
        cheques.iter().all(|(_, s)| s == "pending"),
        "الشيك تمت معالجته؛ استخدم سند رد أو مرتجعاً",
    )?;
    let allocations:Vec<i64>=db.prepare("SELECT s.id FROM settlements s WHERE s.amount_minor>0 AND NOT EXISTS(SELECT 1 FROM settlements r WHERE r.reversal_of_id=s.id) AND (s.document_id=?1 OR s.debit_line_id IN(SELECT id FROM journal_lines WHERE document_id=?1) OR s.credit_line_id IN(SELECT id FROM journal_lines WHERE document_id=?1))")?.query_map([source],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    for allocation in allocations {
        finance::unsettle(db, a, day, allocation, reason)?;
    }
    // Reversal requires its source FK at insertion time, so create explicitly with snapshots.
    documents::date(day)?;
    db.execute("INSERT INTO documents(kind,document_date,due_date,party_id,warehouse_id,reversal_of_id,currency_code,currency_symbol,currency_minor_digits,party_name_snapshot,party_address_snapshot,party_tax_number_snapshot,store_snapshot_json,reason,created_by) SELECT 'reversal',?1,?1,party_id,warehouse_id,id,currency_code,currency_symbol,currency_minor_digits,party_name_snapshot,party_address_snapshot,party_tax_number_snapshot,store_snapshot_json,?2,?3 FROM documents WHERE id=?4",params![day,reason,a.user_id,source])?;
    let doc = db.last_insert_rowid();
    documents::begin(db, doc)?;
    db.execute("INSERT INTO stock_movements(document_id,product_id,warehouse_id,movement_type,quantity_micros,value_minor,unit_cost_micropiastres,reverses_movement_id,reason,created_by) SELECT ?1,product_id,warehouse_id,'reversal',-quantity_micros,-value_minor,unit_cost_micropiastres,id,?2,?3 FROM stock_movements WHERE document_id=?4 ORDER BY id DESC",params![doc,reason,a.user_id,source])?;
    let entries:Vec<Entry>=db.prepare("SELECT id,account_id,credit_minor-debit_minor,party_id,due_date,product_id,category_id,memo FROM journal_lines WHERE document_id=?1 ORDER BY line_number")?.query_map([source],|r|Ok(Entry{reversal:Some(r.get(0)?),account:r.get(1)?,amount:r.get(2)?,party:r.get(3)?,due:r.get(4)?,product:r.get(5)?,category:r.get(6)?,memo:r.get(7)?,line:None}))?.collect::<rusqlite::Result<_>>()?;
    Journal { entries }.write(db, doc)?;
    db.execute("INSERT INTO cost_attributions(document_id,source_movement_id,account_id,delta_minor) SELECT ?1,source_movement_id,account_id,-delta_minor FROM cost_attributions WHERE document_id=?2",params![doc,source])?;
    for (cheque, _) in cheques {
        db.execute("INSERT INTO cheque_events(cheque_id,document_id,status,reason,created_by) VALUES (?1,?2,'bounced',?3,?4)",params![cheque,doc,reason,a.user_id])?;
    }
    match_source(db, doc, source)?;
    let _ = (party, warehouse);
    documents::finish(db, a, doc)
}
