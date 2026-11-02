use super::documents;
use crate::{
    db,
    error::{Error, Result, ensure},
    models::{
        arithmetic::{narrow, parse_decimal, sum},
        *,
    },
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

fn reason(text: &str) -> Result<()> {
    ensure(
        !text.trim().is_empty() && text.len() <= 2000,
        "السبب مطلوب ولا يتجاوز ألفي حرف",
    )
}
pub fn parse_csv(db: &Connection, a: &Actor, text: &str) -> Result<Value> {
    a.require("finance.reconcile")?;
    ensure(text.len() <= 2_000_000, "ملف الكشف يتجاوز الحجم المسموح")?;
    let digits: u32 = db.query_row(
        "SELECT currency_minor_digits FROM store_settings WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    let mut csv = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    ensure(
        csv.headers()
            .map_err(|_| Error::validation("ترويسة CSV غير صالحة"))?
            .iter()
            .eq(["date", "amount", "reference", "description"]),
        "ترويسة CSV المطلوبة: date,amount,reference,description",
    )?;
    let mut lines = Vec::new();
    for row in csv.records() {
        let row = row.map_err(|_| Error::validation("سطر CSV غير صالح"))?;
        ensure(lines.len() < 5000, "الكشف يتجاوز خمسة آلاف سطر")?;
        let day = &row[0];
        documents::date(day)?;
        let amount = parse_decimal(&row[1], digits)?;
        ensure(
            amount != 0 && amount.unsigned_abs() <= 1_000_000_000_000_000,
            "مبلغ سطر الكشف غير صالح",
        )?;
        ensure(
            row[2].len() <= 200 && row[3].len() <= 1000,
            "نص سطر الكشف طويل",
        )?;
        lines.push(BankStatementLine {
            date: day.into(),
            amount: Int(amount),
            reference: row[2].into(),
            description: row[3].into(),
        });
    }
    ensure(!lines.is_empty(), "الكشف فارغ")?;
    Ok(json!(lines))
}
pub fn save(db: &Connection, a: &Actor, s: &BankStatement) -> Result<Value> {
    a.require("finance.reconcile")?;
    documents::date(&s.starts_on)?;
    documents::date(&s.ends_on)?;
    ensure(
        s.starts_on <= s.ends_on && !s.reference.trim().is_empty() && s.reference.len() <= 200,
        "تواريخ الكشف أو مرجعه غير صالحة",
    )?;
    ensure(
        !s.lines.is_empty() && s.lines.len() <= 5000,
        "الكشف يحتاج من سطر إلى خمسة آلاف سطر",
    )?;
    ensure(
        s.opening.0.unsigned_abs() <= 2_000_000_000_000_000
            && s.closing.0.unsigned_abs() <= 2_000_000_000_000_000,
        "رصيد الكشف غير صالح",
    )?;
    for l in &s.lines {
        documents::date(&l.date)?;
        ensure(
            l.date >= s.starts_on
                && l.date <= s.ends_on
                && l.amount.0 != 0
                && l.amount.0.unsigned_abs() <= 1_000_000_000_000_000
                && l.reference.len() <= 200
                && l.description.len() <= 1000,
            "سطر الكشف غير صالح",
        )?;
    }
    ensure(
        sum(std::iter::once(s.opening.0).chain(s.lines.iter().map(|l| l.amount.0)))? == s.closing.0,
        "الرصيد الافتتاحي وحركات الكشف لا يساويان الرصيد الختامي",
    )?;
    let account: i64 = db.query_row(
        "SELECT account_id FROM cash_accounts WHERE id=?1 AND active=1",
        [s.cash_account_id],
        |r| r.get(0),
    )?;
    // Reference is intentionally excluded: renaming an imported file cannot import it twice.
    let hash = db::digest(
        serde_json::to_string(&json!([
            s.starts_on,
            s.ends_on,
            s.opening,
            s.closing,
            s.lines
        ]))?
        .as_bytes(),
    );
    db.execute("INSERT INTO bank_statements(cash_account_id,account_id,starts_on,ends_on,reference,opening_minor,closing_minor,source_hash,created_by,line_count) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![s.cash_account_id,account,s.starts_on,s.ends_on,s.reference.trim(),s.opening,s.closing,hash,a.user_id,s.lines.len() as i64])?;
    let id = db.last_insert_rowid();
    for (i, l) in s.lines.iter().enumerate() {
        db.execute("INSERT INTO bank_statement_lines(statement_id,line_number,value_date,amount_minor,reference,description) VALUES (?1,?2,?3,?4,?5,?6)",params![id,(i+1)as i64,l.date,l.amount,l.reference,l.description])?;
    }
    Ok(json!({"id":Int(id)}))
}
pub fn list(db: &Connection, a: &Actor, after: i64, limit: u16) -> Result<Value> {
    a.require("finance.reconcile")?;
    Ok(json!(db::rows(
        db,
        "SELECT s.*,c.name_ar,CASE WHEN v.statement_id IS NULL THEN 'active' ELSE 'cancelled' END status FROM bank_statements s JOIN cash_accounts c ON c.id=s.cash_account_id LEFT JOIN bank_statement_voids v ON v.statement_id=s.id WHERE s.id>?1 ORDER BY s.id LIMIT ?2",
        params![after, i64::from(limit.clamp(1, 200))]
    )?))
}
pub fn query(
    db: &Connection,
    a: &Actor,
    id: i64,
    view: ReconciliationView,
    limit: u16,
    offset: u32,
) -> Result<Value> {
    a.require("finance.reconcile")?;
    let (account, end, closing): (i64, String, i64) = db.query_row(
        "SELECT account_id,ends_on,closing_minor FROM bank_statements WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let sql = match view {
        ReconciliationView::Statement => {
            "SELECT l.*,abs(l.amount_minor)-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE statement_line_id=l.id),0) remaining_minor FROM bank_statement_lines l WHERE l.statement_id=?1 ORDER BY l.id LIMIT ?2 OFFSET ?3"
        }
        ReconciliationView::Matches => {
            "SELECT m.*,d.document_date,d.number AS document_number,d.reason,l.line_number,src.document_id AS source_document_id,CASE WHEN m.amount_minor<0 THEN 'reversal' WHEN EXISTS(SELECT 1 FROM reconciliation_matches r WHERE r.reversal_of_id=m.id) THEN 'cancelled' ELSE 'active' END status FROM reconciliation_matches m JOIN bank_statement_lines l ON l.id=m.statement_line_id JOIN documents d ON d.id=m.document_id JOIN journal_lines src ON src.id=m.journal_line_id WHERE l.statement_id=?1 ORDER BY m.id DESC LIMIT ?2 OFFSET ?3"
        }
        ReconciliationView::Journal => {
            "SELECT j.id,j.document_id,d.number AS document_number,d.document_date,j.debit_minor-j.credit_minor AS amount_minor,j.memo,j.debit_minor+j.credit_minor-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE journal_line_id=j.id),0) remaining_minor FROM journal_lines j JOIN documents d ON d.id=j.document_id JOIN bank_statements s ON s.id=?1 WHERE j.account_id=s.account_id AND d.status='posted' AND d.document_date<=s.ends_on AND j.debit_minor+j.credit_minor>COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE journal_line_id=j.id),0) AND d.kind<>'reversal' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted') ORDER BY d.document_date,j.id LIMIT ?2 OFFSET ?3"
        }
    };
    let rows = db::rows(db, sql, params![id, i64::from(limit.clamp(1, 200)), offset])?;
    let book:i64=db.query_row("SELECT COALESCE(SUM(j.debit_minor-j.credit_minor),0) FROM journal_lines j JOIN documents d ON d.id=j.document_id WHERE j.account_id=?1 AND d.status='posted' AND d.document_date<=?2",params![account,end],|r|r.get(0))?;
    let unmatched_statement:i64=db.query_row("SELECT COALESCE(SUM(abs(l.amount_minor)-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE statement_line_id=l.id),0)),0) FROM bank_statement_lines l WHERE statement_id=?1",[id],|r|r.get(0))?;
    let unmatched_journal:i64=db.query_row("SELECT COALESCE(SUM(j.debit_minor+j.credit_minor-COALESCE((SELECT SUM(amount_minor) FROM reconciliation_matches WHERE journal_line_id=j.id),0)),0) FROM journal_lines j JOIN documents d ON d.id=j.document_id WHERE j.account_id=?1 AND d.status='posted' AND d.document_date<=?2 AND d.kind<>'reversal' AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted')",params![account,end],|r|r.get(0))?;
    Ok(
        json!({"rows":rows,"summary":[{"metric":"statementClosing","amount_minor":Int(closing)},{"metric":"bookClosing","amount_minor":Int(book)},{"metric":"bankReconciliationDifference","amount_minor":Int(narrow(i128::from(closing)-i128::from(book))?)},{"metric":"unmatchedStatement","amount_minor":Int(unmatched_statement)},{"metric":"unmatchedJournal","amount_minor":Int(unmatched_journal)}]}),
    )
}
pub fn post_match(
    db: &Connection,
    a: &Actor,
    line: i64,
    journal: i64,
    amount: i64,
    day: &str,
    why: &str,
) -> Result<Value> {
    a.require("finance.reconcile")?;
    reason(why)?;
    ensure(
        (1..=1_000_000_000_000_000).contains(&amount),
        "مبلغ المطابقة غير صالح",
    )?;
    let doc = documents::header(db, a, "bank_match", day, None, None, why)?;
    documents::begin(db, doc)?;
    db.execute("INSERT INTO reconciliation_matches(document_id,statement_line_id,journal_line_id,amount_minor) VALUES (?1,?2,?3,?4)",params![doc,line,journal,amount])?;
    documents::finish(db, a, doc)
}
pub fn unmatch(db: &Connection, a: &Actor, id: i64, day: &str, why: &str) -> Result<Value> {
    a.require("finance.reconcile")?;
    reason(why)?;
    let (line,journal,amount):(i64,i64,i64)=db.query_row("SELECT statement_line_id,journal_line_id,amount_minor FROM reconciliation_matches WHERE id=?1 AND amount_minor>0",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let doc = documents::header(db, a, "bank_match", day, None, None, why)?;
    documents::begin(db, doc)?;
    db.execute("INSERT INTO reconciliation_matches(document_id,statement_line_id,journal_line_id,amount_minor,reversal_of_id) VALUES (?1,?2,?3,?4,?5)",params![doc,line,journal,-amount,id])?;
    documents::finish(db, a, doc)
}
pub fn void(db: &Connection, a: &Actor, id: i64, day: &str, why: &str) -> Result<Value> {
    a.require("finance.reconcile")?;
    reason(why)?;
    let end: String = db.query_row(
        "SELECT ends_on FROM bank_statements WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    ensure(day >= end.as_str(), "تاريخ الإلغاء يسبق نهاية الكشف")?;
    let doc = documents::header(db, a, "bank_match", day, None, None, why)?;
    documents::begin(db, doc)?;
    db.execute(
        "INSERT INTO bank_statement_voids(statement_id,document_id) VALUES (?1,?2)",
        params![id, doc],
    )?;
    documents::finish(db, a, doc)
}
