use crate::{
    db,
    error::{Error, Result, ensure},
    models::{Int, Setup},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use chrono::{Duration, Utc};
use rand_core::OsRng;
use rusqlite::{Connection, OptionalExtension, params};

fn hash(password: &str) -> Result<String> {
    ensure(
        (12..=256).contains(&password.chars().count()),
        "كلمة المرور يجب أن تحتوي على 12 إلى 256 حرفاً",
    )?;
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|h| h.to_string())
        .map_err(|_| Error::new("password", "تعذر حماية كلمة المرور"))
}
fn verify(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}
pub fn create_user(
    db: &Connection,
    username: &str,
    name: &str,
    password: &str,
    roles: &[Int],
) -> Result<i64> {
    ensure(
        !name.trim().is_empty() && !roles.is_empty() && roles.len() <= 4,
        "الاسم والدور مطلوبان",
    )?;
    let hash = hash(password)?;
    db.execute(
        "INSERT INTO users(username,display_name,password_hash) VALUES (?1,?2,?3)",
        params![username.trim(), name.trim(), hash],
    )?;
    let id = db.last_insert_rowid();
    for role in roles {
        db.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES (?1,?2)",
            params![id, role],
        )?;
    }
    Ok(id)
}
pub fn setup(db: &Connection, input: &Setup) -> Result<()> {
    ensure(
        db.query_row(
            "SELECT setup_completed_at IS NULL FROM store_settings",
            [],
            |r| r.get::<_, bool>(0),
        )?,
        "تم إعداد النظام مسبقاً",
    )?;
    ensure(
        !input.store_name.trim().is_empty() && (2000..=9998).contains(&input.fiscal_year),
        "اسم المتجر أو السنة غير صالح",
    )?;
    let user = create_user(
        db,
        &input.username,
        &input.display_name,
        &input.password,
        &[Int(1)],
    )?;
    db.execute("UPDATE store_settings SET store_name=?1,setup_completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",[input.store_name.trim()])?;
    db.execute(
        "INSERT INTO fiscal_periods(name,starts_on,ends_on) VALUES (?1,?2,?3)",
        params![
            input.fiscal_year.to_string(),
            format!("{}-01-01", input.fiscal_year),
            format!("{}-12-31", input.fiscal_year)
        ],
    )?;
    if input.demo {
        super::catalog::seed_demo(db)?;
    }
    db::audit(db, user, "setup", "store", 1, "")
}
pub fn login(db: &mut Connection, username: &str, password: &str) -> Result<(i64, String, String)> {
    ensure(
        username.len() <= 80 && password.len() <= 1024,
        "بيانات الدخول غير صحيحة",
    )?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let user: Option<(i64,String,String,bool,i64,Option<String>)> = tx.query_row("SELECT id,password_hash,display_name,active,failed_login_count,locked_until FROM users WHERE username=?1 COLLATE NOCASE",[username.trim()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    let Some((id, hash, name, active, failures, locked)) = user else {
        // Do comparable password work even for an unknown username.
        let _ =
            Argon2::default().hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng));
        return Err(Error::new("login", "بيانات الدخول غير صحيحة"));
    };
    let locked = locked
        .as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .is_some_and(|v| v > Utc::now());
    if !active || locked {
        return Err(Error::new(
            "login",
            "بيانات الدخول غير صحيحة أو الحساب موقوف مؤقتاً",
        ));
    }
    if !verify(&hash, password) {
        let next = failures.saturating_add(1);
        let until = if next >= 5 {
            Some((Utc::now() + Duration::minutes(5)).to_rfc3339())
        } else {
            None
        };
        tx.execute(
            "UPDATE users SET failed_login_count=?1,locked_until=?2 WHERE id=?3",
            params![next, until, id],
        )?;
        db::audit(&tx, id, "login_failed", "user", id, "")?;
        tx.commit()?;
        return Err(Error::new("login", "بيانات الدخول غير صحيحة"));
    }
    tx.execute(
        "UPDATE users SET failed_login_count=0,locked_until=NULL WHERE id=?1",
        [id],
    )?;
    db::audit(&tx, id, "login", "user", id, "")?;
    tx.commit()?;
    Ok((id, hash, name))
}
pub fn change_password(db: &mut Connection, user: i64, old: &str, new: &str) -> Result<()> {
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current: String =
        tx.query_row("SELECT password_hash FROM users WHERE id=?1", [user], |r| {
            r.get(0)
        })?;
    ensure(verify(&current, old), "كلمة المرور الحالية غير صحيحة")?;
    tx.execute("UPDATE users SET password_hash=?1,must_change_password=0,password_changed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2",params![hash(new)?,user])?;
    db::audit(&tx, user, "password_changed", "user", user, "")?;
    tx.commit()?;
    Ok(())
}

pub fn reset_password(db: &Connection, user: i64, new: &str) -> Result<()> {
    ensure(db.execute("UPDATE users SET password_hash=?1,must_change_password=1,failed_login_count=0,locked_until=NULL,password_changed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2",params![hash(new)?,user])?==1,"المستخدم غير موجود")?;
    Ok(())
}
