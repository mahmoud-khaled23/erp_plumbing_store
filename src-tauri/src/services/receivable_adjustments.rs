use super::{
    documents,
    finance::{self, Entry, Journal},
};
use crate::{
    db,
    error::{Result, ensure},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub fn open_items(
    db: &Connection,
    a: &Actor,
    search: &str,
    after: i64,
    limit: u16,
) -> Result<Value> {
    a.require("finance.read")?;
    ensure(search.len() <= 256, "نص البحث طويل")?;
    Ok(json!(db::rows(
        db,
        "SELECT o.*,p.name_ar FROM v_party_open_items o JOIN parties p ON p.id=o.party_id WHERE control_kind='receivable' AND outstanding_signed_minor>0 AND journal_line_id>?1 AND (?2='' OR instr(p.name_ar,?2)>0 OR instr(o.document_number,?2)>0) ORDER BY journal_line_id LIMIT ?3",
        params![after, search, i64::from(limit.clamp(1, 200))]
    )?))
}

pub fn post(
    db: &Connection,
    a: &Actor,
    kind: ReceivableAdjustmentKind,
    day: &str,
    debit: i64,
    amount: i64,
    reason: &str,
) -> Result<Value> {
    a.require(kind.permission())?;
    ensure(
        !reason.trim().is_empty() && reason.len() <= 2000,
        "سبب التسوية مطلوب ولا يتجاوز ألفي حرف",
    )?;
    ensure(
        (1..=1_000_000_000_000_000).contains(&amount),
        "مبلغ التسوية غير صالح",
    )?;
    documents::date(day)?;
    let (party,account,source,outstanding,source_day):(i64,i64,i64,i64,String)=db.query_row("SELECT party_id,account_id,document_id,outstanding_signed_minor,document_date FROM v_party_open_items WHERE journal_line_id=?1 AND control_kind='receivable' AND original_signed_minor>0",[debit],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    ensure(
        amount <= outstanding && day >= source_day.as_str(),
        "المبلغ يتجاوز الدين المفتوح أو التاريخ يسبق المستند",
    )?;
    ensure(!db.query_row("SELECT EXISTS(SELECT 1 FROM settlements s JOIN documents d ON d.id=s.document_id WHERE s.debit_line_id=?1 AND d.document_date>?2) OR EXISTS(SELECT 1 FROM documents WHERE reversal_of_id=?3 AND status='posted')",params![debit,day,source],|r|r.get::<_,bool>(0))?,"توجد تسويات لاحقة أو المستند معكوس؛ استخدم تاريخاً حالياً")?;
    let doc = documents::header(db, a, kind.code(), day, Some(party), None, reason)?;
    db.execute(
        "UPDATE documents SET source_document_id=?1,due_date=?3 WHERE id=?2",
        params![source, doc, day],
    )?;
    finance::set_total(db, doc, amount, 0)?;
    db.execute("INSERT INTO receivable_adjustments(document_id,source_debit_line_id,amount_minor) VALUES (?1,?2,?3)",params![doc,debit,amount])?;
    documents::begin(db, doc)?;
    let key = match kind {
        ReceivableAdjustmentKind::BadDebt => "bad_debt",
        ReceivableAdjustmentKind::CustomerCredit => "sales_returns",
        ReceivableAdjustmentKind::SettlementDiscount => "sales_discounts",
    };
    let mut journal = Journal::default();
    journal.add(Entry::new(finance::account(db, key)?, amount));
    journal.add(Entry::party(account, -amount, party, day));
    journal.write(db, doc)?;
    let credit: i64 = db.query_row(
        "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=?2 AND credit_minor>0",
        params![doc, account],
        |r| r.get(0),
    )?;
    db.execute("INSERT INTO settlements(document_id,debit_line_id,credit_line_id,amount_minor) VALUES (?1,?2,?3,?4)",params![doc,debit,credit,amount])?;
    documents::finish(db, a, doc)
}
