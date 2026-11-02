use super::{documents, purchasing::reason};
use crate::{
    error::{Result, ensure},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub fn classified(db: &Connection, id: i64) -> Result<bool> {
    Ok(db.query_row(
        "SELECT is_quarantine FROM warehouses WHERE id=?1 AND active=1",
        [id],
        |r| r.get(0),
    )?)
}
pub fn sale_warehouse(db: &Connection, id: i64) -> Result<()> {
    ensure(
        !classified(db, id)?,
        "لا يمكن البيع من مخزن العزل؛ أفرج عن البضاعة بتحويل أولاً",
    )
}
pub fn set_warehouse(
    db: &Connection,
    a: &Actor,
    id: i64,
    revision: i64,
    quarantine: bool,
    why: &str,
) -> Result<Value> {
    a.require("inventory.quarantine_manage")?;
    reason(why)?;
    ensure(db.execute("UPDATE warehouses SET is_quarantine=?1,quarantine_revision=quarantine_revision+1 WHERE id=?2 AND quarantine_revision=?3",params![quarantine,id,revision])?==1,"تغير تصنيف المخزن؛ أعد تحميله")?;
    Ok(json!({"id":Int(id)}))
}
pub fn transfer_guard(db: &Connection, a: &Actor, doc: i64) -> Result<()> {
    let (from, to, why): (i64, i64, String) = db.query_row(
        "SELECT warehouse_id,destination_warehouse_id,reason FROM documents WHERE id=?1",
        [doc],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let f = classified(db, from)?;
    let t = classified(db, to)?;
    if f || t {
        a.require("inventory.quarantine")?;
        reason(&why)?;
        db.execute("INSERT INTO quarantine_transfers(document_id,from_quarantine,to_quarantine) VALUES (?1,?2,?3)",params![doc,f,t])?;
    }
    Ok(())
}
pub fn transfer(
    db: &Connection,
    a: &Actor,
    day: &str,
    from: i64,
    to: i64,
    lines: &[BundleItem],
    why: &str,
) -> Result<Value> {
    a.require("inventory.quarantine")?;
    a.require("inventory.transfer")?;
    reason(why)?;
    ensure(
        from != to && classified(db, from)? != classified(db, to)?,
        "اختر مخزن عزل ومخزناً عادياً مختلفين",
    )?;
    let d = Draft {
        id: None,
        revision: Int(0),
        kind: DocumentKind::Transfer,
        date: day.into(),
        due_date: None,
        party_id: None,
        warehouse_id: Some(Int(from)),
        destination_id: Some(Int(to)),
        source_id: None,
        wholesale: false,
        discount: Int(0),
        notes: String::new(),
        reason: why.into(),
        payments: vec![],
        lines: lines
            .iter()
            .map(|i| DraftLine {
                unit_id: i.unit_id,
                quantity: i.quantity,
                price: Int(0),
                discount: Int(0),
                tax_bps: 0,
                order_line_id: None,
                damaged: Int(0),
                short: Int(0),
                matches: vec![],
                notes: String::new(),
            })
            .collect(),
    };
    let saved = documents::save(db, a, &d)?;
    documents::post(
        db,
        a,
        saved["id"].as_str().unwrap().parse().unwrap(),
        why,
        false,
    )
}
