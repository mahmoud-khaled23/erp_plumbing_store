use super::{
    documents,
    finance::{self, Entry, Journal},
};
use crate::{
    db,
    error::{Result, ensure},
    models::{
        Int,
        arithmetic::{narrow, sum},
    },
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

struct Preview {
    rows: Vec<Value>,
    hash: String,
    result: i64,
}

fn calculate(db: &Connection, period: i64, day: &str) -> Result<Preview> {
    documents::date(day)?;
    let (end, locked): (String, bool) = db.query_row(
        "SELECT ends_on,locked_at IS NOT NULL FROM fiscal_periods WHERE id=?1",
        [period],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (next_start,next_end,next_locked):(String,String,bool)=db.query_row("SELECT starts_on,ends_on,locked_at IS NOT NULL FROM fiscal_periods WHERE starts_on=date(?1,'+1 day')",[&end],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    ensure(!next_locked, "يجب أن تكون الفترة التالية مباشرة مفتوحة")?;
    ensure(!db.query_row("SELECT EXISTS(SELECT 1 FROM year_closings c JOIN documents d ON d.id=c.document_id WHERE c.period_id=?1 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted'))",[period],|r|r.get::<_,bool>(0))?,"الفترة مقفلة بالفعل؛ اعكس الإقفال أولاً")?;
    ensure(!db.query_row("SELECT EXISTS(SELECT 1 FROM year_closings c JOIN fiscal_periods p ON p.id=c.period_id JOIN documents d ON d.id=c.document_id WHERE p.ends_on>?1 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted'))",[&end],|r|r.get::<_,bool>(0))?,"اعكس إقفال الفترات اللاحقة أولاً")?;
    let last_reversal:Option<String>=db.query_row("SELECT MAX(r.document_date) FROM year_closings c JOIN documents r ON r.reversal_of_id=c.document_id AND r.status='posted' WHERE c.period_id=?1",[period],|r|r.get(0))?;
    if let Some(last) = last_reversal {
        ensure(
            day >= next_start.as_str() && day <= next_end.as_str() && day >= last.as_str(),
            "إعادة الإقفال تكون في الفترة التالية وبعد تاريخ العكس",
        )?;
    } else {
        ensure(
            !locked && day == end,
            "الإقفال الأول يكون بتاريخ نهاية الفترة المفتوحة",
        )?;
    }
    // Include later reopen/reclose entries belonging to this subject period only.
    // Other operations in the next period are deliberately excluded.
    let rows = db::rows(
        db,
        "SELECT j.account_id,a.code AS account_code,a.name_ar AS account_name,SUM(j.debit_minor-j.credit_minor) AS balance_minor FROM journal_lines j JOIN accounts a ON a.id=j.account_id JOIN documents d ON d.id=j.document_id WHERE d.status='posted' AND a.kind IN ('income','expense') AND (d.document_date<=?1 OR (d.document_date<=?2 AND (d.id IN (SELECT document_id FROM year_closings WHERE period_id=?3) OR d.reversal_of_id IN (SELECT document_id FROM year_closings WHERE period_id=?3)))) GROUP BY j.account_id HAVING SUM(j.debit_minor-j.credit_minor)<>0 ORDER BY j.account_id",
        params![end, day, period],
    )?;
    let balances = rows
        .iter()
        .map(|r| r["balance_minor"].as_str().unwrap().parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    let result = narrow(-i128::from(sum(balances)?))?;
    let hash = db::digest(
        serde_json::to_string(&json!([
            period,
            day,
            end,
            locked,
            next_start,
            next_end,
            next_locked,
            rows
        ]))?
        .as_bytes(),
    );
    Ok(Preview { rows, hash, result })
}
pub fn preview(db: &Connection, a: &Actor, period: i64, day: &str) -> Result<Value> {
    a.require("finance.close")?;
    let p = calculate(db, period, day)?;
    Ok(json!({"rows":p.rows,"preview_hash":p.hash,"result_minor":Int(p.result)}))
}
pub fn post(
    db: &Connection,
    a: &Actor,
    period: i64,
    day: &str,
    hash: &str,
    reason: &str,
) -> Result<Value> {
    a.require("finance.close")?;
    ensure(
        !reason.trim().is_empty() && reason.len() <= 2000,
        "سبب الإقفال مطلوب",
    )?;
    let p = calculate(db, period, day)?;
    ensure(hash == p.hash, "تغيرت أرصدة الإقفال؛ أعد المعاينة")?;
    let doc = documents::header(db, a, "year_end_close", day, None, None, reason)?;
    db.execute("INSERT INTO year_closings(document_id,period_id,result_minor,preview_hash) VALUES (?1,?2,?3,?4)",params![doc,period,p.result,p.hash])?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    fn add(j: &mut Journal, account: i64, amount: i64) -> Result<()> {
        let mut rest = i128::from(amount);
        while rest != 0 {
            let part = rest.clamp(-2_000_000_000_000_000, 2_000_000_000_000_000);
            j.add(Entry::new(account, narrow(part)?));
            rest -= part;
            ensure(
                j.entries.len() <= 10000,
                "عدد سطور الإقفال يتجاوز الحد المسموح",
            )?;
        }
        Ok(())
    }
    for row in p.rows {
        let account = row["account_id"].as_str().unwrap().parse::<i64>().unwrap();
        let balance = row["balance_minor"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        add(&mut journal, account, narrow(-i128::from(balance))?)?;
    }
    add(
        &mut journal,
        finance::account(db, "retained_earnings")?,
        narrow(-i128::from(p.result))?,
    )?;
    journal.write(db, doc)?;
    let result = documents::finish(db, a, doc)?;
    db.execute("UPDATE fiscal_periods SET locked_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),locked_by=?1 WHERE id=?2",params![a.user_id,period])?;
    Ok(result)
}
pub fn allow_reverse(db: &Connection, a: &Actor, source: i64, day: &str) -> Result<()> {
    a.require("finance.close_reverse")?;
    let end:String=db.query_row("SELECT p.ends_on FROM year_closings c JOIN fiscal_periods p ON p.id=c.period_id WHERE c.document_id=?1",[source],|r|r.get(0))?;
    ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM fiscal_periods WHERE starts_on=date(?1,'+1 day') AND ?2 BETWEEN starts_on AND ends_on AND locked_at IS NULL)",params![end,day],|r|r.get::<_,bool>(0))?,"عكس الإقفال متاح فقط داخل الفترة التالية المفتوحة")?;
    ensure(!db.query_row("SELECT EXISTS(SELECT 1 FROM year_closings c JOIN fiscal_periods p ON p.id=c.period_id JOIN documents d ON d.id=c.document_id WHERE p.ends_on>?1 AND d.status='posted' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted'))",[end],|r|r.get::<_,bool>(0))?,"اعكس إقفال الفترات اللاحقة أولاً")?;
    Ok(())
}
