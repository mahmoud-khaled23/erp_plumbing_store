use super::catalog;
use crate::{
    db,
    error::{Error, Result, ensure},
    models::*,
    state::App,
};
use calamine::{Reader, open_workbook_auto};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const HEADERS: &[&str] = &[
    "code",
    "name_ar",
    "category_id",
    "brand",
    "material",
    "size",
    "base_unit_id",
    "standard_cost_minor",
    "min_stock_micros",
    "active",
    "unit_id",
    "factor_numerator",
    "factor_denominator",
    "barcode",
    "retail_price_minor",
    "wholesale_price_minor",
    "unit_active",
];
const EXPORT_SQL: &str = "SELECT p.code,p.name_ar,p.category_id,p.brand,p.material,p.size,p.base_unit_id,p.standard_cost_minor,p.min_stock_micros,p.active,u.unit_id,u.factor_numerator,u.factor_denominator,COALESCE(u.barcode,''),u.retail_price_minor,u.wholesale_price_minor,u.active FROM products p JOIN product_units u ON u.product_id=p.id ORDER BY p.id,u.id";
const MAX_IMPORT: u64 = 20 * 1024 * 1024;

fn file_error<E>(_: E) -> Error {
    Error::new("file_format", "تعذر معالجة الملف؛ تحقق من الصيغة والمحتوى")
}
fn file_digest(path: &Path) -> Result<String> {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn export_cell(cell: &str) -> String {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{cell}")
    } else {
        cell.into()
    }
}
fn records(path: &Path) -> Result<Vec<Vec<String>>> {
    ensure(
        fs::metadata(path)?.len() <= MAX_IMPORT,
        "حجم ملف الاستيراد يتجاوز 20 ميجابايت",
    )?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut output = Vec::new();
    if extension == "csv" {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(path)
            .map_err(file_error)?;
        for row in reader.records() {
            ensure(
                output.len() <= 100_000,
                "الملف يحتوي على أكثر من 100000 سطر",
            )?;
            output.push(
                row.map_err(file_error)?
                    .iter()
                    .map(|v| v.trim_start_matches('\u{feff}').to_owned())
                    .collect(),
            );
        }
    } else {
        ensure(
            extension == "xlsx" || extension == "xls",
            "صيغة الملف غير مدعومة",
        )?;
        if extension == "xlsx" {
            let archive = ZipArchive::new(File::open(path)?).map_err(file_error)?;
            ensure(
                archive.decompressed_size().unwrap_or(u128::MAX) <= 200 * 1024 * 1024,
                "حجم بيانات Excel كبير جداً",
            )?;
        }
        let mut book = open_workbook_auto(path).map_err(file_error)?;
        let sheet = book
            .worksheet_range_at(0)
            .ok_or_else(|| Error::validation("الملف لا يحتوي على ورقة عمل"))?
            .map_err(file_error)?;
        ensure(
            sheet.height() <= 100_001 && sheet.width() <= 100,
            "حجم ورقة العمل كبير جداً",
        )?;
        for row in sheet.rows() {
            output.push(row.iter().map(ToString::to_string).collect());
        }
    }
    Ok(output)
}
fn parse_products(path: &Path) -> Result<Vec<Product>> {
    let rows = records(path)?;
    ensure(!rows.is_empty(), "الملف فارغ")?;
    ensure(
        rows[0]
            .iter()
            .map(String::as_str)
            .eq(HEADERS.iter().copied()),
        "عناوين الملف لا تطابق قالب التصدير",
    )?;
    let mut products: BTreeMap<String, Product> = BTreeMap::new();
    for (index, row) in rows.iter().enumerate().skip(1) {
        ensure(
            row.len() == HEADERS.len(),
            &format!("عدد الأعمدة غير صحيح في السطر {}", index + 1),
        )?;
        let n = |i: usize| -> Result<Int> {
            row[i].parse::<i64>().map(Int).map_err(|_| {
                Error::validation(&format!(
                    "رقم غير صحيح في السطر {}: {}",
                    index + 1,
                    HEADERS[i]
                ))
            })
        };
        let flag = |i: usize| -> Result<bool> {
            ensure(
                row[i] == "0" || row[i] == "1",
                "حقول الحالة تقبل 0 أو 1 فقط",
            )?;
            Ok(row[i] == "1")
        };
        let text = |i: usize| -> String {
            let s = &row[i];
            if s.starts_with("'=")
                || s.starts_with("'+")
                || s.starts_with("'-")
                || s.starts_with("'@")
            {
                s[1..].into()
            } else {
                s.clone()
            }
        };
        let product = Product {
            id: None,
            revision: Int(0),
            code: text(0),
            name: text(1),
            category_id: n(2)?,
            brand: text(3),
            material: text(4),
            size: text(5),
            base_unit_id: n(6)?,
            standard_cost: n(7)?,
            min_stock: n(8)?,
            active: flag(9)?,
            units: Vec::new(),
        };
        let p = products
            .entry(product.code.clone())
            .or_insert_with(|| product.clone());
        ensure(
            p.name == product.name
                && p.category_id == product.category_id
                && p.base_unit_id == product.base_unit_id
                && p.standard_cost == product.standard_cost,
            "بيانات الصنف تختلف بين سطور الوحدات",
        )?;
        p.units.push(ProductUnit {
            id: None,
            unit_id: n(10)?,
            numerator: n(11)?,
            denominator: n(12)?,
            barcode: if row[13].is_empty() {
                None
            } else {
                Some(text(13))
            },
            retail_price: n(14)?,
            wholesale_price: n(15)?,
            active: flag(16)?,
        });
    }
    ensure(!products.is_empty(), "الملف لا يحتوي على أصناف")?;
    Ok(products.into_values().collect())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    schema: usize,
    files: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct RestoreIntent {
    previous: PathBuf,
    staging: PathBuf,
}
fn intent_path(root: &Path) -> Result<PathBuf> {
    Ok(root
        .parent()
        .ok_or_else(|| Error::validation("مسار المتجر غير صالح"))?
        .join(format!(
            ".restore-{}.json",
            root.file_name()
                .ok_or_else(|| Error::validation("اسم المتجر غير صالح"))?
                .to_string_lossy()
        )))
}
fn begin_replace(root: &Path, previous: &Path, staging: &Path) -> Result<()> {
    let mut marker = File::create_new(intent_path(root)?)?;
    marker.write_all(&serde_json::to_vec(&RestoreIntent {
        previous: previous.into(),
        staging: staging.into(),
    })?)?;
    marker.sync_all()?;
    Ok(())
}
fn finish_replace(root: &Path) -> Result<()> {
    fs::remove_file(intent_path(root)?)?;
    Ok(())
}
pub(crate) fn recover_interrupted_replace(root: &Path) -> Result<()> {
    let marker = intent_path(root)?;
    if !marker.exists() {
        return Ok(());
    }
    ensure(
        fs::metadata(&marker)?.len() < 16_384,
        "ملف حالة الاستعادة غير صالح",
    )?;
    let intent: RestoreIntent = serde_json::from_reader(File::open(&marker)?)?;
    let parent = root
        .parent()
        .ok_or_else(|| Error::validation("مسار المتجر غير صالح"))?
        .canonicalize()?;
    for path in [&intent.previous, &intent.staging] {
        ensure(
            path.parent()
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|p| p == parent)
                && path.file_name().is_some_and(|n| {
                    let name = n.to_string_lossy();
                    ["previous-", "damaged-", "restore-", "recovery-"]
                        .iter()
                        .any(|prefix| {
                            name.strip_prefix(prefix)
                                .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok())
                        })
                }),
            "مسارات الاستعادة غير صالحة",
        )?;
    }
    if intent.previous.exists() {
        if root.exists() {
            ensure(
                !intent.staging.exists(),
                "توجد استعادة معلقة تحتاج إلى مراجعة",
            )?;
            fs::rename(root, &intent.staging)?;
        }
        fs::rename(&intent.previous, root)?;
    }
    ensure(root.exists(), "مجلد المتجر مفقود بعد انقطاع الاستعادة")?;
    finish_replace(root)
}
pub(crate) fn archive_backup(db: &Connection, root: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| Error::validation("مجلد النسخ الاحتياطي غير صالح"))?
        .canonicalize()?;
    ensure(
        !parent.starts_with(root.canonicalize()?),
        "احفظ النسخة الاحتياطية خارج مجلد البيانات الحية",
    )?;
    let temp = root.join(format!("backup-{}.db", uuid::Uuid::new_v4()));
    let mut created = false;
    let result = (|| {
        let mut snapshot = Connection::open(&temp)?;
        rusqlite::backup::Backup::new(db, &mut snapshot)?.run_to_completion(
            128,
            Duration::from_millis(5),
            None,
        )?;
        db::integrity(&snapshot, true)?;
        snapshot.close().map_err(|(c, e)| {
            drop(c);
            Error::from(e)
        })?;
        let file = File::create_new(destination)?;
        created = true;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut manifest = Manifest {
            format: 1,
            schema: db.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| {
                r.get::<_, u32>(0)
            })? as usize,
            files: BTreeMap::new(),
        };
        let mut files = vec![("store.db".to_owned(), temp.clone())];
        for entry in fs::read_dir(root.join("assets"))? {
            let entry = entry?;
            ensure(
                entry.file_type()?.is_file(),
                "مجلد الصور يحتوي على ملف غير صالح",
            )?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or_else(|| Error::validation("اسم ملف غير صالح"))?
                .to_owned();
            files.push((format!("assets/{name}"), entry.path()));
        }
        for (name, path) in files {
            let mut file = File::open(path)?;
            let mut hash = sha2::Sha256::new();
            use sha2::Digest;
            zip.start_file(&name, options).map_err(file_error)?;
            let mut buffer = [0u8; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
                zip.write_all(&buffer[..n])?;
            }
            manifest
                .files
                .insert(name, format!("{:x}", hash.finalize()));
        }
        zip.start_file("manifest.json", options)
            .map_err(file_error)?;
        zip.write_all(&serde_json::to_vec(&manifest)?)?;
        zip.finish().map_err(file_error)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    if result.is_err() && created {
        let _ = fs::remove_file(destination);
    }
    result
}
fn extract_backup(archive: &Path, staging: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(File::open(archive)?).map_err(file_error)?;
    ensure(
        zip.len() <= 10002
            && zip.decompressed_size().unwrap_or(u128::MAX) <= 2 * 1024 * 1024 * 1024,
        "حجم النسخة الاحتياطية غير مسموح",
    )?;
    let manifest: Manifest = {
        let file = zip.by_name("manifest.json").map_err(file_error)?;
        ensure(file.size() <= 2 * 1024 * 1024, "ملف وصف النسخة كبير جداً")?;
        serde_json::from_reader(file)?
    };
    ensure(
        manifest.format == 1
            && manifest.schema <= db::MIGRATIONS.len()
            && manifest.files.contains_key("store.db"),
        "إصدار النسخة الاحتياطية غير مدعوم",
    )?;
    ensure(
        zip.len() == manifest.files.len() + 1,
        "محتويات النسخة لا تطابق الوصف",
    )?;
    fs::create_dir(staging)?;
    fs::create_dir(staging.join("assets"))?;
    for (name, hash) in manifest.files {
        let parts: Vec<_> = name.split('/').collect();
        ensure(
            name == "store.db"
                || (parts.len() == 2
                    && parts[0] == "assets"
                    && !parts[1].is_empty()
                    && parts[1] != "."
                    && parts[1] != ".."),
            "مسار غير آمن داخل النسخة",
        )?;
        ensure(
            !name.contains('\\') && !name.contains(':') && !name.contains('\0'),
            "اسم ملف غير صالح داخل النسخة",
        )?;
        let mut file = zip.by_name(&name).map_err(file_error)?;
        ensure(
            file.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
            "الروابط الرمزية غير مسموحة",
        )?;
        let mut output = File::create_new(staging.join(&name))?;
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n])?;
            hasher.update(&buffer[..n]);
        }
        output.sync_all()?;
        ensure(
            format!("{:x}", hasher.finalize()) == hash,
            "بصمة ملف النسخة لا تطابق المحتوى",
        )?;
    }
    let connection = db::open(&staging.join("store.db"))?;
    db::integrity(&connection, true)?;
    Ok(())
}
impl App {
    pub fn preview_opening(&self, path: &Path, balances: bool) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require(if balances {
            "finance.write"
        } else {
            "inventory.adjust"
        })?;
        let rows = records(path)?;
        let headers: &[&str] = if balances {
            &["account_code", "party_code", "amount", "due_date"]
        } else {
            &["product_code", "unit_id", "quantity", "unit_cost"]
        };
        ensure(
            rows.len() > 1
                && rows[0]
                    .iter()
                    .map(String::as_str)
                    .eq(headers.iter().copied()),
            "عناوين ملف الأرصدة الافتتاحية غير صحيحة",
        )?;
        let token = uuid::Uuid::new_v4().to_string();
        inner.import_grants.clear();
        inner
            .import_grants
            .insert(token.clone(), (path.to_path_buf(), file_digest(path)?));
        Ok(
            json!({"token":token,"count":Int((rows.len()-1)as i64),"headers":headers,"preview":rows.iter().skip(1).take(100).collect::<Vec<_>>()}),
        )
    }
    pub fn import_opening(
        &self,
        token: &str,
        balances: bool,
        day: &str,
        warehouse: Option<Int>,
    ) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require(if balances {
            "finance.write"
        } else {
            "inventory.adjust"
        })?;
        let (path, checksum) = inner
            .import_grants
            .get(token)
            .cloned()
            .ok_or_else(|| Error::validation("أعد معاينة ملف الاستيراد"))?;
        ensure(
            fs::metadata(&path)?.len() <= MAX_IMPORT && file_digest(&path)? == checksum,
            "الملف تغير؛ أعد المعاينة",
        )?;
        let rows = records(&path)?;
        let headers: &[&str] = if balances {
            &["account_code", "party_code", "amount", "due_date"]
        } else {
            &["product_code", "unit_id", "quantity", "unit_cost"]
        };
        ensure(
            rows.len() > 1
                && rows[0]
                    .iter()
                    .map(String::as_str)
                    .eq(headers.iter().copied()),
            "عناوين ملف الأرصدة الافتتاحية غير صحيحة",
        )?;
        let kind = if balances {
            "opening_balances"
        } else {
            "opening_stock"
        };
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_batches WHERE kind=?1 AND sha256=?2)",
            params![kind, checksum],
            |r| r.get(0),
        )?;
        ensure(!prior, "تم استيراد هذا الملف سابقاً")?;
        let digits: u32 = tx.query_row(
            "SELECT currency_minor_digits FROM store_settings",
            [],
            |r| r.get(0),
        )?;
        let mut documents = Vec::new();
        for chunk in rows[1..].chunks(500) {
            if balances {
                let mut lines = Vec::new();
                let mut amounts = Vec::new();
                for row in chunk {
                    ensure(row.len() == 4, "عدد أعمدة الرصيد الافتتاحي غير صحيح")?;
                    let account = tx.query_row(
                        "SELECT id FROM accounts WHERE code=?1 AND active=1 AND is_postable=1",
                        [&row[0]],
                        |r| r.get::<_, Int>(0),
                    )?;
                    let party = if row[1].trim().is_empty() {
                        None
                    } else {
                        Some(tx.query_row(
                            "SELECT id FROM parties WHERE code=?1 AND active=1",
                            [&row[1]],
                            |r| r.get::<_, Int>(0),
                        )?)
                    };
                    let amount = crate::models::arithmetic::parse_decimal(&row[2], digits)?;
                    ensure(amount != 0, "الرصيد الافتتاحي الصفري غير مطلوب")?;
                    amounts.push(amount);
                    lines.push(JournalLine {
                        account_id: account,
                        party_id: party,
                        due_date: Some(if row[3].is_empty() {
                            day.into()
                        } else {
                            row[3].clone()
                        }),
                        amount: Int(amount),
                        memo: "استيراد رصيد افتتاحي".into(),
                    });
                }
                let total = crate::models::arithmetic::sum(amounts)?;
                if total != 0 {
                    lines.push(JournalLine {
                        account_id: Int(super::finance::account(&tx, "opening_equity")?),
                        party_id: None,
                        due_date: None,
                        amount: Int(-total),
                        memo: String::new(),
                    });
                }
                let posted = super::finance::manual(
                    &tx,
                    &actor,
                    day,
                    true,
                    &lines,
                    "استيراد الأرصدة الافتتاحية",
                )?;
                documents.push(posted["id"].clone());
            } else {
                let warehouse = warehouse.ok_or_else(|| Error::validation("المخزن مطلوب"))?;
                let mut lines = Vec::new();
                for row in chunk {
                    ensure(row.len() == 4, "عدد أعمدة المخزون الافتتاحي غير صحيح")?;
                    let unit: i64 = row[1].parse().map_err(file_error)?;
                    let unit=tx.query_row("SELECT u.id FROM product_units u JOIN products p ON p.id=u.product_id WHERE p.code=?1 AND u.unit_id=?2 AND u.active=1 AND p.active=1",params![row[0],unit],|r|r.get::<_,Int>(0))?;
                    lines.push(DraftLine {
                        unit_id: unit,
                        quantity: Int(crate::models::arithmetic::parse_decimal(&row[2], 6)?),
                        price: Int(crate::models::arithmetic::parse_decimal(&row[3], digits)?),
                        discount: Int(0),
                        tax_bps: 0,
                        order_line_id: None,
                        damaged: Int(0),
                        short: Int(0),
                        matches: Vec::new(),
                        notes: String::new(),
                    });
                }
                let draft = Draft {
                    id: None,
                    revision: Int(0),
                    kind: DocumentKind::OpeningStock,
                    date: day.into(),
                    due_date: None,
                    party_id: None,
                    warehouse_id: Some(warehouse),
                    destination_id: None,
                    source_id: None,
                    wholesale: false,
                    discount: Int(0),
                    lines,
                    payments: Vec::new(),
                    notes: "استيراد المخزون الافتتاحي".into(),
                    reason: String::new(),
                };
                let saved = super::documents::save(&tx, &actor, &draft)?;
                let id: i64 = saved["id"]
                    .as_str()
                    .ok_or_else(|| Error::validation("نتيجة الحفظ غير صالحة"))?
                    .parse()
                    .map_err(file_error)?;
                super::documents::post(&tx, &actor, id, "", false)?;
                documents.push(json!(Int(id)));
            }
        }
        tx.execute("INSERT INTO import_batches(kind,source_name,sha256,row_count,created_by) VALUES (?1,?2,?3,?4,?5)",params![kind,path.file_name().unwrap_or_default().to_string_lossy(),checksum,(rows.len()-1)as i64,actor.user_id])?;
        db::audit(
            &tx,
            actor.user_id,
            "import_opening",
            "import",
            tx.last_insert_rowid(),
            kind,
        )?;
        tx.commit()?;
        inner.import_grants.remove(token);
        Ok(json!({"documents":documents,"imported":Int((rows.len()-1)as i64)}))
    }
    pub fn export_products(&self, path: &Path, excel: bool) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("products.read")?;
        ensure(!path.exists(), "الملف موجود؛ اختر اسماً جديداً")?;
        let mut statement = inner.db.prepare(EXPORT_SQL)?;
        let mut rows = statement.query([])?;
        let mut count = 0u32;
        if excel {
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let sheet = workbook.add_worksheet();
            sheet.set_right_to_left(true);
            for (i, h) in HEADERS.iter().enumerate() {
                sheet.write_string(0, i as u16, *h).map_err(file_error)?;
            }
            while let Some(row) = rows.next()? {
                count += 1;
                ensure(count <= 100_000, "التصدير يتجاوز 100000 سطر")?;
                for i in 0..HEADERS.len() {
                    let text = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                        rusqlite::types::ValueRef::Text(v) => String::from_utf8_lossy(v).into(),
                        _ => String::new(),
                    };
                    sheet
                        .write_string(count, i as u16, &text)
                        .map_err(file_error)?;
                }
            }
            let mut file = File::create_new(path)?;
            workbook.save_to_writer(&mut file).map_err(file_error)?;
            file.sync_all()?;
        } else {
            let mut file = File::create_new(path)?;
            file.write_all(&[0xef, 0xbb, 0xbf])?;
            let mut writer = csv::Writer::from_writer(file);
            writer.write_record(HEADERS).map_err(file_error)?;
            while let Some(row) = rows.next()? {
                let mut cells = Vec::new();
                for i in 0..HEADERS.len() {
                    cells.push(match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                        rusqlite::types::ValueRef::Text(v) => {
                            export_cell(&String::from_utf8_lossy(v))
                        }
                        _ => String::new(),
                    });
                }
                writer.write_record(cells).map_err(file_error)?;
                count += 1;
            }
            writer.flush()?;
        }
        db::audit(&inner.db, actor.user_id, "export_products", "file", 0, "")?;
        Ok(json!({"rows":Int(i64::from(count))}))
    }
    pub fn preview_products(&self, path: &Path) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("products.import")?;
        let products = parse_products(path)?;
        let token = uuid::Uuid::new_v4().to_string();
        let checksum = db::digest(&fs::read(path)?);
        inner.import_grants.clear();
        inner
            .import_grants
            .insert(token.clone(), (path.to_path_buf(), checksum.clone()));
        Ok(
            json!({"token":token,"sha256":checksum,"count":Int(products.len()as i64),"preview":products.iter().take(100).collect::<Vec<_>>()}),
        )
    }
    pub fn import_products(&self, token: &str, replace: bool) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("products.import")?;
        actor.require("products.write")?;
        let (path, hash) = inner
            .import_grants
            .get(token)
            .cloned()
            .ok_or_else(|| Error::validation("أعد معاينة ملف الاستيراد"))?;
        ensure(
            fs::metadata(&path)?.len() <= MAX_IMPORT && db::digest(&fs::read(&path)?) == hash,
            "ملف الاستيراد تغير؛ أعد المعاينة",
        )?;
        let mut products = parse_products(&path)?;
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for p in &mut products {
            use rusqlite::OptionalExtension;
            let existing: Option<(Int, Int)> = tx
                .query_row(
                    "SELECT id,revision FROM products WHERE code=?1 COLLATE NOCASE",
                    [&p.code],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((id, revision)) = existing {
                ensure(replace, "يوجد كود صنف مسجل؛ اختر تحديث الأصناف الموجودة")?;
                p.id = Some(id);
                p.revision = revision;
                for unit in &mut p.units {
                    unit.id = tx
                        .query_row(
                            "SELECT id FROM product_units WHERE product_id=?1 AND unit_id=?2",
                            params![id, unit.unit_id],
                            |r| r.get(0),
                        )
                        .optional()?;
                }
            }
            catalog::product(&tx, p)?;
        }
        tx.execute("INSERT INTO import_batches(kind,source_name,sha256,row_count,created_by) VALUES ('products',?1,?2,?3,?4)",params![path.file_name().unwrap_or_default().to_string_lossy(),hash,products.len()as i64,actor.user_id])?;
        db::audit(
            &tx,
            actor.user_id,
            "import_products",
            "import",
            tx.last_insert_rowid(),
            "",
        )?;
        tx.commit()?;
        inner.import_grants.remove(token);
        Ok(json!({"imported":Int(products.len()as i64)}))
    }
    pub fn backup(&self, destination: &Path) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("backup.create")?;
        archive_backup(&inner.db, &self.root, destination)?;
        let size = fs::metadata(destination)?.len();
        inner.db.execute("INSERT INTO backup_runs(kind,path,sha256,size_bytes,status,completed_at,user_id) VALUES ('manual',?1,?2,?3,'verified',strftime('%Y-%m-%dT%H:%M:%fZ','now'),?4)",params![destination.to_string_lossy(),file_digest(destination)?,i64::try_from(size).map_err(file_error)?,actor.user_id])?;
        db::audit(
            &inner.db,
            actor.user_id,
            "backup",
            "backup",
            inner.db.last_insert_rowid(),
            "",
        )?;
        Ok(json!({"saved":true}))
    }
    pub fn daily_backup(&self) -> Result<()> {
        let inner = self.lock()?;
        if inner.recovery_error.is_some() {
            return Ok(());
        }
        let (enabled, zone, folder): (bool, String, Option<String>) = inner.db.query_row(
            "SELECT auto_backup,timezone,backup_directory FROM store_settings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if !enabled {
            return Ok(());
        }
        let zone: chrono_tz::Tz = zone.parse().map_err(file_error)?;
        let day = chrono::Utc::now()
            .with_timezone(&zone)
            .format("%Y-%m-%d")
            .to_string();
        let folder = folder
            .map(PathBuf::from)
            .unwrap_or_else(|| self.root.parent().unwrap_or(&self.root).join("backups"));
        fs::create_dir_all(&folder)?;
        let verified:Vec<String>=inner.db.prepare("SELECT path FROM backup_runs WHERE kind='automatic' AND status='verified' ORDER BY id DESC")?.query_map([],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        if verified.iter().map(PathBuf::from).any(|p| {
            p.parent() == Some(folder.as_path())
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&format!("daily-{day}")))
                && p.is_file()
        }) {
            return Ok(());
        }
        let mut target = folder.join(format!("daily-{day}.erpbackup"));
        if target.exists() {
            target = folder.join(format!("daily-{day}-{}.erpbackup", uuid::Uuid::new_v4()));
        }
        let result = archive_backup(&inner.db, &self.root, &target);
        if let Err(ref e) = result {
            inner.db.execute("INSERT INTO backup_runs(kind,path,status,error_message,completed_at) VALUES ('automatic',?1,'failed',?2,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",params![target.to_string_lossy(),e.message])?;
        } else {
            inner.db.execute("INSERT INTO backup_runs(kind,path,sha256,size_bytes,status,completed_at) VALUES ('automatic',?1,?2,?3,'verified',strftime('%Y-%m-%dT%H:%M:%fZ','now'))",params![target.to_string_lossy(),file_digest(&target)?,fs::metadata(&target)?.len()as i64])?;
            let old:Vec<(String,String)>=inner.db.prepare("SELECT path,sha256 FROM backup_runs WHERE kind='automatic' AND status='verified' ORDER BY id DESC LIMIT -1 OFFSET 30")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            for (path, checksum) in old {
                let path = PathBuf::from(path);
                if path.parent() == Some(folder.as_path())
                    && path.is_file()
                    && file_digest(&path)? == checksum
                {
                    fs::remove_file(path)?;
                }
            }
        }
        result
    }
    pub fn set_backup_directory(&self, path: &Path) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("settings.write")?;
        let path = path.canonicalize()?;
        ensure(
            path.is_dir() && !path.starts_with(&self.root),
            "اختر مجلداً خارج مجلد البيانات الحية",
        )?;
        inner.db.execute(
            "UPDATE store_settings SET backup_directory=?1,revision=revision+1 WHERE id=1",
            [path.to_string_lossy()],
        )?;
        db::audit(
            &inner.db,
            actor.user_id,
            "backup_directory",
            "settings",
            1,
            "",
        )?;
        Ok(json!({"saved":true}))
    }
    pub fn restore(&self, archive: &Path) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("backup.restore")?;
        let parent = self
            .root
            .parent()
            .ok_or_else(|| Error::validation("مجلد البيانات غير صالح"))?;
        let staging = parent.join(format!("restore-{}", uuid::Uuid::new_v4()));
        extract_backup(archive, &staging)?;
        let backup = parent.join(format!("pre-restore-{}.erpbackup", uuid::Uuid::new_v4()));
        archive_backup(&inner.db, &self.root, &backup)?;
        let previous = parent.join(format!("previous-{}", uuid::Uuid::new_v4()));
        begin_replace(&self.root, &previous, &staging)?;
        inner.db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        let old = std::mem::replace(&mut inner.db, Connection::open_in_memory()?);
        drop(old);
        inner.session = None;
        inner.import_grants.clear();
        if let Err(e) = fs::rename(&self.root, &previous) {
            inner.db = db::open(&self.root.join("store.db"))?;
            return Err(e.into());
        }
        if let Err(e) = fs::rename(&staging, &self.root) {
            fs::rename(&previous, &self.root)?;
            inner.db = db::open(&self.root.join("store.db"))?;
            return Err(e.into());
        }
        match db::open(&self.root.join("store.db")) {
            Ok(connection) => inner.db = connection,
            Err(e) => {
                fs::rename(&self.root, &staging)?;
                fs::rename(&previous, &self.root)?;
                inner.db = db::open(&self.root.join("store.db"))?;
                return Err(e);
            }
        }
        if let Err(error) = finish_replace(&self.root) {
            inner.recovery_error = Some(error.message.clone());
            return Err(error);
        }
        inner.db.execute(
            "INSERT INTO audit_log(action,entity_type,reason) VALUES ('restore','system',?1)",
            [format!(
                "استعادة نسخة احتياطية بواسطة المستخدم السابق {}",
                actor.user_id
            )],
        )?;
        Ok(
            json!({"restored":true,"login_required":true,"previous_store":previous.to_string_lossy()}),
        )
    }
    pub fn recover_backup(&self, archive: &Path, username: &str, password: &str) -> Result<Value> {
        let mut inner = self.lock()?;
        ensure(
            inner.recovery_error.is_some(),
            "استخدم الاستعادة المعتادة للمتجر السليم",
        )?;
        let parent = self
            .root
            .parent()
            .ok_or_else(|| Error::validation("مجلد البيانات غير صالح"))?;
        let staging = parent.join(format!("recovery-{}", uuid::Uuid::new_v4()));
        extract_backup(archive, &staging)?;
        let mut restored = db::open(&staging.join("store.db"))?;
        let (user, _, _) = super::auth::login(&mut restored, username, password)?;
        ensure(restored.query_row("SELECT EXISTS(SELECT 1 FROM user_roles u JOIN role_permissions p ON p.role_id=u.role_id WHERE u.user_id=?1 AND p.permission_code='backup.restore')",[user],|r|r.get(0))?,"حساب النسخة لا يملك صلاحية الاستعادة")?;
        db::audit(
            &restored,
            user,
            "recover_store",
            "system",
            1,
            "حفظ مجلد القاعدة المتعطلة دون تغيير",
        )?;
        restored.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        drop(restored);
        let previous = parent.join(format!("damaged-{}", uuid::Uuid::new_v4()));
        begin_replace(&self.root, &previous, &staging)?;
        fs::rename(&self.root, &previous)?;
        if let Err(error) = fs::rename(&staging, &self.root) {
            fs::rename(&previous, &self.root)?;
            return Err(error.into());
        }
        match db::open(&self.root.join("store.db")) {
            Ok(connection) => {
                inner.db = connection;
                inner.recovery_error = None;
                inner.session = None;
            }
            Err(error) => {
                fs::rename(&self.root, &staging)?;
                fs::rename(&previous, &self.root)?;
                return Err(error);
            }
        }
        if let Err(error) = finish_replace(&self.root) {
            inner.recovery_error = Some(error.message.clone());
            return Err(error);
        }
        Ok(
            json!({"recovered":true,"login_required":true,"preserved_store":previous.to_string_lossy()}),
        )
    }
    pub fn attach_image(&self, path: &Path, product: Option<Int>) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require(if product.is_some() {
            "products.write"
        } else {
            "settings.write"
        })?;
        ensure(
            fs::metadata(path)?.len() <= 5 * 1024 * 1024,
            "الصورة أكبر من 5 ميجابايت",
        )?;
        let bytes = fs::read(path)?;
        let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "jpg"
        } else {
            return Err(Error::validation("اختر صورة PNG أو JPEG"));
        };
        let name = format!("{}.{}", uuid::Uuid::new_v4(), extension);
        let relative = format!("assets/{name}");
        let target = self.root.join(&relative);
        File::create_new(&target)?.write_all(&bytes)?;
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = if let Some(id) = product {
            tx.execute(
                "UPDATE products SET image_path=?1,revision=revision+1 WHERE id=?2",
                params![relative, id],
            )?
        } else {
            tx.execute(
                "UPDATE store_settings SET logo_path=?1,revision=revision+1 WHERE id=1",
                [&relative],
            )?
        };
        ensure(result == 1, "السجل غير موجود")?;
        db::audit(
            &tx,
            actor.user_id,
            "attach_image",
            "asset",
            product.map_or(1, |x| x.0),
            &relative,
        )?;
        tx.commit()?;
        Ok(json!({"path":relative}))
    }
}
