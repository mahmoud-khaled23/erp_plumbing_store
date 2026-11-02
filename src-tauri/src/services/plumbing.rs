use super::{
    catalog,
    purchasing::{number, reason},
};
use crate::{
    db,
    error::{Result, ensure},
    models::{
        arithmetic::{MONEY_MAX, SCALE, convert, prorate, sum},
        *,
    },
    state::Actor,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn template(db: &Connection, id: i64) -> Result<(Product, Value)> {
    let rows = db::rows(db, "SELECT * FROM products WHERE id=?1 AND active=1", [id])?;
    ensure(rows.len() == 1, "اختر صنف قالب نشطاً")?;
    let p = &rows[0];
    let units = db::rows(
        db,
        "SELECT * FROM product_units WHERE product_id=?1 AND active=1 ORDER BY id",
        [id],
    )?;
    let product = Product {
        id: None,
        revision: Int(0),
        code: String::new(),
        name: p["name_ar"].as_str().unwrap().into(),
        category_id: Int(number(p, "category_id")),
        brand: p["brand"].as_str().unwrap().into(),
        material: String::new(),
        size: String::new(),
        base_unit_id: Int(number(p, "base_unit_id")),
        standard_cost: Int(number(p, "standard_cost_minor")),
        min_stock: Int(number(p, "min_stock_micros")),
        active: true,
        units: units
            .iter()
            .map(|u| ProductUnit {
                id: None,
                unit_id: Int(number(u, "unit_id")),
                numerator: Int(number(u, "factor_numerator")),
                denominator: Int(number(u, "factor_denominator")),
                barcode: None,
                retail_price: Int(number(u, "retail_price_minor")),
                wholesale_price: Int(number(u, "wholesale_price_minor")),
                active: true,
            })
            .collect(),
    };
    let planning = db::rows(
        db,
        "SELECT * FROM product_planning WHERE product_id=?1",
        [id],
    )?;
    Ok((product, json!([rows, units, planning])))
}
fn calculate_variants(
    db: &Connection,
    id: i64,
    inputs: &[VariantInput],
) -> Result<(Vec<Value>, String)> {
    ensure(
        !inputs.is_empty() && inputs.len() <= 100,
        "اختر من صنف إلى مائة صنف في دفعة المقاسات",
    )?;
    let (base, snapshot) = template(db, id)?;
    let mut codes = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    let mut barcodes = BTreeSet::new();
    let mut rows = Vec::new();
    for v in inputs {
        let (code, size, material) = (v.code.trim(), v.size.trim(), v.material.trim());
        ensure(
            !code.is_empty()
                && code.len() <= 128
                && !size.is_empty()
                && size.chars().count() <= 120
                && !material.is_empty()
                && material.chars().count() <= 120,
            "الكود والمقاس والخامة مطلوبة ضمن الطول المسموح",
        )?;
        ensure(
            codes.insert(code.to_ascii_lowercase()) && pairs.insert((size, material)),
            "الكود أو المقاس والخامة مكرران",
        )?;
        ensure(!db.query_row("SELECT EXISTS(SELECT 1 FROM products WHERE code=?1) OR EXISTS(SELECT 1 FROM product_variants WHERE template_product_id=?2 AND size_snapshot=?3 AND material_snapshot=?4)",params![code,id,size,material],|r|r.get::<_,bool>(0))?,"الكود أو مقاس القالب وخامته موجود بالفعل")?;
        let barcode = v
            .barcode
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty());
        if let Some(b) = barcode {
            ensure(
                b.len() <= 128 && barcodes.insert(b),
                "الباركود طويل أو مكرر",
            )?;
            ensure(
                !db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM product_units WHERE barcode=?1)",
                    [b],
                    |r| r.get::<_, bool>(0),
                )?,
                "الباركود مستخدم",
            )?;
        }
        rows.push(json!({"code":code,"name_ar":format!("{} / {} / {}",base.name,size,material),"size":size,"material":material,"barcode":barcode}));
    }
    let hash = db::digest(serde_json::to_string(&json!([id, snapshot, rows]))?.as_bytes());
    Ok((rows, hash))
}
pub fn variant_preview(
    db: &Connection,
    a: &Actor,
    id: i64,
    inputs: &[VariantInput],
) -> Result<Value> {
    a.require("products.variants")?;
    let (rows, hash) = calculate_variants(db, id, inputs)?;
    Ok(json!({"rows":rows,"preview_hash":hash}))
}
pub fn variants(db: &Connection, a: &Actor, id: i64, after: i64, limit: u16) -> Result<Value> {
    a.require("products.read")?;
    Ok(json!(db::rows(
        db,
        "SELECT p.id AS product_id,p.code,p.name_ar,p.size,p.material,v.template_product_id,v.template_revision FROM product_variants v JOIN products p ON p.id=v.product_id WHERE v.template_product_id=?1 AND p.id>?2 ORDER BY p.id LIMIT ?3",
        params![id, after, i64::from(limit.clamp(1, 200))]
    )?))
}
pub fn create_variants(
    db: &Connection,
    a: &Actor,
    id: i64,
    inputs: &[VariantInput],
    hash: &str,
    why: &str,
) -> Result<Value> {
    a.require("products.variants")?;
    a.require("products.write")?;
    reason(why)?;
    let (rows, current) = calculate_variants(db, id, inputs)?;
    ensure(
        hash == current,
        "تغير القالب أو الأسعار؛ أعد معاينة المقاسات",
    )?;
    let (base, _) = template(db, id)?;
    let mut ids = Vec::new();
    for row in rows {
        let mut p = base.clone();
        p.code = row["code"].as_str().unwrap().into();
        p.name = row["name_ar"].as_str().unwrap().into();
        p.size = row["size"].as_str().unwrap().into();
        p.material = row["material"].as_str().unwrap().into();
        for u in &mut p.units {
            if u.unit_id == p.base_unit_id {
                u.barcode = row["barcode"].as_str().map(str::to_owned);
            }
        }
        let created = catalog::product(db, &p)?;
        let product = number(&created, "id");
        db.execute("UPDATE products SET warranty_days=(SELECT warranty_days FROM products WHERE id=?1) WHERE id=?2",params![id,product])?;
        db.execute("INSERT INTO product_variants(product_id,template_product_id,template_revision,size_snapshot,material_snapshot,created_by) SELECT ?1,id,revision,?3,?4,?5 FROM products WHERE id=?2",params![product,id,p.size,p.material,a.user_id])?;
        db.execute("INSERT INTO product_planning(product_id,supplier_id,reorder_point_micros,max_level_micros,lead_time_days,weight_grams) SELECT ?1,supplier_id,reorder_point_micros,max_level_micros,lead_time_days,weight_grams FROM product_planning WHERE product_id=?2",params![product,id])?;
        ids.push(Int(product));
    }
    Ok(json!({"product_ids":ids,"count":Int(ids.len() as i64)}))
}
pub fn warranty(
    db: &Connection,
    a: &Actor,
    id: i64,
    revision: i64,
    days: Option<u16>,
    why: &str,
) -> Result<Value> {
    a.require("products.warranty")?;
    reason(why)?;
    ensure(
        days.is_none_or(|d| (1..=36500).contains(&d)),
        "مدة الضمان من يوم إلى ٣٦٥٠٠ يوم، أو اتركها فارغة",
    )?;
    ensure(
        db.execute(
            "UPDATE products SET warranty_days=?1,revision=revision+1 WHERE id=?2 AND revision=?3",
            params![days, id, revision],
        )? == 1,
        "تغير الصنف؛ أعد تحميله قبل تعديل الضمان",
    )?;
    Ok(json!({"id":Int(id)}))
}
pub fn bundles(db: &Connection, a: &Actor, search: &str, after: i64, limit: u16) -> Result<Value> {
    a.require("products.read")?;
    ensure(search.len() <= 256, "نص البحث طويل")?;
    Ok(json!(db::rows(
        db,
        "SELECT * FROM sales_bundles WHERE id>?1 AND (?2='' OR instr(name_ar,?2)>0 OR instr(code,?2)>0) ORDER BY id LIMIT ?3",
        params![after, search, i64::from(limit.clamp(1, 200))]
    )?))
}
pub fn bundle(db: &Connection, a: &Actor, id: i64) -> Result<Value> {
    a.require("products.read")?;
    Ok(
        json!({"header":db::rows(db,"SELECT * FROM sales_bundles WHERE id=?1",[id])?,"items":db::rows(db,"SELECT b.product_unit_id AS unit_id,b.quantity_micros,u.product_id,p.code,p.name_ar,n.name_ar AS unit_name,u.factor_numerator,u.factor_denominator,u.active,p.active AS product_active FROM sales_bundle_items b JOIN product_units u ON u.id=b.product_unit_id JOIN products p ON p.id=u.product_id JOIN units n ON n.id=u.unit_id WHERE b.bundle_id=?1 ORDER BY b.product_unit_id",[id])?}),
    )
}
pub fn save_bundle(db: &Connection, a: &Actor, b: &SalesBundle, why: &str) -> Result<Value> {
    a.require("products.bundles")?;
    reason(why)?;
    ensure(
        !b.code.trim().is_empty()
            && b.code.len() <= 64
            && !b.name.trim().is_empty()
            && b.name.chars().count() <= 200
            && !b.items.is_empty()
            && b.items.len() <= 100,
        "بيانات الطقم أو عدد مكوناته غير صالح",
    )?;
    let mut unique = BTreeSet::new();
    for i in &b.items {
        ensure(
            unique.insert(i.unit_id.0) && (1..=1_000_000_000_000_000).contains(&i.quantity.0),
            "وحدة الطقم مكررة أو كميتها غير صالحة",
        )?;
        let (num,den):(i64,i64)=db.query_row("SELECT u.factor_numerator,u.factor_denominator FROM product_units u JOIN products p ON p.id=u.product_id WHERE u.id=?1 AND (?2=0 OR (u.active=1 AND p.active=1))",params![i.unit_id,b.active],|r|Ok((r.get(0)?,r.get(1)?)))?;
        ensure(
            convert(i.quantity.0, num, den)? <= 1_000_000_000_000_000,
            "كمية المكون تتجاوز الحد المسموح",
        )?;
    }
    let id = if let Some(id) = b.id {
        ensure(db.execute("UPDATE sales_bundles SET code=?1,name_ar=?2,active=?3,revision=revision+1 WHERE id=?4 AND revision=?5",params![b.code.trim(),b.name.trim(),b.active,id,b.revision])?==1,"تغير تعريف الطقم؛ أعد تحميله")?;
        id.0
    } else {
        db.execute(
            "INSERT INTO sales_bundles(code,name_ar,active) VALUES (?1,?2,?3)",
            params![b.code.trim(), b.name.trim(), b.active],
        )?;
        db.last_insert_rowid()
    };
    db.execute("DELETE FROM sales_bundle_items WHERE bundle_id=?1", [id])?;
    for i in &b.items {
        db.execute("INSERT INTO sales_bundle_items(bundle_id,product_unit_id,quantity_micros) VALUES (?1,?2,?3)",params![id,i.unit_id,i.quantity])?;
    }
    Ok(json!({"id":Int(id)}))
}
pub fn expand(
    db: &Connection,
    a: &Actor,
    id: i64,
    quantity: i64,
    wholesale: bool,
) -> Result<Value> {
    a.require("sales.write")?;
    ensure(
        (1..=1_000_000_000_000_000).contains(&quantity),
        "كمية الأطقم غير صالحة",
    )?;
    let name: String = db.query_row(
        "SELECT name_ar FROM sales_bundles WHERE id=?1 AND active=1",
        [id],
        |r| r.get(0),
    )?;
    let mut rows = db::rows(
        db,
        "SELECT u.id AS unit_id,u.product_id,p.code,p.name_ar,n.name_ar AS unit_name,b.quantity_micros AS per_bundle_micros,u.factor_numerator,u.factor_denominator,u.retail_price_minor,u.wholesale_price_minor,u.active,p.active AS product_active FROM sales_bundle_items b JOIN product_units u ON u.id=b.product_unit_id JOIN products p ON p.id=u.product_id JOIN units n ON n.id=u.unit_id WHERE b.bundle_id=?1 ORDER BY b.product_unit_id",
        [id],
    )?;
    ensure(
        !rows.is_empty() && rows.len() <= 100,
        "الطقم فارغ أو يتجاوز عدد المكونات المسموح",
    )?;
    for r in &mut rows {
        ensure(
            number(r, "active") == 1 && number(r, "product_active") == 1,
            "يوجد مكون غير نشط في الطقم",
        )?;
        let qty = convert(quantity, number(r, "per_bundle_micros"), SCALE)?;
        ensure(
            qty > 0
                && qty <= 1_000_000_000_000_000
                && convert(
                    qty,
                    number(r, "factor_numerator"),
                    number(r, "factor_denominator"),
                )? <= 1_000_000_000_000_000,
            "كمية المكون خارج الحدود",
        )?;
        let price = number(
            r,
            if wholesale {
                "wholesale_price_minor"
            } else {
                "retail_price_minor"
            },
        );
        let gross = prorate(price, qty, SCALE)?;
        ensure(gross <= MONEY_MAX, "قيمة مكون الطقم تتجاوز الحد المسموح")?;
        r["quantity_micros"] = json!(Int(qty));
        r["unit_price_minor"] = json!(Int(price));
        r["subtotal_minor"] = json!(Int(gross));
        r["bundle_name"] = json!(name);
    }
    let total = sum(rows.iter().map(|r| number(r, "subtotal_minor")))?;
    Ok(json!({"rows":rows,"total_minor":Int(total)}))
}
