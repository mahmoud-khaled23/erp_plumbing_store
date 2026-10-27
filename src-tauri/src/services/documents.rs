use super::{
    finance::{Entry, Journal},
    inventory,
};
use crate::{
    db,
    error::{Error, Result, ensure},
    models::arithmetic::{MONEY_MAX, SCALE, allocate, convert, prorate, sum},
    models::*,
    state::Actor,
};
use chrono::{Duration, NaiveDate};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub fn date(value: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| Error::validation("التاريخ غير صالح"))?;
    ensure(
        parsed.format("%Y-%m-%d").to_string() == value,
        "صيغة التاريخ غير صالحة",
    )?;
    Ok(parsed)
}
pub fn header(
    db: &Connection,
    a: &Actor,
    kind: &str,
    day: &str,
    party: Option<i64>,
    warehouse: Option<i64>,
    reason: &str,
) -> Result<i64> {
    date(day)?;
    let settings = db::rows(
        db,
        "SELECT store_name,tax_number,address,phones,logo_path,currency_code,currency_symbol,currency_minor_digits FROM store_settings WHERE id=1",
        [],
    )?;
    let (name, address, tax, terms): (String, String, String, i64) = if let Some(id) = party {
        db.query_row("SELECT name_ar,address,tax_number,payment_terms_days FROM parties WHERE id=?1 AND active=1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?
    } else {
        ("عميل نقدي".into(), String::new(), String::new(), 0)
    };
    let due = date(day)?
        .checked_add_signed(Duration::days(terms))
        .ok_or_else(|| Error::validation("تاريخ الاستحقاق غير صالح"))?
        .to_string();
    db.execute("INSERT INTO documents(kind,document_date,due_date,party_id,warehouse_id,reason,created_by,currency_code,currency_symbol,currency_minor_digits,party_name_snapshot,party_address_snapshot,party_tax_number_snapshot,store_snapshot_json) SELECT ?1,?2,?3,?4,?5,?6,?7,currency_code,currency_symbol,currency_minor_digits,?8,?9,?10,?11 FROM store_settings WHERE id=1",params![kind,day,due,party,warehouse,reason,a.user_id,name,address,tax,serde_json::to_string(&settings[0])?])?;
    Ok(db.last_insert_rowid())
}
pub fn number(db: &Connection, id: i64) -> Result<()> {
    let (kind, day, current): (String, String, Option<String>) = db.query_row(
        "SELECT kind,document_date,number FROM documents WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if current.is_some() {
        return Ok(());
    }
    let year: i64 = day[..4]
        .parse()
        .map_err(|_| Error::validation("السنة غير صالحة"))?;
    db.execute("INSERT OR IGNORE INTO number_sequences(document_type,fiscal_year,prefix) VALUES (?1,?2,?3)",params![kind,year,kind.to_uppercase()])?;
    let(prefix,next,padding):(String,i64,i64)=db.query_row("SELECT prefix,next_value,padding FROM number_sequences WHERE document_type=?1 AND fiscal_year=?2",params![kind,year],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let number = format!("{prefix}-{year}-{next:0width$}", width = padding as usize);
    db.execute("UPDATE number_sequences SET next_value=next_value+1 WHERE document_type=?1 AND fiscal_year=?2",params![kind,year])?;
    db.execute(
        "UPDATE documents SET number=?1 WHERE id=?2",
        params![number, id],
    )?;
    Ok(())
}
pub fn begin(db: &Connection, id: i64) -> Result<()> {
    number(db, id)?;
    ensure(
        db.execute(
            "UPDATE documents SET status='posting' WHERE id=?1 AND status='draft'",
            [id],
        )? == 1,
        "المستند ليس مسودة قابلة للترحيل",
    )
}
pub fn finish(db: &Connection, a: &Actor, id: i64) -> Result<Value> {
    ensure(db.execute("UPDATE documents SET status='posted',posted_by=?1,posted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),revision=revision+1 WHERE id=?2 AND status='posting'",params![a.user_id,id])?==1,"المستند غير جاهز للترحيل")?;
    event(db, a, id, "draft", "posted", "")?;
    Ok(
        json!({"id":Int(id),"status":"posted","number":db.query_row("SELECT number FROM documents WHERE id=?1",[id],|r|r.get::<_,String>(0))?}),
    )
}
pub fn event(
    db: &Connection,
    a: &Actor,
    id: i64,
    from: &str,
    to: &str,
    reason: &str,
) -> Result<()> {
    db.execute("INSERT INTO document_events(document_id,from_status,to_status,reason,user_id) VALUES (?1,?2,?3,?4,?5)",params![id,from,to,reason,a.user_id])?;
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Line {
    pub id: i64,
    pub product: i64,
    pub category: i64,
    pub qty: i64,
    pub gross: i64,
    pub discount: i64,
    pub share: i64,
    pub tax: i64,
    pub total: i64,
    pub source: Option<i64>,
}
pub fn lines(db: &Connection, id: i64) -> Result<Vec<Line>> {
    Ok(db.prepare("SELECT id,product_id,category_id_snapshot,base_quantity_micros,subtotal_minor,line_discount_minor,invoice_discount_share_minor,tax_minor,total_minor,source_line_id FROM document_lines WHERE document_id=?1 ORDER BY line_number")?
        .query_map([id],|r|Ok(Line{id:r.get(0)?,product:r.get(1)?,category:r.get(2)?,qty:r.get(3)?,gross:r.get(4)?,discount:r.get(5)?,share:r.get(6)?,tax:r.get(7)?,total:r.get(8)?,source:r.get(9)?}))?.collect::<rusqlite::Result<_>>()?)
}
pub fn totals(db: &Connection, id: i64) -> Result<()> {
    db.execute("UPDATE documents SET subtotal_minor=(SELECT COALESCE(SUM(subtotal_minor),0) FROM document_lines WHERE document_id=?1),line_discount_minor=(SELECT COALESCE(SUM(line_discount_minor),0) FROM document_lines WHERE document_id=?1),invoice_discount_minor=(SELECT COALESCE(SUM(invoice_discount_share_minor),0) FROM document_lines WHERE document_id=?1),tax_minor=(SELECT COALESCE(SUM(tax_minor),0) FROM document_lines WHERE document_id=?1),total_minor=(SELECT COALESCE(SUM(total_minor),0) FROM document_lines WHERE document_id=?1) WHERE id=?1",[id])?;
    Ok(())
}
pub fn save(db: &Connection, a: &Actor, d: &Draft) -> Result<Value> {
    a.require(d.kind.permission())?;
    date(&d.date)?;
    ensure(
        !d.lines.is_empty() && d.lines.len() <= 1000 && d.payments.len() <= 20,
        "عدد السطور أو الدفعات غير صالح",
    )?;
    ensure(d.discount.0 >= 0, "خصم الفاتورة غير صالح")?;
    if matches!(d.kind, DocumentKind::Sale | DocumentKind::Quotation)
        && (d.discount.0 > 0 || d.lines.iter().any(|l| l.discount.0 > 0))
    {
        a.require("sales.discount")?;
    }
    let mut gross = Vec::new();
    let mut weights = Vec::new();
    for l in &d.lines {
        ensure(
            (0..=MONEY_MAX).contains(&l.price.0)
                && l.quantity.0 >= 0
                && l.quantity.0 <= 1_000_000_000_000_000
                && l.tax_bps <= 10000,
            "كمية أو سعر أو ضريبة غير صالحة",
        )?;
        ensure(
            l.quantity.0 > 0 || (d.kind == DocumentKind::GoodsReceipt && l.damaged.0 > 0),
            "كمية الصنف يجب أن تكون موجبة",
        )?;
        let total = prorate(l.price.0, l.quantity.0, SCALE)?;
        ensure(
            total <= MONEY_MAX && l.discount.0 >= 0 && l.discount.0 <= total,
            "خصم السطر غير صالح",
        )?;
        if matches!(
            d.kind,
            DocumentKind::GoodsReceipt | DocumentKind::Transfer | DocumentKind::OpeningStock
        ) {
            ensure(
                l.tax_bps == 0 && l.discount.0 == 0 && d.discount.0 == 0,
                "المستند المخزني لا يقبل ضريبة أو خصماً",
            )?;
        }
        gross.push(total);
        weights.push(total - l.discount.0);
    }
    ensure(
        d.discount.0 <= sum(weights.iter().copied())?,
        "الخصم يتجاوز الإجمالي",
    )?;
    let shares = allocate(d.discount.0, &weights)?;
    let id = if let Some(id) = d.id {
        let (old, status, revision): (String, String, i64) = db.query_row(
            "SELECT kind,status,revision FROM documents WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure(
            old == d.kind.code() && status == "draft" && revision == d.revision.0,
            "المستند تغير أو لم يعد مسودة",
        )?;
        db.execute("DELETE FROM cheques WHERE payment_leg_id IN (SELECT id FROM payment_legs WHERE document_id=?1)",[id])?;
        db.execute("DELETE FROM payment_legs WHERE document_id=?1", [id])?;
        db.execute("DELETE FROM purchase_matches WHERE invoice_line_id IN (SELECT id FROM document_lines WHERE document_id=?1)",[id])?;
        db.execute("DELETE FROM receipt_lines WHERE document_line_id IN (SELECT id FROM document_lines WHERE document_id=?1)",[id])?;
        db.execute("DELETE FROM document_lines WHERE document_id=?1", [id])?;
        id.0
    } else {
        header(
            db,
            a,
            d.kind.code(),
            &d.date,
            d.party_id.map(|x| x.0),
            d.warehouse_id.map(|x| x.0),
            &d.reason,
        )?
    };
    let (party_name, party_address, party_tax, terms): (String, String, String, i64) = if let Some(
        p,
    ) =
        d.party_id
    {
        db.query_row("SELECT name_ar,address,tax_number,payment_terms_days FROM parties WHERE id=?1 AND active=1",[p],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?
    } else {
        ("عميل نقدي".into(), String::new(), String::new(), 0)
    };
    let due = d.due_date.clone().unwrap_or(
        date(&d.date)?
            .checked_add_signed(Duration::days(terms))
            .ok_or_else(|| Error::validation("تاريخ غير صالح"))?
            .to_string(),
    );
    date(&due)?;
    db.execute("UPDATE documents SET document_date=?1,due_date=?2,party_id=?3,warehouse_id=?4,destination_warehouse_id=?5,source_document_id=?6,price_list=?7,notes=?8,reason=?9,party_name_snapshot=?10,party_address_snapshot=?11,party_tax_number_snapshot=?12,revision=revision+1 WHERE id=?13",params![d.date,due,d.party_id,d.warehouse_id,d.destination_id,d.source_id,if d.wholesale{"wholesale"}else{"retail"},d.notes,d.reason,party_name,party_address,party_tax,id])?;
    let vat: bool = db.query_row("SELECT vat_enabled FROM store_settings", [], |r| r.get(0))?;
    for (index, l) in d.lines.iter().enumerate() {
        let(product,category,code,name,unit,num,den,retail,wholesale):(i64,i64,String,String,String,i64,i64,i64,i64)=db.query_row("SELECT p.id,p.category_id,p.code,p.name_ar,u.name_ar,pu.factor_numerator,pu.factor_denominator,pu.retail_price_minor,pu.wholesale_price_minor FROM product_units pu JOIN products p ON p.id=pu.product_id JOIN units u ON u.id=pu.unit_id WHERE pu.id=?1 AND pu.active=1 AND p.active=1",[l.unit_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)))?;
        if matches!(d.kind, DocumentKind::Sale | DocumentKind::Quotation)
            && l.price.0 != if d.wholesale { wholesale } else { retail }
        {
            a.require("sales.discount")?;
        }
        let qty = convert(l.quantity.0, num, den)?;
        let tax = if vat {
            prorate(weights[index] - shares[index], i64::from(l.tax_bps), 10000)?
        } else {
            ensure(l.tax_bps == 0, "الضريبة معطلة في الإعدادات")?;
            0
        };
        let total = sum([weights[index], -shares[index], tax])?;
        db.execute("INSERT INTO document_lines(document_id,line_number,product_id,product_unit_id,product_code_snapshot,product_name_snapshot,unit_name_snapshot,category_id_snapshot,factor_numerator,factor_denominator,quantity_micros,base_quantity_micros,unit_price_minor,subtotal_minor,line_discount_minor,invoice_discount_share_minor,tax_rate_bps,tax_minor,total_minor,notes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",params![id,(index+1)as i64,product,l.unit_id,code,name,unit,category,num,den,l.quantity,qty,l.price,gross[index],l.discount,shares[index],l.tax_bps,tax,total,l.notes])?;
        let line = db.last_insert_rowid();
        if d.kind == DocumentKind::GoodsReceipt {
            db.execute("INSERT INTO receipt_lines(document_line_id,purchase_order_line_id,damaged_quantity_micros,short_quantity_micros,inspection_notes) VALUES (?1,?2,?3,?4,?5)",params![line,l.order_line_id,convert(l.damaged.0,num,den)?,convert(l.short.0,num,den)?,l.notes])?;
        }
        if d.kind == DocumentKind::Purchase {
            for m in &l.matches {
                ensure(m.base_quantity.0 > 0, "كمية المطابقة يجب أن تكون موجبة")?;
                let(received,value):(i64,i64)=db.query_row("SELECT base_quantity_micros,subtotal_minor-line_discount_minor-invoice_discount_share_minor FROM document_lines WHERE id=?1",[m.receipt_line_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
                let used:(i64,i64)=db.query_row("SELECT COALESCE(SUM(m.base_quantity_micros),0),COALESCE(SUM(m.accrued_value_minor),0) FROM purchase_matches m JOIN document_lines l ON l.id=m.invoice_line_id JOIN documents d ON d.id=l.document_id WHERE m.receipt_line_id=?1 AND (d.status='posted' OR d.id=?2) AND NOT EXISTS(SELECT 1 FROM documents rev WHERE rev.reversal_of_id=d.id AND rev.status='posted')",params![m.receipt_line_id,id],|r|Ok((r.get(0)?,r.get(1)?)))?;
                ensure(
                    used.0 + m.base_quantity.0 <= received,
                    "كمية الفاتورة تتجاوز المستلم",
                )?;
                let accrued = if used.0 + m.base_quantity.0 == received {
                    value - used.1
                } else {
                    prorate(value, m.base_quantity.0, received)?
                };
                db.execute("INSERT INTO purchase_matches(invoice_line_id,receipt_line_id,base_quantity_micros,accrued_value_minor) VALUES (?1,?2,?3,?4)",params![line,m.receipt_line_id,m.base_quantity,accrued])?;
            }
        } else {
            ensure(
                l.matches.is_empty(),
                "مطابقة الاستلام متاحة لفاتورة المشتريات فقط",
            )?;
        }
    }
    totals(db, id)?;
    if !matches!(d.kind, DocumentKind::Sale | DocumentKind::Purchase) {
        ensure(d.payments.is_empty(), "المستند لا يقبل دفعات")?;
    }
    super::finance::save_payments(
        db,
        id,
        &d.payments,
        d.kind == DocumentKind::Purchase,
        d.party_id.map(|x| x.0),
    )?;
    event(db, a, id, "draft", "draft", "حفظ المسودة")?;
    Ok(json!({"id":Int(id),"revision":Int(if d.id.is_some(){d.revision.0+1}else{1})}))
}
pub fn kind_permission(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "sale" | "quotation" => "sales.write",
        "purchase" | "purchase_order" => "purchases.write",
        "goods_receipt" => "inventory.receive",
        "transfer" => "inventory.transfer",
        "opening_stock" | "stocktake" => "inventory.adjust",
        _ => return Err(Error::validation("نوع المستند غير متاح لهذه العملية")),
    })
}
pub fn action(
    db: &Connection,
    a: &Actor,
    id: i64,
    revision: i64,
    action: DocumentAction,
    reason: &str,
    ack: bool,
) -> Result<Value> {
    let (kind, status, current): (String, String, i64) = db.query_row(
        "SELECT kind,status,revision FROM documents WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    a.require(kind_permission(&kind)?)?;
    ensure(current == revision, "تم تعديل المستند؛ أعد تحميله")?;
    if action == DocumentAction::Post {
        return post(db, a, id, reason, ack);
    }
    let next = match action {
        DocumentAction::Hold => "held",
        DocumentAction::Resume => "draft",
        DocumentAction::Approve => "approved",
        DocumentAction::Close => "closed",
        DocumentAction::Cancel => "cancelled",
        DocumentAction::Post => unreachable!(),
    };
    if action == DocumentAction::Approve {
        if kind == "purchase_order" {
            a.require("purchases.approve")?;
        }
        ensure(!lines(db, id)?.is_empty(), "المستند فارغ")?;
        number(db, id)?;
    }
    if matches!(action, DocumentAction::Close | DocumentAction::Cancel) {
        ensure(!reason.trim().is_empty(), "سبب الإغلاق أو الإلغاء مطلوب")?;
    }
    db.execute(
        "UPDATE documents SET status=?1,reason=?2,revision=revision+1 WHERE id=?3",
        params![next, reason, id],
    )?;
    event(db, a, id, &status, next, reason)?;
    Ok(json!({"id":Int(id),"status":next}))
}
pub fn post(db: &Connection, a: &Actor, id: i64, reason: &str, ack: bool) -> Result<Value> {
    let(kind,party,warehouse,destination,total,due):(String,Option<i64>,Option<i64>,Option<i64>,i64,String)=db.query_row("SELECT kind,party_id,warehouse_id,destination_warehouse_id,total_minor,due_date FROM documents WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    a.require(kind_permission(&kind)?)?;
    if kind == "stocktake" {
        return inventory::post_count(db, a, id);
    }
    ensure(
        !matches!(kind.as_str(), "purchase_order" | "quotation"),
        "اعتمد المستند بدلاً من ترحيله",
    )?;
    let paid: i64 = db.query_row(
        "SELECT COALESCE(SUM(amount_minor),0) FROM payment_legs WHERE document_id=?1",
        [id],
        |r| r.get(0),
    )?;
    ensure(paid <= total, "الدفعات تتجاوز قيمة الفاتورة")?;
    if kind == "sale" {
        ensure(
            party.is_some() || paid == total,
            "البيع الآجل يحتاج إلى عميل",
        )?;
        if let Some(p) = party {
            super::finance::credit_check(db, a, p, total - paid, reason, ack)?;
        }
    }
    begin(db, id)?;
    let mut journal = Journal::default();
    for l in lines(db, id)? {
        let net = l.gross - l.discount - l.share;
        match kind.as_str() {
            "sale" => {
                journal.add(Entry::item(
                    super::finance::account(db, "sales")?,
                    -net,
                    party,
                    &l,
                ));
                journal.add(Entry::new(
                    super::finance::account(db, "vat_output")?,
                    -l.tax,
                ));
                let cost = inventory::issue(
                    db,
                    a,
                    id,
                    Some(l.id),
                    l.product,
                    warehouse.ok_or_else(|| Error::validation("المخزن مطلوب"))?,
                    l.qty,
                    "sale",
                )?;
                journal.add(Entry::item(
                    super::finance::account(db, "cogs")?,
                    cost,
                    party,
                    &l,
                ));
                journal.add(Entry::item(
                    super::finance::account(db, "inventory")?,
                    -cost,
                    party,
                    &l,
                ));
            }
            "goods_receipt" | "opening_stock" => {
                if l.qty > 0 {
                    let (stock, correction) = inventory::receive(
                        db,
                        a,
                        id,
                        Some(l.id),
                        l.product,
                        warehouse.ok_or_else(|| Error::validation("المخزن مطلوب"))?,
                        l.qty,
                        net,
                        if kind == "opening_stock" {
                            "opening"
                        } else {
                            "receipt"
                        },
                    )?;
                    journal.add(Entry::item(
                        super::finance::account(db, "inventory")?,
                        stock,
                        party,
                        &l,
                    ));
                    journal.add(Entry::item(
                        super::finance::account(db, "cogs")?,
                        correction,
                        party,
                        &l,
                    ));
                    journal.add(Entry::new(
                        super::finance::account(
                            db,
                            if kind == "opening_stock" {
                                "opening_equity"
                            } else {
                                "grni"
                            },
                        )?,
                        -net,
                    ));
                }
            }
            "transfer" => {
                let from = warehouse.ok_or_else(|| Error::validation("المخزن مطلوب"))?;
                let to = destination.ok_or_else(|| Error::validation("مخزن الاستلام مطلوب"))?;
                let cost = inventory::issue(
                    db,
                    a,
                    id,
                    Some(l.id),
                    l.product,
                    from,
                    l.qty,
                    "transfer_out",
                )?;
                let mut destination_pool = inventory::pool(db, l.product, to)?;
                let (_, correction) = destination_pool.receive(l.qty, cost)?;
                // Transfer legs conserve value; a separate atomic correction resolves provisional COGS.
                inventory::movement(
                    db,
                    a,
                    id,
                    Some(l.id),
                    l.product,
                    to,
                    l.qty,
                    cost,
                    "transfer_in",
                    None,
                )?;
                if correction != 0 {
                    let day: String = db.query_row(
                        "SELECT document_date FROM documents WHERE id=?1",
                        [id],
                        |r| r.get(0),
                    )?;
                    let adjustment = header(
                        db,
                        a,
                        "cost_adjustment",
                        &day,
                        None,
                        Some(to),
                        "تسوية تكلفة تحويل إلى مخزون سالب",
                    )?;
                    db.execute(
                        "UPDATE documents SET source_document_id=?1 WHERE id=?2",
                        params![id, adjustment],
                    )?;
                    begin(db, adjustment)?;
                    inventory::movement(
                        db,
                        a,
                        adjustment,
                        None,
                        l.product,
                        to,
                        0,
                        -correction,
                        "revaluation",
                        None,
                    )?;
                    let mut j = Journal::default();
                    let mut e = Entry::new(super::finance::account(db, "cogs")?, correction);
                    e.product = Some(l.product);
                    e.category = Some(l.category);
                    j.add(e);
                    j.add(Entry::new(
                        super::finance::account(db, "inventory")?,
                        -correction,
                    ));
                    j.write(db, adjustment)?;
                    finish(db, a, adjustment)?;
                }
            }
            "purchase" => {
                let accrued:i64=db.query_row("SELECT COALESCE(SUM(accrued_value_minor),0) FROM purchase_matches WHERE invoice_line_id=?1",[l.id],|r|r.get(0))?;
                journal.add(Entry::new(super::finance::account(db, "grni")?, accrued));
                journal.add(Entry::new(super::finance::account(db, "vat_input")?, l.tax));
                let delta = net - accrued;
                if delta != 0 {
                    inventory::invoice_cost_delta(db, a, id, &l, delta, &mut journal)?;
                }
            }
            _ => return Err(Error::validation("نوع المستند غير صالح للترحيل")),
        }
    }
    if kind == "sale" || kind == "purchase" {
        if let Some(p) = party {
            journal.add(Entry::party(
                super::finance::account(
                    db,
                    if kind == "sale" {
                        "receivable"
                    } else {
                        "payable"
                    },
                )?,
                if kind == "sale" { total } else { -total },
                p,
                &due,
            ));
        }
        super::finance::post_payments(db, a, id, party, kind == "purchase", &mut journal)?;
    }
    journal.write(db, id)?;
    super::finance::match_document_payments(db, id)?;
    finish(db, a, id)
}
pub fn convert_quote(db: &Connection, a: &Actor, id: i64, day: &str) -> Result<Value> {
    date(day)?;
    let (kind, status): (String, String) =
        db.query_row("SELECT kind,status FROM documents WHERE id=?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    ensure(
        kind == "quotation" && status == "approved",
        "يجب اعتماد عرض الأسعار أولاً",
    )?;
    let result = clone_draft(db, a, id, day, "sale")?;
    db.execute("UPDATE documents SET status='closed',reason='تحويل إلى فاتورة',revision=revision+1 WHERE id=?1",[id])?;
    event(db, a, id, "approved", "closed", "تحويل إلى فاتورة")?;
    Ok(result)
}
fn clone_draft(db: &Connection, a: &Actor, source: i64, day: &str, kind: &str) -> Result<Value> {
    let (party, warehouse): (Option<i64>, Option<i64>) = db.query_row(
        "SELECT party_id,warehouse_id FROM documents WHERE id=?1",
        [source],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let id = header(db, a, kind, day, party, warehouse, "")?;
    db.execute(
        "UPDATE documents SET source_document_id=?1 WHERE id=?2",
        params![source, id],
    )?;
    db.execute("INSERT INTO document_lines(document_id,line_number,product_id,product_unit_id,product_code_snapshot,product_name_snapshot,unit_name_snapshot,category_id_snapshot,factor_numerator,factor_denominator,quantity_micros,base_quantity_micros,unit_price_minor,subtotal_minor,line_discount_minor,invoice_discount_share_minor,tax_rate_bps,tax_minor,total_minor,notes) SELECT ?1,line_number,product_id,product_unit_id,product_code_snapshot,product_name_snapshot,unit_name_snapshot,category_id_snapshot,factor_numerator,factor_denominator,quantity_micros,base_quantity_micros,unit_price_minor,subtotal_minor,line_discount_minor,invoice_discount_share_minor,tax_rate_bps,tax_minor,total_minor,notes FROM document_lines WHERE document_id=?2",params![id,source])?;
    totals(db, id)?;
    Ok(json!({"id":Int(id),"status":"draft","revision":"0"}))
}
