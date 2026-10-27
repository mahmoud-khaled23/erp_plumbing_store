use crate::{
    error::{Result, ensure},
    models::*,
    state::Actor,
};
use rusqlite::{Connection, params, types::Value as Sql};
use serde_json::{Value, json};

fn save(db: &Connection, table: &str, id: Option<Int>, fields: Vec<(&str, Sql)>) -> Result<Value> {
    let mut values: Vec<Sql> = fields.iter().map(|(_, v)| v.clone()).collect();
    let id = if let Some(id) = id {
        values.push(Sql::Integer(id.0));
        let assignments = fields
            .iter()
            .enumerate()
            .map(|(i, (k, _))| format!("{k}=?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "UPDATE {table} SET {assignments} WHERE id=?{}",
            values.len()
        );
        ensure(
            db.execute(&sql, rusqlite::params_from_iter(values))? == 1,
            "السجل غير موجود",
        )?;
        id.0
    } else {
        let columns = fields.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(",");
        let parameters = (1..=values.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        db.execute(
            &format!("INSERT INTO {table}({columns}) VALUES ({parameters})"),
            rusqlite::params_from_iter(values),
        )?;
        db.last_insert_rowid()
    };
    Ok(json!({"id":Int(id)}))
}
fn text(s: &str) -> Sql {
    Sql::Text(s.trim().into())
}
fn id(v: Option<Int>) -> Sql {
    v.map_or(Sql::Null, |i| Sql::Integer(i.0))
}
fn flag(v: bool) -> Sql {
    Sql::Integer(i64::from(v))
}
pub fn master(db: &Connection, m: &Master) -> Result<Value> {
    match m {
        Master::Category {
            id: i,
            name,
            parent_id,
        } => save(
            db,
            "categories",
            *i,
            vec![("name_ar", text(name)), ("parent_id", id(*parent_id))],
        ),
        Master::Unit { id: i, name, code } => save(
            db,
            "units",
            *i,
            vec![("name_ar", text(name)), ("code", text(code))],
        ),
        Master::Warehouse {
            id: i,
            name,
            code,
            address,
            active,
        } => save(
            db,
            "warehouses",
            *i,
            vec![
                ("name_ar", text(name)),
                ("code", text(code)),
                ("address", text(address)),
                ("active", flag(*active)),
            ],
        ),
        Master::Account {
            id: i,
            code,
            name,
            parent_id,
            kind,
            postable,
            active,
        } => save(
            db,
            "accounts",
            *i,
            vec![
                ("code", text(code)),
                ("name_ar", text(name)),
                ("parent_id", id(*parent_id)),
                ("kind", text(kind.code())),
                ("is_postable", flag(*postable)),
                ("active", flag(*active)),
            ],
        ),
        Master::CashAccount {
            id: i,
            name,
            bank,
            account_id,
            bank_name,
            account_number,
            active,
        } => {
            let kind = if *bank { "bank" } else { "cash" };
            ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1 AND (control_kind=?2 OR control_kind IS NULL) AND kind='asset' AND active=1 AND is_postable=1)",params![account_id,kind],|r|r.get(0))?,"يجب اختيار حساب خزينة أو بنك مناسب")?;
            db.execute(
                "UPDATE accounts SET control_kind=?1 WHERE id=?2 AND control_kind IS NULL",
                params![kind, account_id],
            )?;
            save(
                db,
                "cash_accounts",
                *i,
                vec![
                    ("name_ar", text(name)),
                    ("kind", text(kind)),
                    ("account_id", Sql::Integer(account_id.0)),
                    ("bank_name", text(bank_name)),
                    ("account_number", text(account_number)),
                    ("active", flag(*active)),
                ],
            )
        }
        Master::ExpenseCategory {
            id: i,
            name,
            account_id,
            active,
        } => {
            ensure(db.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1 AND kind='expense' AND is_postable=1 AND active=1)",[account_id],|r|r.get(0))?,"يجب اختيار حساب مصروف نشط")?;
            save(
                db,
                "expense_categories",
                *i,
                vec![
                    ("name_ar", text(name)),
                    ("account_id", Sql::Integer(account_id.0)),
                    ("active", flag(*active)),
                ],
            )
        }
        Master::WarehouseProduct {
            warehouse_id,
            product_id,
            min_stock,
            bin,
        } => {
            db.execute("INSERT INTO warehouse_product_settings(warehouse_id,product_id,min_stock_micros,bin_location) VALUES (?1,?2,?3,?4) ON CONFLICT(warehouse_id,product_id) DO UPDATE SET min_stock_micros=excluded.min_stock_micros,bin_location=excluded.bin_location",params![warehouse_id,product_id,min_stock,bin])?;
            Ok(json!({"saved":true}))
        }
        Master::Sequence {
            kind,
            year,
            prefix,
            padding,
        } => {
            ensure(
                prefix.len() <= 30 && !prefix.trim().is_empty(),
                "بادئة الترقيم غير صالحة",
            )?;
            db.execute("INSERT INTO number_sequences(document_type,fiscal_year,prefix,padding) VALUES (?1,?2,?3,?4) ON CONFLICT(document_type,fiscal_year) DO UPDATE SET prefix=excluded.prefix,padding=excluded.padding",params![kind,year,prefix,padding])?;
            Ok(json!({"saved":true}))
        }
    }
}
pub fn settings(db: &Connection, s: &Settings) -> Result<Value> {
    ensure(
        s.timezone.parse::<chrono_tz::Tz>().is_ok(),
        "المنطقة الزمنية غير صالحة",
    )?;
    ensure(
        !s.store_name.trim().is_empty()
            && s.currency_code.len() == 3
            && s.currency_code.bytes().all(|c| c.is_ascii_uppercase()),
        "اسم المتجر أو رمز العملة غير صالح",
    )?;
    ensure(db.execute("UPDATE store_settings SET store_name=?1,tax_number=?2,address=?3,phones=?4,currency_code=?5,currency_symbol=?6,currency_minor_digits=?7,digit_style=?8,timezone=?9,vat_enabled=?10,default_vat_rate_bps=?11,allow_negative_stock=?12,credit_limit_policy=?13,default_print_layout=?14,receipt_footer=?15,print_copies=?16,auto_backup=?17,revision=revision+1 WHERE id=1 AND revision=?18",params![s.store_name,s.tax_number,s.address,s.phones,s.currency_code,s.currency_symbol,s.currency_digits,if s.arabic_digits {"arabic_indic"} else {"western"},s.timezone,s.vat_enabled,s.vat_bps,s.negative_stock,if s.block_credit {"block"} else {"warn"},if s.thermal {"thermal80"} else {"a4"},s.receipt_footer,s.print_copies,s.auto_backup,s.revision])?==1,"تم تعديل الإعدادات؛ أعد تحميلها")?;
    Ok(json!({"saved":true}))
}
pub fn period(
    db: &Connection,
    a: &Actor,
    id: Option<i64>,
    name: &str,
    start: &str,
    end: &str,
    locked: bool,
    reason: &str,
) -> Result<Value> {
    super::documents::date(start)?;
    super::documents::date(end)?;
    ensure(!reason.trim().is_empty(), "سبب تعديل الفترة مطلوب")?;
    let locked_at = locked.then(|| chrono::Utc::now().to_rfc3339());
    let user = locked.then_some(a.user_id);
    let id = if let Some(id) = id {
        ensure(db.execute("UPDATE fiscal_periods SET name=?1,starts_on=?2,ends_on=?3,locked_at=?4,locked_by=?5 WHERE id=?6",params![name,start,end,locked_at,user,id])?==1,"الفترة غير موجودة")?;
        id
    } else {
        db.execute("INSERT INTO fiscal_periods(name,starts_on,ends_on,locked_at,locked_by) VALUES (?1,?2,?3,?4,?5)",params![name,start,end,locked_at,user])?;
        db.last_insert_rowid()
    };
    Ok(json!({"id":Int(id)}))
}
pub fn access(db: &Connection, a: &Actor, user: i64, active: bool, roles: &[Int]) -> Result<Value> {
    ensure(
        !roles.is_empty() && roles.len() <= 4,
        "يجب تحديد دور واحد على الأقل",
    )?;
    if !active || !roles.contains(&Int(1)) {
        ensure(db.query_row("SELECT COUNT(*) FROM users u JOIN user_roles r ON r.user_id=u.id WHERE u.active=1 AND r.role_id=1 AND u.id<>?1",[user],|r|r.get::<_,i64>(0))?>0,"لا يمكن تعطيل آخر مدير للنظام")?;
    }
    ensure(user != a.user_id || active, "لا يمكن تعطيل حسابك الحالي")?;
    ensure(
        db.execute(
            "UPDATE users SET active=?1 WHERE id=?2",
            params![active, user],
        )? == 1,
        "المستخدم غير موجود",
    )?;
    db.execute("DELETE FROM user_roles WHERE user_id=?1", [user])?;
    for role in roles {
        db.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES (?1,?2)",
            params![user, role],
        )?;
    }
    Ok(json!({"saved":true}))
}
