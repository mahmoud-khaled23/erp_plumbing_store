use super::*;
use crate::{
    db,
    models::arithmetic::{narrow, sum},
};
use rusqlite::params;

pub(super) fn period_start(db: &Connection, to: &str) -> Result<String> {
    Ok(db.query_row("SELECT COALESCE((SELECT starts_on FROM fiscal_periods WHERE ?1 BETWEEN starts_on AND ends_on),substr(?1,1,4)||'-01-01')",[to],|r|r.get(0))?)
}

// A close/reversal is included only when actually posted by the as-of date, but
// its income/expense effect belongs to the fiscal period being closed. Reopening
// last year's result must not become this year's trading profit.
pub(super) const RESULT_JOURNAL: &str = "WITH result_journal AS (
 SELECT j.*,COALESCE(p.ends_on,j.document_date) result_date
 FROM v_journal j JOIN documents d ON d.id=j.document_id
 LEFT JOIN year_closings c ON c.document_id=COALESCE(d.reversal_of_id,d.id)
 LEFT JOIN fiscal_periods p ON p.id=c.period_id
)";

pub(super) fn balance_sheet(
    db: &Connection,
    f: &ReportFilter,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    let start = period_start(db, &f.to)?;
    let mut rows = db::rows(
        db,
        "SELECT a.id account_id,a.code account_code,a.name_ar account_name,a.kind account_kind,CASE a.kind WHEN 'asset' THEN SUM(j.debit_minor-j.credit_minor) ELSE SUM(j.credit_minor-j.debit_minor) END balance_minor,'account:'||a.id drill_key,NULL period_start_date FROM v_journal j JOIN accounts a ON a.id=j.account_id WHERE j.document_date<=?1 AND a.kind IN ('asset','liability','equity') GROUP BY a.id HAVING balance_minor<>0 ORDER BY CASE a.kind WHEN 'asset' THEN 0 WHEN 'liability' THEN 1 ELSE 2 END,a.code",
        [&f.to],
    )?;
    let (prior,current):(i64,i64)=db.query_row(&format!("{RESULT_JOURNAL} SELECT COALESCE(SUM(CASE WHEN result_date<?1 THEN credit_minor-debit_minor ELSE 0 END),0),COALESCE(SUM(CASE WHEN result_date>=?1 THEN credit_minor-debit_minor ELSE 0 END),0) FROM result_journal WHERE document_date<=?2 AND account_kind IN ('income','expense')"),params![start,f.to],|r|Ok((r.get(0)?,r.get(1)?)))?;
    rows.push(json!({"account_id":null,"account_code":"","account_name":label("priorUnclosedResult"),"account_kind":"equity","balance_minor":Int(prior),"period_start_date":null,"drill_key":"result:prior"}));
    rows.push(json!({"account_id":null,"account_code":"","account_name":label("currentPeriodResult"),"account_kind":"equity","balance_minor":Int(current),"period_start_date":start,"drill_key":"result:current"}));
    let mut totals = [0, 0, 0];
    for row in &rows {
        let i = match row["account_kind"].as_str() {
            Some("asset") => 0,
            Some("liability") => 1,
            _ => 2,
        };
        totals[i] = sum([totals[i], integer(row, "balance_minor")?])?;
    }
    let summary = vec![
        metric("totalAssets", totals[0]),
        metric("totalLiabilities", totals[1]),
        metric("totalEquity", totals[2]),
        metric(
            "reconciliationDifference",
            narrow(i128::from(totals[0]) - i128::from(totals[1]) - i128::from(totals[2]))?,
        ),
    ];
    Ok(output(
        rows.into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect(),
        summary,
    ))
}

pub(super) fn reconciliation(
    db: &Connection,
    f: &ReportFilter,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    let stock:i64=db.query_row("SELECT COALESCE(SUM(s.value_minor),0) FROM stock_movements s JOIN documents d ON d.id=s.document_id WHERE d.status='posted' AND d.document_date<=?1",[&f.to],|r|r.get(0))?;
    let ledger:i64=db.query_row("SELECT COALESCE(SUM(debit_minor-credit_minor),0) FROM v_journal WHERE control_kind='inventory' AND document_date<=?1",[&f.to],|r|r.get(0))?;
    let difference = narrow(i128::from(stock) - i128::from(ledger))?;
    let rows = if offset == 0 && limit > 0 {
        vec![
            json!({"name_ar":label("wholeStore"),"stock_value_minor":Int(stock),"ledger_value_minor":Int(ledger),"difference_minor":Int(difference),"drill_key":"reconciliation:all"}),
        ]
    } else {
        vec![]
    };
    Ok(output(
        rows,
        vec![metric("reconciliationDifference", difference)],
    ))
}

