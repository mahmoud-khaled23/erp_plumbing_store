use crate::error::{Error, Result, ensure};
use rusqlite::{Connection, Params, types::ValueRef};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_foundation.sql",
        include_str!("../../migrations/0001_foundation.sql"),
    ),
    (
        "0002_documents.sql",
        include_str!("../../migrations/0002_documents.sql"),
    ),
    (
        "0003_ledgers.sql",
        include_str!("../../migrations/0003_ledgers.sql"),
    ),
    (
        "0004_guards.sql",
        include_str!("../../migrations/0004_guards.sql"),
    ),
    (
        "0005_detail_guards.sql",
        include_str!("../../migrations/0005_detail_guards.sql"),
    ),
    (
        "0006_views.sql",
        include_str!("../../migrations/0006_views.sql"),
    ),
    (
        "0007_reference_data.sql",
        include_str!("../../migrations/0007_reference_data.sql"),
    ),
    (
        "0008_cost_attribution.sql",
        include_str!("../../migrations/0008_cost_attribution.sql"),
    ),
];
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn open(path: &Path) -> Result<Connection> {
    ensure(
        rusqlite::version_number() >= 3_051_003,
        "إصدار SQLite غير مدعوم؛ يلزم تحديث التطبيق",
    )?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut db = Connection::open(path)?;
    db.execute_batch(include_str!("../../db/connection.sql"))?;
    ensure(
        db.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))? == 1,
        "تعذر تفعيل العلاقات المرجعية",
    )?;
    let existing: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='schema_migrations')",
        [],
        |r| r.get(0),
    )?;
    if existing
        && db.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| {
            r.get::<_, i64>(0)
        })? < (MIGRATIONS.len() as i64)
    {
        let root = path
            .parent()
            .ok_or_else(|| Error::validation("مسار قاعدة البيانات غير صالح"))?;
        let folder = root
            .parent()
            .ok_or_else(|| Error::validation("مسار النسخ الاحتياطي غير صالح"))?;
        crate::services::files::archive_backup(
            &db,
            root,
            &folder.join(format!("pre-migration-{}.erpbackup", uuid::Uuid::new_v4())),
        )?;
    }
    migrate(&mut db)?;
    integrity(&db, false)?;
    ensure(
        db.query_row(
            "SELECT COUNT(*) FROM documents WHERE status='posting'",
            [],
            |r| r.get::<_, i64>(0),
        )? == 0,
        "يوجد ترحيل غير مكتمل؛ استعد نسخة احتياطية سليمة",
    )?;
    Ok(db)
}
pub fn migrate(db: &mut Connection) -> Result<()> {
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='schema_migrations')",
        [],
        |r| r.get(0),
    )?;
    let applied: Vec<(i64, String, String)> = if exists {
        db.prepare("SELECT version,name,sha256 FROM schema_migrations ORDER BY version")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    } else {
        Vec::new()
    };
    ensure(
        applied.len() <= MIGRATIONS.len(),
        "قاعدة البيانات من إصدار أحدث من التطبيق",
    )?;
    for (i, (version, name, checksum)) in applied.iter().enumerate() {
        ensure(
            *version == (i + 1) as i64
                && *name == MIGRATIONS[i].0
                && *checksum == digest(MIGRATIONS[i].1.as_bytes()),
            "ملفات ترقية قاعدة البيانات تغيرت أو تسلسلها غير صالح",
        )?;
    }
    for (i, (name, sql)) in MIGRATIONS.iter().enumerate().skip(applied.len()) {
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations(version,name,sha256) VALUES (?1,?2,?3)",
            rusqlite::params![(i + 1) as i64, name, digest(sql.as_bytes())],
        )?;
        ensure(
            tx.prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none(),
            "فشل فحص العلاقات بعد الترقية",
        )?;
        tx.commit()?;
    }
    Ok(())
}
pub fn integrity(db: &Connection, full: bool) -> Result<()> {
    let sql = if full {
        "PRAGMA integrity_check"
    } else {
        "PRAGMA quick_check"
    };
    let result: Vec<String> = db
        .prepare(sql)?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ensure(
        result == ["ok"],
        "فشل فحص سلامة قاعدة البيانات؛ استخدم نسخة احتياطية",
    )?;
    ensure(
        db.prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_none(),
        "العلاقات المرجعية غير سليمة",
    )
}
pub fn rows(db: &Connection, sql: &str, params: impl Params) -> Result<Vec<Value>> {
    let mut statement = db.prepare(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|s| (*s).into())
        .collect();
    let mut cursor = statement.query(params)?;
    let mut values = Vec::new();
    while let Some(row) = cursor.next()? {
        let mut object = Map::new();
        for (i, name) in columns.iter().enumerate() {
            let value = match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => Value::String(n.to_string()),
                ValueRef::Text(v) => Value::String(String::from_utf8_lossy(v).into()),
                _ => return Err(Error::new("data_type", "نوع بيانات غير مسموح في النتائج")),
            };
            object.insert(name.clone(), value);
        }
        values.push(Value::Object(object));
    }
    Ok(values)
}
pub fn audit(
    db: &Connection,
    user: i64,
    action: &str,
    entity: &str,
    id: i64,
    reason: &str,
) -> Result<()> {
    db.execute("INSERT INTO audit_log(user_id,action,entity_type,entity_id,reason) VALUES (?1,?2,?3,?4,?5)", rusqlite::params![user,action,entity,id.to_string(),reason])?;
    Ok(())
}
