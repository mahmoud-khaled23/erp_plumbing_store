use crate::{
    error::{Result, ensure},
    models::arithmetic::MONEY_MAX,
    models::*,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{0640}' | '\u{064b}'..='\u{065f}' | '\u{0670}'))
        .map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
}
pub fn product(db: &Connection, p: &Product) -> Result<Value> {
    ensure(
        !p.name.trim().is_empty() && !p.code.trim().is_empty() && p.units.len() <= 50,
        "اسم وكود ووحدات الصنف مطلوبة",
    )?;
    ensure(
        p.units
            .iter()
            .any(|u| u.unit_id == p.base_unit_id && u.numerator == u.denominator && u.active),
        "يجب إضافة الوحدة الأساسية بمعامل واحد",
    )?;
    ensure(
        (0..=MONEY_MAX).contains(&p.standard_cost.0) && p.min_stock.0 >= 0,
        "التكلفة والحد الأدنى غير صالحين",
    )?;
    let id = if let Some(id) = p.id {
        let count = db.execute("UPDATE products SET code=?1,name_ar=?2,normalized_name=?3,category_id=?4,brand=?5,material=?6,size=?7,base_unit_id=?8,standard_cost_minor=?9,min_stock_micros=?10,active=?11,revision=revision+1 WHERE id=?12 AND revision=?13",
            params![p.code.trim(),p.name.trim(),normalize(&p.name),p.category_id,p.brand,p.material,p.size,p.base_unit_id,p.standard_cost,p.min_stock,p.active,id,p.revision])?;
        ensure(count == 1, "تم تعديل الصنف في جلسة أخرى؛ أعد تحميله")?;
        id.0
    } else {
        db.execute("INSERT INTO products(code,name_ar,normalized_name,category_id,brand,material,size,base_unit_id,standard_cost_minor,min_stock_micros,active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![p.code.trim(),p.name.trim(),normalize(&p.name),p.category_id,p.brand,p.material,p.size,p.base_unit_id,p.standard_cost,p.min_stock,p.active])?;
        db.last_insert_rowid()
    };
    db.execute(
        "UPDATE product_units SET active=0 WHERE product_id=?1",
        [id],
    )?;
    let mut units = std::collections::BTreeSet::new();
    for u in &p.units {
        ensure(units.insert(u.unit_id.0), "وحدة الصنف مكررة")?;
        let barcode = u
            .barcode
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(unit_id) = u.id {
            ensure(db.execute("UPDATE product_units SET unit_id=?1,factor_numerator=?2,factor_denominator=?3,barcode=?4,retail_price_minor=?5,wholesale_price_minor=?6,active=?7 WHERE id=?8 AND product_id=?9",params![u.unit_id,u.numerator,u.denominator,barcode,u.retail_price,u.wholesale_price,u.active,unit_id,id])? == 1,"وحدة الصنف غير موجودة")?;
        } else {
            db.execute("INSERT INTO product_units(product_id,unit_id,factor_numerator,factor_denominator,barcode,retail_price_minor,wholesale_price_minor,active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",params![id,u.unit_id,u.numerator,u.denominator,barcode,u.retail_price,u.wholesale_price,u.active])?;
        }
    }
    Ok(json!({"id":Int(id)}))
}
pub fn party(db: &Connection, p: &Party) -> Result<Value> {
    ensure(
        !p.name.trim().is_empty() && !p.code.trim().is_empty() && (p.customer || p.supplier),
        "اسم وكود ونوع الطرف مطلوبة",
    )?;
    let id = if let Some(id) = p.id {
        ensure(db.execute("UPDATE parties SET code=?1,name_ar=?2,normalized_name=?3,is_customer=?4,is_supplier=?5,phone=?6,address=?7,tax_number=?8,credit_limit_minor=?9,payment_terms_days=?10,active=?11,notes=?12 WHERE id=?13",params![p.code,p.name,normalize(&p.name),p.customer,p.supplier,p.phone,p.address,p.tax_number,p.credit_limit,p.terms_days,p.active,p.notes,id])? == 1,"الطرف غير موجود")?;
        id.0
    } else {
        db.execute("INSERT INTO parties(code,name_ar,normalized_name,is_customer,is_supplier,phone,address,tax_number,credit_limit_minor,payment_terms_days,active,notes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![p.code,p.name,normalize(&p.name),p.customer,p.supplier,p.phone,p.address,p.tax_number,p.credit_limit,p.terms_days,p.active,p.notes])?;
        db.last_insert_rowid()
    };
    Ok(json!({"id":Int(id)}))
}
pub fn seed_demo(db: &Connection) -> Result<()> {
    ensure(
        db.query_row("SELECT COUNT(*) FROM products", [], |r| r.get::<_, i64>(0))? == 0,
        "البيانات التجريبية متاحة للمتجر الجديد فقط",
    )?;
    let mut reader =
        csv::Reader::from_reader(include_bytes!("../../seeds/demo_products.csv").as_slice());
    for record in reader.records() {
        let r = record
            .map_err(|_| crate::error::Error::validation("ملف البيانات التجريبية غير صالح"))?;
        let num = |i: usize| -> Result<i64> {
            r[i].parse()
                .map_err(|_| crate::error::Error::validation("رقم تجريبي غير صالح"))
        };
        product(
            db,
            &Product {
                id: None,
                revision: Int(0),
                code: r[0].into(),
                name: r[1].into(),
                category_id: Int(num(2)?),
                brand: r[3].into(),
                material: r[4].into(),
                size: r[5].into(),
                base_unit_id: Int(num(6)?),
                standard_cost: Int(num(7)?),
                min_stock: Int(5_000_000),
                active: true,
                units: vec![ProductUnit {
                    id: None,
                    unit_id: Int(num(6)?),
                    numerator: Int(1),
                    denominator: Int(1),
                    barcode: Some(r[10].into()),
                    retail_price: Int(num(8)?),
                    wholesale_price: Int(num(9)?),
                    active: true,
                }],
            },
        )?;
    }
    db.execute(
        "UPDATE store_settings SET demo_seeded_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        [],
    )?;
    Ok(())
}
