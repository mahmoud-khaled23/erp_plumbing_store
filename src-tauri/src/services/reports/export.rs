use super::*;
use crate::state::App;
use std::{fs::File, io::Write, path::Path};

pub(super) fn columns(report: Report) -> Result<&'static [&'static str]> {
    Ok(match report {
        Report::BalanceSheet => &[
            "account_code",
            "account_name",
            "account_kind",
            "period_start_date",
            "balance_minor",
        ],
        Report::InventoryReconciliation => &[
            "name_ar",
            "stock_value_minor",
            "ledger_value_minor",
            "difference_minor",
        ],
        Report::StockAbc => &[
            "code",
            "name_ar",
            "net_sales_minor",
            "share_bps",
            "cumulative_bps",
            "stock_class",
        ],
        Report::StockActivity => &[
            "code",
            "name_ar",
            "warehouse_name",
            "quantity_micros",
            "value_minor",
            "last_sale_date",
            "days_since_sale",
            "stock_activity",
        ],
        Report::StockAging => &[
            "code",
            "name_ar",
            "warehouse_name",
            "quantity_micros",
            "age_0_30_micros",
            "age_31_60_micros",
            "age_61_90_micros",
            "age_over_90_micros",
            "negative_quantity_micros",
            "value_minor",
        ],
        Report::StockTurnover => &[
            "code",
            "name_ar",
            "opening_value_minor",
            "closing_value_minor",
            "average_value_minor",
            "cogs_minor",
            "turnover_milli",
            "inventory_days_milli",
        ],
        Report::DiscountsGranted => &[
            "document_id",
            "document_date",
            "document_number",
            "document_kind",
            "name_ar",
            "gross_minor",
            "line_discount_minor",
            "invoice_discount_minor",
            "discount_minor",
            "net_sales_minor",
        ],
        Report::VatReturn => &[
            "vat_side",
            "tax_rate_bps",
            "taxable_minor",
            "tax_minor",
            "document_count",
        ],
        Report::Collections => &[
            "code",
            "name_ar",
            "phone",
            "days_0_30_minor",
            "days_31_60_minor",
            "days_61_90_minor",
            "over_90_minor",
            "overdue_minor",
            "unapplied_credit_minor",
            "last_payment_date",
        ],
        _ => return Err(Error::validation("تصدير هذا التقرير غير متاح")),
    })
}
fn decimal(value: i64, places: u32) -> String {
    let magnitude = value.unsigned_abs();
    let scale = 10u64.pow(places);
    let sign = if value < 0 { "-" } else { "" };
    if places == 0 {
        format!("{sign}{magnitude}")
    } else {
        format!(
            "{sign}{}.{:0width$}",
            magnitude / scale,
            magnitude % scale,
            width = places as usize
        )
    }
}
fn date(day: &str) -> String {
    format!("{}/{}/{}", &day[8..10], &day[5..7], &day[..4])
}
fn cell(row: &Value, key: &str, digits: u32) -> Result<String> {
    let Some(value) = row[key].as_str() else {
        return Ok(if key == "tax_rate_bps" {
            label("unclassifiedVat")
        } else {
            String::new()
        });
    };
    let scale = if key.ends_with("_minor") {
        Some(digits)
    } else if key.ends_with("_micros") {
        Some(6)
    } else if key.ends_with("_milli") {
        Some(3)
    } else if key.ends_with("_bps") {
        Some(2)
    } else {
        None
    };
    if let Some(scale) = scale {
        return Ok(decimal(integer(row, key)?, scale));
    }
    if key.ends_with("_date") && value.len() == 10 {
        return Ok(date(value));
    }
    if [
        "account_kind",
        "document_kind",
        "stock_class",
        "stock_activity",
        "vat_side",
    ]
    .contains(&key)
    {
        return Ok(labels()["enums"][value].as_str().unwrap_or(value).into());
    }
    Ok(value.into())
}
fn safe_csv(value: &str) -> String {
    if value
        .trim_start()
        .starts_with(['=', '+', '-', '@', '\t', '\r', '\n'])
    {
        format!("'{value}")
    } else {
        value.into()
    }
}
fn file_error<E>(_: E) -> Error {
    Error::new("export", "تعذر إنشاء ملف التقرير")
}

