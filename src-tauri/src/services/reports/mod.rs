//! Read-only report computations. All callers hold a consistent SQLite read transaction.
mod drilldown;
mod export;
mod financial;
mod stock;
pub use drilldown::drilldown;

use crate::{
    error::{Error, Result, ensure},
    models::{Int, Report, ReportFilter},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::OnceLock;

pub(crate) fn labels() -> &'static Value {
    static LABELS: OnceLock<Value> = OnceLock::new();
    LABELS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../src/i18n/ar.json"))
            .expect("validated Arabic dictionary")
    })
}
pub(crate) fn label(key: &str) -> String {
    labels()[key].as_str().unwrap_or(key).to_owned()
}
pub(crate) fn integer(row: &Value, key: &str) -> Result<i64> {
    row[key]
        .as_str()
        .ok_or_else(|| Error::validation("بيانات التقرير غير صالحة"))?
        .parse()
        .map_err(|_| Error::validation("قيمة التقرير خارج الحدود المسموحة"))
}
pub(crate) fn metric(key: &str, amount: i64) -> Value {
    json!({"metric":key,"amount_minor":Int(amount)})
}
pub(crate) fn output(rows: Vec<Value>, summary: Vec<Value>) -> Value {
    json!({"rows":rows,"opening":[],"summary":summary})
}
pub(crate) fn validate(filter: &ReportFilter) -> Result<()> {
    super::documents::date(&filter.from)?;
    super::documents::date(&filter.to)?;
    ensure(
        filter.party_id.is_none()
            || matches!(
                filter.report,
                Report::Collections | Report::DiscountsGranted
            ),
        "مرشح الطرف غير متاح لهذا التقرير",
    )?;
    ensure(filter.from <= filter.to, "نطاق التاريخ غير صالح")
}
pub fn run(db: &Connection, filter: &ReportFilter, limit: i64, offset: i64) -> Result<Value> {
    validate(filter)?;
    match filter.report {
        Report::BalanceSheet => financial::balance_sheet(db, filter, limit, offset),
        Report::InventoryReconciliation => financial::reconciliation(db, filter, limit, offset),
        Report::DiscountsGranted => financial::discounts(db, filter, limit, offset),
        Report::VatReturn => financial::vat(db, filter, limit, offset),
        Report::Collections => financial::collections(db, filter, limit, offset),
        Report::StockAbc | Report::StockActivity | Report::StockAging | Report::StockTurnover => {
            stock::run(db, filter, limit, offset)
        }
        _ => Err(Error::validation("التقرير غير مدعوم")),
    }
}