// Reversals are dated events, not deletions of the original period's activity.
pub(super) const DISCOUNTS:&str = "WITH amounts AS (
 SELECT d.id document_id,d.number document_number,d.kind document_kind,d.document_date,
 o.party_id,o.party_name_snapshot name_ar,
 (CASE WHEN d.kind='reversal' THEN -1 ELSE 1 END)*(CASE WHEN o.kind='sale_return' THEN -1 ELSE 1 END) sign,
 o.subtotal_minor gross_minor,o.line_discount_minor,o.invoice_discount_minor
 FROM documents d JOIN documents o ON o.id=COALESCE(d.reversal_of_id,d.id)
 WHERE d.status='posted' AND o.status='posted' AND o.kind IN ('sale','sale_return')
 AND d.document_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR o.party_id=?3)
 AND o.line_discount_minor+o.invoice_discount_minor>0
), report AS (SELECT document_id,document_number,document_kind,document_date,party_id,name_ar,
 sign*gross_minor gross_minor,sign*line_discount_minor line_discount_minor,
 sign*invoice_discount_minor invoice_discount_minor,
 sign*(line_discount_minor+invoice_discount_minor) discount_minor,
 sign*(gross_minor-line_discount_minor-invoice_discount_minor) net_sales_minor,
 'document:'||document_id drill_key FROM amounts)";

pub(super) fn discounts(
    db: &Connection,
    f: &ReportFilter,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    let rows = db::rows(
        db,
        &format!(
            "{DISCOUNTS} SELECT * FROM report ORDER BY document_date,document_id LIMIT ?4 OFFSET ?5"
        ),
        params![f.from, f.to, f.party_id, limit, offset],
    )?;
    let sums = db::rows(
        db,
        &format!(
            "{DISCOUNTS} SELECT COALESCE(SUM(gross_minor),0) gross_minor,COALESCE(SUM(line_discount_minor),0) line_discount_minor,COALESCE(SUM(invoice_discount_minor),0) invoice_discount_minor,COALESCE(SUM(discount_minor),0) discount_minor,COALESCE(SUM(net_sales_minor),0) net_sales_minor FROM report"
        ),
        params![f.from, f.to, f.party_id],
    )?;
    let summary = [
        ("grossSales", "gross_minor"),
        ("lineDiscount", "line_discount_minor"),
        ("invoiceDiscount", "invoice_discount_minor"),
        ("discountsGranted", "discount_minor"),
        ("netSales", "net_sales_minor"),
    ]
    .into_iter()
    .map(|(name, key)| Ok(metric(name, integer(&sums[0], key)?)))
    .collect::<Result<Vec<_>>>()?;
    Ok(output(rows, summary))
}

pub(super) const VAT_ITEMS:&str="WITH events AS (
 SELECT d.id document_id,d.number document_number,d.kind document_kind,d.document_date,
 o.id source_id,o.kind source_kind,CASE WHEN d.kind='reversal' THEN -1 ELSE 1 END reverse_sign
 FROM documents d JOIN documents o ON o.id=COALESCE(d.reversal_of_id,d.id)
 WHERE d.status='posted' AND o.status='posted' AND d.document_date BETWEEN ?1 AND ?2
), classified AS (
 SELECT e.document_id,e.document_number,e.document_kind,e.document_date,
 CASE WHEN e.source_kind IN ('sale','sale_return') THEN 'output' ELSE 'input' END vat_side,
 l.tax_rate_bps,
 e.reverse_sign*(CASE WHEN e.source_kind IN ('sale_return','purchase_return') THEN -1 ELSE 1 END)*SUM(l.subtotal_minor-l.line_discount_minor-l.invoice_discount_share_minor) taxable_minor,
 e.reverse_sign*(CASE WHEN e.source_kind IN ('sale_return','purchase_return') THEN -1 ELSE 1 END)*SUM(l.tax_minor) tax_minor
 FROM events e JOIN document_lines l ON l.document_id=e.source_id
 WHERE e.source_kind IN ('sale','sale_return','purchase','purchase_return') GROUP BY e.document_id,l.tax_rate_bps
 UNION ALL
 SELECT e.document_id,e.document_number,e.document_kind,e.document_date,'input',l.tax_rate_bps,
 e.reverse_sign*SUM(l.net_minor),e.reverse_sign*SUM(l.tax_minor)
 FROM events e JOIN expense_lines l ON l.document_id=e.source_id WHERE e.source_kind='expense' GROUP BY e.document_id,l.tax_rate_bps
), taxes AS (
 SELECT j.document_id,j.document_number,j.document_kind,j.document_date,
 CASE WHEN j.control_kind='vat_input' THEN 'input' ELSE 'output' END vat_side,
 SUM(CASE WHEN j.control_kind='vat_input' THEN j.debit_minor-j.credit_minor ELSE j.credit_minor-j.debit_minor END) tax_minor
 FROM v_journal j WHERE j.document_date BETWEEN ?1 AND ?2 AND j.control_kind IN ('vat_input','vat_output') GROUP BY j.document_id,j.control_kind
), items AS (
 SELECT * FROM classified
 UNION ALL
 SELECT t.document_id,t.document_number,t.document_kind,t.document_date,t.vat_side,NULL,NULL,
 t.tax_minor-COALESCE((SELECT SUM(c.tax_minor) FROM classified c WHERE c.document_id=t.document_id AND c.vat_side=t.vat_side),0)
 FROM taxes t WHERE t.tax_minor<>COALESCE((SELECT SUM(c.tax_minor) FROM classified c WHERE c.document_id=t.document_id AND c.vat_side=t.vat_side),0)
)";

