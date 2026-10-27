use super::documents::{self, Line};
use crate::{
    error::{Error, Result, ensure},
    models::arithmetic::{prorate, sum},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub fn account(db: &Connection, key: &str) -> Result<i64> {
    Ok(db.query_row(
        "SELECT account_id FROM posting_accounts WHERE key=?1",
        [key],
        |r| r.get(0),
    )?)
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub account: i64,
    pub amount: i64,
    pub party: Option<i64>,
    pub due: Option<String>,
    pub product: Option<i64>,
    pub category: Option<i64>,
    pub line: Option<i64>,
    pub reversal: Option<i64>,
    pub memo: String,
}
impl Entry {
    pub fn new(account: i64, amount: i64) -> Self {
        Self {
            account,
            amount,
            party: None,
            due: None,
            product: None,
            category: None,
            line: None,
            reversal: None,
            memo: String::new(),
        }
    }
    pub fn party(account: i64, amount: i64, party: i64, due: &str) -> Self {
        Self {
            party: Some(party),
            due: Some(due.into()),
            ..Self::new(account, amount)
        }
    }
    pub fn item(account: i64, amount: i64, party: Option<i64>, line: &Line) -> Self {
        Self {
            party,
            product: Some(line.product),
            category: Some(line.category),
            line: Some(line.id),
            ..Self::new(account, amount)
        }
    }
}
#[derive(Default)]
pub struct Journal {
    pub entries: Vec<Entry>,
}
impl Journal {
    pub fn add(&mut self, e: Entry) {
        if e.amount != 0 {
            self.entries.push(e);
        }
    }
    pub fn write(&self, db: &Connection, doc: i64) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        ensure(
            self.entries.len() >= 2 && sum(self.entries.iter().map(|e| e.amount))? == 0,
            "القيد المحاسبي غير متوازن",
        )?;
        db.execute(
            "INSERT INTO journal_entries(document_id) VALUES (?1)",
            [doc],
        )?;
        let entry = db.last_insert_rowid();
        for (i, e) in self.entries.iter().enumerate() {
            let control: Option<String> = db.query_row(
                "SELECT control_kind FROM accounts WHERE id=?1",
                [e.account],
                |r| r.get(0),
            )?;
            if matches!(control.as_deref(), Some("receivable" | "payable")) {
                let party = e
                    .party
                    .ok_or_else(|| Error::validation("حساب الطرف يحتاج إلى عميل أو مورد"))?;
                ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM parties WHERE id=?1 AND active=1 AND CASE WHEN ?2='receivable' THEN is_customer ELSE is_supplier END=1)",params![party,control],|r|r.get(0))?,"نوع الطرف لا يطابق الحساب")?;
                documents::date(
                    e.due
                        .as_deref()
                        .ok_or_else(|| Error::validation("تاريخ الاستحقاق مطلوب"))?,
                )?;
            }
            db.execute("INSERT INTO journal_lines(entry_id,document_id,line_number,account_id,party_id,product_id,category_id,document_line_id,reverses_line_id,due_date,debit_minor,credit_minor,memo) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![entry,doc,(i+1)as i64,e.account,e.party,e.product,e.category,e.line,e.reversal,e.due,e.amount.max(0),e.amount.checked_neg().ok_or_else(||Error::validation("المبلغ كبير جداً"))?.max(0),e.memo])?;
        }
        Ok(())
    }
}
pub fn set_total(db: &Connection, doc: i64, net: i64, tax: i64) -> Result<()> {
    db.execute(
        "UPDATE documents SET subtotal_minor=?1,tax_minor=?2,total_minor=?3 WHERE id=?4",
        params![net, tax, sum([net, tax])?, doc],
    )?;
    Ok(())
}
pub fn save_payments(
    db: &Connection,
    doc: i64,
    payments: &[Payment],
    outgoing: bool,
    party: Option<i64>,
) -> Result<()> {
    ensure(payments.len() <= 20, "عدد الدفعات كبير")?;
    for p in payments {
        ensure(
            p.amount.0 > 0 && p.amount.0 <= 1_000_000_000_000_000,
            "مبلغ الدفعة غير صالح",
        )?;
        if p.method != PaymentMethod::Cheque {
            let cash = p
                .cash_account_id
                .ok_or_else(|| Error::validation("حساب الدفع مطلوب"))?;
            let kind: String = db.query_row(
                "SELECT kind FROM cash_accounts WHERE id=?1 AND active=1",
                [cash],
                |r| r.get(0),
            )?;
            ensure(
                kind == p.method.code() && p.cheque.is_none(),
                "طريقة الدفع لا تطابق الحساب",
            )?;
            if let Some(shift) = p.shift_id {
                ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM cash_shifts WHERE id=?1 AND cash_account_id=?2 AND closed_at IS NULL)",params![shift,cash],|r|r.get(0))?,"الوردية مغلقة أو لا تطابق الخزينة")?;
            }
        }
        db.execute("INSERT INTO payment_legs(document_id,direction,method,cash_account_id,shift_id,amount_minor,reference) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![doc,if outgoing{"out"}else{"in"},p.method.code(),p.cash_account_id,p.shift_id,p.amount,p.reference])?;
        let leg = db.last_insert_rowid();
        if p.method == PaymentMethod::Cheque {
            let cheque = p
                .cheque
                .as_ref()
                .ok_or_else(|| Error::validation("بيانات الشيك مطلوبة"))?;
            documents::date(&cheque.due_date)?;
            ensure(
                !cheque.number.trim().is_empty() && !cheque.bank.trim().is_empty(),
                "رقم الشيك والبنك مطلوبان",
            )?;
            db.execute("INSERT INTO cheques(payment_leg_id,party_id,cheque_number,bank_name,due_date) VALUES (?1,?2,?3,?4,?5)",params![leg,party.ok_or_else(||Error::validation("الشيك يحتاج إلى طرف"))?,cheque.number,cheque.bank,cheque.due_date])?;
        }
    }
    Ok(())
}
pub fn post_payments(
    db: &Connection,
    a: &Actor,
    doc: i64,
    party: Option<i64>,
    supplier: bool,
    journal: &mut Journal,
) -> Result<()> {
    let due: String = db.query_row(
        "SELECT document_date FROM documents WHERE id=?1",
        [doc],
        |r| r.get(0),
    )?;
    type StoredPayment = (i64, String, String, Option<i64>, i64, Option<i64>);
    let payments:Vec<StoredPayment>=db.prepare("SELECT id,direction,method,cash_account_id,amount_minor,shift_id FROM payment_legs WHERE document_id=?1 ORDER BY id")?.query_map([doc],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?.collect::<rusqlite::Result<_>>()?;
    for (leg, direction, method, cash, amount, shift) in payments {
        if let Some(shift) = shift {
            ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM cash_shifts WHERE id=?1 AND closed_at IS NULL AND cash_account_id=?2)",params![shift,cash],|r|r.get(0))?,"الوردية أغلقت؛ أعد اختيار الخزينة")?;
        }
        let signed = if direction == "out" { -amount } else { amount };
        let ledger = if method == "cheque" {
            let cheque: i64 = db.query_row(
                "SELECT id FROM cheques WHERE payment_leg_id=?1",
                [leg],
                |r| r.get(0),
            )?;
            db.execute("INSERT INTO cheque_events(cheque_id,document_id,status,created_by) VALUES (?1,?2,'pending',?3)",params![cheque,doc,a.user_id])?;
            account(
                db,
                if direction == "out" {
                    "cheque_issued"
                } else {
                    "cheque_received"
                },
            )?
        } else {
            db.query_row(
                "SELECT account_id FROM cash_accounts WHERE id=?1 AND active=1",
                [cash],
                |r| r.get(0),
            )?
        };
        journal.add(Entry::new(ledger, signed));
        if let Some(party) = party {
            journal.add(Entry::party(
                account(db, if supplier { "payable" } else { "receivable" })?,
                -signed,
                party,
                &due,
            ));
        }
    }
    Ok(())
}
pub fn credit_check(
    db: &Connection,
    a: &Actor,
    party: i64,
    additional: i64,
    reason: &str,
    ack: bool,
) -> Result<()> {
    if additional <= 0 {
        return Ok(());
    }
    let(limit,balance,policy):(Option<i64>,i64,String)=db.query_row("SELECT p.credit_limit_minor,COALESCE(b.receivable_minor,0),(SELECT credit_limit_policy FROM store_settings) FROM parties p LEFT JOIN v_party_balances b ON b.party_id=p.id WHERE p.id=?1",[party],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if limit.is_some_and(|limit| i128::from(balance) + i128::from(additional) > i128::from(limit)) {
        if policy == "block" {
            a.require("sales.credit_override")?;
        }
        if !ack {
            return Err(Error::new(
                "credit_warning",
                "تم تجاوز الحد الائتماني؛ يلزم تأكيد التحذير",
            ));
        }
        if policy == "block" {
            ensure(
                !reason.trim().is_empty(),
                "تجاوز الحد الائتماني يحتاج إلى سبب",
            )?;
        }
    }
    Ok(())
}
pub fn match_document_payments(db: &Connection, doc: i64) -> Result<()> {
    let pairs:Vec<(i64,i64,i64,i64)>=db.prepare("SELECT a.id,b.id,a.debit_minor,b.credit_minor FROM journal_lines a JOIN journal_lines b ON b.document_id=a.document_id AND b.account_id=a.account_id AND b.party_id=a.party_id JOIN accounts c ON c.id=a.account_id WHERE a.document_id=?1 AND a.debit_minor>0 AND b.credit_minor>0 AND c.control_kind IN ('receivable','payable') ORDER BY a.id,b.id")?.query_map([doc],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
    for (d, c, da, ca) in pairs {
        let used_d: i64 = db.query_row(
            "SELECT COALESCE(SUM(amount_minor),0) FROM settlements WHERE debit_line_id=?1",
            [d],
            |r| r.get(0),
        )?;
        let used_c: i64 = db.query_row(
            "SELECT COALESCE(SUM(amount_minor),0) FROM settlements WHERE credit_line_id=?1",
            [c],
            |r| r.get(0),
        )?;
        let amount = (da - used_d).min(ca - used_c);
        if amount > 0 {
            db.execute("INSERT INTO settlements(document_id,debit_line_id,credit_line_id,amount_minor) VALUES (?1,?2,?3,?4)",params![doc,d,c,amount])?;
        }
    }
    Ok(())
}
pub fn voucher(
    db: &Connection,
    a: &Actor,
    day: &str,
    party: i64,
    supplier: bool,
    outgoing: bool,
    payments: &[Payment],
    notes: &str,
) -> Result<Value> {
    ensure(!payments.is_empty(), "أدخل دفعة واحدة على الأقل")?;
    let doc = documents::header(
        db,
        a,
        if outgoing {
            "payment_voucher"
        } else {
            "receipt_voucher"
        },
        day,
        Some(party),
        None,
        notes,
    )?;
    let total = sum(payments.iter().map(|p| p.amount.0))?;
    set_total(db, doc, total, 0)?;
    save_payments(db, doc, payments, outgoing, Some(party))?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    post_payments(db, a, doc, Some(party), supplier, &mut journal)?;
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}
pub fn manual(
    db: &Connection,
    a: &Actor,
    day: &str,
    opening: bool,
    lines: &[JournalLine],
    notes: &str,
) -> Result<Value> {
    ensure(
        (2..=1000).contains(&lines.len()),
        "القيد يحتاج إلى سطرين على الأقل",
    )?;
    let doc = documents::header(
        db,
        a,
        if opening {
            "opening_balance"
        } else {
            "manual_journal"
        },
        day,
        None,
        None,
        notes,
    )?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    for l in lines {
        ensure(
            l.amount.0 != 0 && l.amount.0.unsigned_abs() <= 2_000_000_000_000_000,
            "مبلغ القيد غير صالح",
        )?;
        let mut e = Entry::new(l.account_id.0, l.amount.0);
        e.party = l.party_id.map(|x| x.0);
        e.due = l.due_date.clone();
        e.memo = l.memo.clone();
        journal.add(e);
    }
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}
pub fn expense(
    db: &Connection,
    a: &Actor,
    day: &str,
    party: Option<i64>,
    lines: &[ExpenseLine],
    payments: &[Payment],
    notes: &str,
) -> Result<Value> {
    ensure(
        !lines.is_empty() && lines.len() <= 1000,
        "سطور المصروف مطلوبة",
    )?;
    let doc = documents::header(db, a, "expense", day, party, None, notes)?;
    let mut journal = Journal::default();
    let mut net = 0;
    let mut tax = 0;
    let vat: bool = db.query_row("SELECT vat_enabled FROM store_settings", [], |r| r.get(0))?;
    for l in lines {
        ensure(
            l.amount.0 > 0 && l.tax_bps <= 10000 && (vat || l.tax_bps == 0),
            "مبلغ أو ضريبة المصروف غير صالحة",
        )?;
        let line_tax = prorate(l.amount.0, i64::from(l.tax_bps), 10000)?;
        let ledger: i64 = db.query_row(
            "SELECT account_id FROM expense_categories WHERE id=?1 AND active=1",
            [l.category_id],
            |r| r.get(0),
        )?;
        journal.add(Entry::new(ledger, l.amount.0));
        net = sum([net, l.amount.0])?;
        tax = sum([tax, line_tax])?;
        db.execute("INSERT INTO expense_lines(document_id,category_id,description,net_minor,tax_rate_bps,tax_minor) VALUES (?1,?2,?3,?4,?5,?6)",params![doc,l.category_id,l.description,l.amount,l.tax_bps,line_tax])?;
    }
    let total = sum([net, tax])?;
    let paid = sum(payments.iter().map(|p| p.amount.0))?;
    ensure(
        paid <= total && (party.is_some() || paid == total),
        "المصروف غير المدفوع يحتاج إلى مورد",
    )?;
    set_total(db, doc, net, tax)?;
    save_payments(db, doc, payments, true, party)?;
    documents::begin(db, doc)?;
    journal.add(Entry::new(account(db, "vat_input")?, tax));
    if let Some(p) = party {
        journal.add(Entry::party(account(db, "payable")?, -total, p, day));
    }
    post_payments(db, a, doc, party, true, &mut journal)?;
    journal.write(db, doc)?;
    match_document_payments(db, doc)?;
    documents::finish(db, a, doc)
}
pub fn cash_transfer(
    db: &Connection,
    a: &Actor,
    day: &str,
    from: i64,
    to: i64,
    amount: i64,
    notes: &str,
) -> Result<Value> {
    ensure(
        from != to && amount > 0,
        "حسابا التحويل أو المبلغ غير صالحين",
    )?;
    let doc = documents::header(db, a, "cash_transfer", day, None, None, notes)?;
    set_total(db, doc, amount, 0)?;
    for (id, out) in [(from, true), (to, false)] {
        let kind: String = db.query_row(
            "SELECT kind FROM cash_accounts WHERE id=?1 AND active=1",
            [id],
            |r| r.get(0),
        )?;
        save_payments(
            db,
            doc,
            &[Payment {
                method: if kind == "bank" {
                    PaymentMethod::Bank
                } else {
                    PaymentMethod::Cash
                },
                cash_account_id: Some(Int(id)),
                shift_id: None,
                amount: Int(amount),
                reference: String::new(),
                cheque: None,
            }],
            out,
            None,
        )?;
    }
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    post_payments(db, a, doc, None, false, &mut journal)?;
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}
pub fn settle(
    db: &Connection,
    a: &Actor,
    day: &str,
    debit: i64,
    credit: i64,
    amount: i64,
) -> Result<Value> {
    ensure(amount > 0, "مبلغ التسوية يجب أن يكون موجباً")?;
    ensure(db.query_row("SELECT COUNT(*) FROM journal_lines j JOIN documents d ON d.id=j.document_id WHERE j.id IN (?1,?2) AND d.status='posted' AND d.document_date<=?3",params![debit,credit,day],|r|r.get::<_,i64>(0))?==2,"تاريخ التسوية أقدم من المستندات")?;
    let doc = documents::header(db, a, "settlement", day, None, None, "")?;
    documents::begin(db, doc)?;
    db.execute("INSERT INTO settlements(document_id,debit_line_id,credit_line_id,amount_minor) VALUES (?1,?2,?3,?4)",params![doc,debit,credit,amount])?;
    documents::finish(db, a, doc)
}
pub fn unsettle(db: &Connection, a: &Actor, day: &str, id: i64, reason: &str) -> Result<Value> {
    ensure(!reason.trim().is_empty(), "سبب إلغاء التسوية مطلوب")?;
    let(debit,credit,amount,original_day):(i64,i64,i64,String)=db.query_row("SELECT s.debit_line_id,s.credit_line_id,s.amount_minor,d.document_date FROM settlements s JOIN documents d ON d.id=s.document_id WHERE s.id=?1 AND d.status='posted'",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    ensure(
        day >= original_day.as_str() && amount > 0,
        "لا يمكن إلغاء هذه التسوية بهذا التاريخ",
    )?;
    let doc = documents::header(db, a, "settlement", day, None, None, reason)?;
    documents::begin(db, doc)?;
    db.execute("INSERT INTO settlements(document_id,debit_line_id,credit_line_id,amount_minor,reversal_of_id) VALUES (?1,?2,?3,?4,?5)",params![doc,debit,credit,-amount,id])?;
    documents::finish(db, a, doc)
}
pub fn cheque(
    db: &Connection,
    a: &Actor,
    day: &str,
    id: i64,
    cleared: bool,
    bank: Option<i64>,
    reason: &str,
) -> Result<Value> {
    let(status,direction,party,amount,old_bank,due,origin):(String,String,i64,i64,Option<i64>,String,i64)=db.query_row("SELECT status,direction,party_id,amount_minor,bank_account_id,due_date,document_id FROM v_cheque_register WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
    ensure(
        status != "bounced" && (!cleared || status == "pending"),
        "حالة الشيك لا تسمح بهذه العملية",
    )?;
    ensure(!reason.trim().is_empty(), "سبب تغيير حالة الشيك مطلوب")?;
    let origin_date: String = db.query_row(
        "SELECT document_date FROM documents WHERE id=?1",
        [origin],
        |r| r.get(0),
    )?;
    ensure(day >= origin_date.as_str(), "تاريخ العملية أقدم من الشيك")?;
    let latest_date:String=db.query_row("SELECT MAX(d.document_date) FROM cheque_events e JOIN documents d ON d.id=e.document_id WHERE e.cheque_id=?1 AND d.status='posted'",[id],|r|r.get(0))?;
    ensure(
        day >= latest_date.as_str(),
        "تاريخ العملية أقدم من آخر حالة للشيك",
    )?;
    let control_accounts:Vec<i64>=db.prepare("SELECT DISTINCT j.account_id FROM journal_lines j JOIN accounts a ON a.id=j.account_id WHERE j.document_id=?1 AND j.party_id=?2 AND a.control_kind IN ('receivable','payable')")?.query_map(params![origin,party],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    ensure(
        control_accounts.len() == 1,
        "تعذر تحديد حساب الطرف الأصلي للشيك",
    )?;
    let doc = documents::header(
        db,
        a,
        if cleared {
            "cheque_clear"
        } else {
            "cheque_bounce"
        },
        day,
        Some(party),
        None,
        reason,
    )?;
    set_total(db, doc, amount, 0)?;
    documents::begin(db, doc)?;
    let incoming = direction == "in";
    let sign = if incoming { 1 } else { -1 };
    let mut journal = Journal::default();
    let bank_id = if cleared { bank } else { old_bank };
    let cash_ledger = if cleared || status == "cleared" {
        Some(db.query_row(
            "SELECT account_id FROM cash_accounts WHERE id=?1 AND kind='bank' AND active=1",
            [bank_id],
            |r| r.get::<_, i64>(0),
        )?)
    } else {
        None
    };
    let clearing = account(
        db,
        if incoming {
            "cheque_received"
        } else {
            "cheque_issued"
        },
    )?;
    if cleared {
        journal.add(Entry::new(
            cash_ledger.ok_or_else(|| Error::validation("الحساب البنكي مطلوب"))?,
            sign * amount,
        ));
        journal.add(Entry::new(clearing, -sign * amount));
    } else {
        journal.add(Entry::party(
            control_accounts[0],
            sign * amount,
            party,
            &due,
        ));
        journal.add(Entry::new(cash_ledger.unwrap_or(clearing), -sign * amount));
    }
    db.execute("INSERT INTO cheque_events(cheque_id,document_id,status,bank_account_id,reason,created_by) VALUES (?1,?2,?3,?4,?5,?6)",params![id,doc,if cleared{"cleared"}else{"bounced"},bank_id,reason,a.user_id])?;
    journal.write(db, doc)?;
    documents::finish(db, a, doc)
}
pub fn open_shift(db: &Connection, a: &Actor, cash: i64) -> Result<Value> {
    let balance:i64=db.query_row("SELECT balance_minor FROM v_cash_balances WHERE cash_account_id=?1 AND kind='cash' AND active=1",[cash],|r|r.get(0))?;
    db.execute("INSERT INTO cash_shifts(cash_account_id,opened_by,opening_balance_minor) VALUES (?1,?2,?3)",params![cash,a.user_id,balance])?;
    Ok(json!({"id":Int(db.last_insert_rowid())}))
}
pub fn close_shift(
    db: &Connection,
    a: &Actor,
    id: i64,
    day: &str,
    counted: i64,
    reason: &str,
) -> Result<Value> {
    ensure(counted >= 0, "الرصيد المعدود لا يمكن أن يكون سالباً")?;
    let(cash,ledger,expected):(i64,i64,i64)=db.query_row("SELECT s.cash_account_id,c.account_id,b.balance_minor FROM cash_shifts s JOIN cash_accounts c ON c.id=s.cash_account_id JOIN v_cash_balances b ON b.cash_account_id=c.id WHERE s.id=?1 AND s.closed_at IS NULL",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let difference = sum([counted, -expected])?;
    ensure(
        difference == 0 || !reason.trim().is_empty(),
        "فرق الخزينة يحتاج إلى سبب",
    )?;
    if difference != 0 {
        a.require("finance.write")?;
    }
    let doc = documents::header(db, a, "cash_close", day, None, None, reason)?;
    set_total(db, doc, difference.abs(), 0)?;
    documents::begin(db, doc)?;
    let mut journal = Journal::default();
    journal.add(Entry::new(ledger, difference));
    journal.add(Entry::new(account(db, "cash_variance")?, -difference));
    journal.write(db, doc)?;
    let result = documents::finish(db, a, doc)?;
    db.execute("UPDATE cash_shifts SET closed_by=?1,closed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),expected_balance_minor=?2,counted_balance_minor=?3,close_document_id=?4,notes=?5 WHERE id=?6 AND cash_account_id=?7",params![a.user_id,expected,counted,doc,reason,id,cash])?;
    Ok(result)
}