impl App {
    /// The desktop adapter supplies only a native-dialog-selected path. No IPC path input.
    pub fn export_report(&self, path: &Path, filter: &ReportFilter, excel: bool) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("reports.read")?;
        actor.require("reports.export")?;
        validate(filter)?;
        let columns = columns(filter.report)?;
        ensure(!path.exists(), "الملف موجود؛ اختر اسماً جديداً")?;
        let tx = inner.db.transaction()?;
        let data = run(&tx, filter, 10001, 0)?;
        let rows = data["rows"]
            .as_array()
            .ok_or_else(|| Error::validation("نتيجة التقرير غير صالحة"))?;
        ensure(
            rows.len() <= 10000,
            "يتجاوز التقرير عشرة آلاف صف؛ ضيق النطاق قبل التصدير",
        )?;
        let (name,currency,symbol,digits):(String,String,String,u32)=tx.query_row("SELECT store_name,currency_code,currency_symbol,currency_minor_digits FROM store_settings",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let report = serde_json::to_value(filter.report)?;
        let report = report
            .as_str()
            .ok_or_else(|| Error::validation("نوع التقرير غير صالح"))?;
        let mut records = vec![
            vec![label("storeName"), name],
            vec![
                label("report"),
                labels()["reportNames"][report]
                    .as_str()
                    .unwrap_or(report)
                    .into(),
            ],
            vec![label("from"), date(&filter.from)],
            vec![label("to"), date(&filter.to)],
            vec![label("currency"), format!("{currency} ({symbol})")],
            vec![
                label("reportScope"),
                labels()["reportHints"][report]
                    .as_str()
                    .unwrap_or("")
                    .into(),
            ],
        ];
        if let Some(party) = filter.party_id {
            let name: String =
                tx.query_row("SELECT name_ar FROM parties WHERE id=?1", [party], |r| {
                    r.get(0)
                })?;
            records.push(vec![label("party"), name]);
        }
        records.push(vec![]);
        let header_row = records.len();
        let mut warning_rows = std::collections::BTreeSet::new();
        records.push(
            columns
                .iter()
                .map(|k| {
                    labels()["columns"][k]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| label(k))
                })
                .collect(),
        );
        for row in rows {
            if row["difference_minor"].as_str().is_some_and(|v| v != "0") {
                warning_rows.insert(records.len());
            }
            records.push(
                columns
                    .iter()
                    .map(|key| cell(row, key, digits))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        if let Some(summary) = data["summary"].as_array().filter(|s| !s.is_empty()) {
            records.push(vec![]);
            records.push(vec![label("reportTotals")]);
            for row in summary {
                if row["metric"] == "reconciliationDifference" && integer(row, "amount_minor")? != 0
                {
                    warning_rows.insert(records.len());
                }
                records.push(vec![
                    label(row["metric"].as_str().unwrap_or("")),
                    decimal(integer(row, "amount_minor")?, digits),
                ]);
            }
        }
        let bytes = if excel {
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let sheet = workbook.add_worksheet();
            sheet.set_right_to_left(true);
            sheet.set_name(label("report")).map_err(file_error)?;
            let normal = rust_xlsxwriter::Format::new().set_font_name("Arial");
            let header = normal
                .clone()
                .set_bold()
                .set_background_color("#E2E8F0")
                .set_text_wrap();
            let warning = normal
                .clone()
                .set_bold()
                .set_font_color("#991B1B")
                .set_background_color("#FEF2F2");
            for (r, record) in records.iter().enumerate() {
                let format = if warning_rows.contains(&r) {
                    &warning
                } else if r == header_row {
                    &header
                } else {
                    &normal
                };
                for (c, value) in record.iter().enumerate() {
                    sheet
                        .write_string_with_format(r as u32, c as u16, value, format)
                        .map_err(file_error)?;
                }
            }
            for (c, key) in columns.iter().enumerate() {
                sheet
                    .set_column_width(c as u16, if key.contains("name") { 35 } else { 22 })
                    .map_err(file_error)?;
            }
            sheet
                .set_freeze_panes(header_row as u32 + 1, 0)
                .map_err(file_error)?;
            if !rows.is_empty() {
                sheet
                    .autofilter(
                        header_row as u32,
                        0,
                        (header_row + rows.len()) as u32,
                        (columns.len() - 1) as u16,
                    )
                    .map_err(file_error)?;
            }
            workbook.save_to_buffer().map_err(file_error)?
        } else {
            let mut writer = csv::WriterBuilder::new()
                .flexible(true)
                .from_writer(vec![0xef, 0xbb, 0xbf]);
            for record in &records {
                writer
                    .write_record(record.iter().map(|v| safe_csv(v)))
                    .map_err(file_error)?;
            }
            writer.into_inner().map_err(file_error)?
        };
        // Build fully before creating the destination; never overwrite a previous export.
        let mut file = File::create_new(path)?;
        let saved = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            tx.execute("INSERT INTO audit_log(user_id,action,entity_type,after_json) VALUES (?1,'export_report','report',?2)",rusqlite::params![actor.user_id,serde_json::to_string(filter)?])?;
            tx.commit()?;
            Ok(())
        })();
        drop(file);
        if let Err(error) = saved {
            let _ = std::fs::remove_file(path);
            return Err(error);
        }
        Ok(json!({"rows":Int(rows.len() as i64),"path":path.to_string_lossy()}))
    }
}