pub(super) fn vat(db: &Connection, f: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    let rows = db::rows(
        db,
        &format!(
            "{VAT_ITEMS} SELECT vat_side,tax_rate_bps,SUM(taxable_minor) taxable_minor,SUM(tax_minor) tax_minor,COUNT(DISTINCT document_id) document_count,'vat:'||vat_side||':'||COALESCE(tax_rate_bps,'unclassified') drill_key FROM items GROUP BY vat_side,tax_rate_bps ORDER BY vat_side,tax_rate_bps LIMIT ?3 OFFSET ?4"
        ),
        params![f.from, f.to, limit, offset],
    )?;
    let (input,output_tax):(i64,i64)=db.query_row("SELECT COALESCE(SUM(CASE WHEN control_kind='vat_input' THEN debit_minor-credit_minor ELSE 0 END),0),COALESCE(SUM(CASE WHEN control_kind='vat_output' THEN credit_minor-debit_minor ELSE 0 END),0) FROM v_journal WHERE document_date BETWEEN ?1 AND ?2",params![f.from,f.to],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok(output(
        rows,
        vec![
            metric("inputVat", input),
            metric("outputVat", output_tax),
            metric(
                "netVat",
                narrow(i128::from(output_tax) - i128::from(input))?,
            ),
        ],
    ))
}

pub(super) fn collections(
    db: &Connection,
    f: &ReportFilter,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    let aging = include_str!("../../../queries/aging.sql")
        .trim()
        .trim_end_matches(';')
        .replace(":as_of", "?1");
    let rows=db::rows(db,&format!("SELECT a.*,p.phone,
 (SELECT MAX(d.document_date) FROM documents d JOIN payment_legs l ON l.document_id=d.id
  WHERE d.party_id=a.party_id AND d.status='posted' AND d.document_date<=?1 AND l.direction='in'
  AND EXISTS(SELECT 1 FROM v_journal j WHERE j.document_id=d.id AND j.party_id=a.party_id AND j.control_kind='receivable' AND j.credit_minor>0)
  AND NOT EXISTS(SELECT 1 FROM documents r WHERE r.reversal_of_id=d.id AND r.status='posted' AND r.document_date<=?1)) last_payment_date,
 a.days_0_30_minor+a.days_31_60_minor+a.days_61_90_minor+a.over_90_minor overdue_minor,
 'party:'||a.party_id drill_key FROM ({aging}) a JOIN parties p ON p.id=a.party_id
 WHERE a.control_kind='receivable' AND (?2 IS NULL OR a.party_id=?2)
 AND a.days_0_30_minor+a.days_31_60_minor+a.days_61_90_minor+a.over_90_minor>0
 ORDER BY a.over_90_minor DESC,a.days_61_90_minor DESC,a.days_31_60_minor DESC,overdue_minor DESC,a.party_id LIMIT ?3 OFFSET ?4"),params![f.to,f.party_id,limit,offset])?;
    Ok(output(rows, vec![]))
}
