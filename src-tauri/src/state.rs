use crate::{
    db,
    error::{Error, Result, ensure},
    models::*,
    services,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Actor {
    pub user_id: i64,
    pub permissions: BTreeSet<String>,
    pub must_change_password: bool,
}
impl Actor {
    pub fn require(&self, permission: &str) -> Result<()> {
        ensure(!self.must_change_password, "يجب تغيير كلمة المرور أولاً")?;
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(Error::new("forbidden", "ليس لديك صلاحية لتنفيذ هذه العملية"))
        }
    }
}
pub(crate) struct Session {
    pub user_id: i64,
    pub hash: String,
    pub last_used: Instant,
}
pub(crate) struct Inner {
    pub db: Connection,
    pub session: Option<Session>,
    pub import_grants: std::collections::BTreeMap<String, (PathBuf, String)>,
    pub recovery_error: Option<String>,
}
pub struct App {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) root: PathBuf,
}
impl App {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent)?;
        }
        services::files::recover_interrupted_replace(&root)?;
        std::fs::create_dir_all(root.join("assets"))?;
        let root = root.canonicalize()?;
        let db = db::open(&root.join("store.db"))?;
        Ok(Self {
            inner: Mutex::new(Inner {
                db,
                session: None,
                import_grants: Default::default(),
                recovery_error: None,
            }),
            root,
        })
    }
    /// Preserve a damaged store on disk and expose only recovery status to the desktop UI.
    pub fn open_with_recovery(root: impl AsRef<Path>) -> Result<Self> {
        match Self::open(root.as_ref()) {
            Ok(app) => Ok(app),
            Err(error) => {
                let root = root.as_ref().canonicalize()?;
                Ok(Self {
                    root,
                    inner: Mutex::new(Inner {
                        db: Connection::open_in_memory()?,
                        session: None,
                        import_grants: Default::default(),
                        recovery_error: Some(error.message),
                    }),
                })
            }
        }
    }
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Inner>> {
        self.inner
            .lock()
            .map_err(|_| Error::new("internal", "تعذر الوصول إلى جلسة التطبيق؛ أعد تشغيله"))
    }
    pub(crate) fn actor(inner: &mut Inner) -> Result<Actor> {
        let session = inner
            .session
            .as_mut()
            .ok_or_else(|| Error::new("unauthenticated", "يرجى تسجيل الدخول"))?;
        if session.last_used.elapsed() >= Duration::from_secs(30 * 60) {
            inner.session = None;
            return Err(Error::new(
                "unauthenticated",
                "انتهت الجلسة؛ يرجى تسجيل الدخول",
            ));
        }
        let valid: bool = inner.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND active=1 AND password_hash=?2)",
            params![session.user_id, session.hash],
            |r| r.get(0),
        )?;
        if !valid {
            inner.session = None;
            return Err(Error::new(
                "unauthenticated",
                "انتهت صلاحية الجلسة؛ يرجى تسجيل الدخول",
            ));
        }
        session.last_used = Instant::now();
        let permissions = inner.db.prepare("SELECT DISTINCT rp.permission_code FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=?1")?
            .query_map([session.user_id], |r|r.get(0))?.collect::<rusqlite::Result<BTreeSet<_>>>()?;
        let must_change_password = inner.db.query_row(
            "SELECT must_change_password FROM users WHERE id=?1",
            [session.user_id],
            |r| r.get(0),
        )?;
        Ok(Actor {
            user_id: session.user_id,
            permissions,
            must_change_password,
        })
    }
    pub fn setup(&self, input: Setup) -> Result<Value> {
        let mut inner = self.lock()?;
        ensure(
            inner.recovery_error.is_none(),
            "لا يمكن إعداد متجر جديد فوق قاعدة تحتاج إلى استعادة",
        )?;
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        services::auth::setup(&tx, &input)?;
        tx.commit()?;
        Ok(json!({"ready": true}))
    }
    pub fn login(&self, username: &str, password: &str) -> Result<Value> {
        let mut inner = self.lock()?;
        ensure(
            inner.recovery_error.is_none(),
            "يلزم استعادة قاعدة البيانات أولاً",
        )?;
        inner.session = None;
        inner.import_grants.clear();
        let (id, hash, name) = services::auth::login(&mut inner.db, username, password)?;
        inner.session = Some(Session {
            user_id: id,
            hash,
            last_used: Instant::now(),
        });
        let actor = Self::actor(&mut inner)?;
        Ok(json!({"id": Int(id), "name":name, "permissions":actor.permissions}))
    }
    pub fn logout(&self) -> Result<()> {
        let mut inner = self.lock()?;
        inner.session = None;
        inner.import_grants.clear();
        Ok(())
    }
    pub fn change_password(&self, old: &str, new: &str) -> Result<()> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        services::auth::change_password(&mut inner.db, actor.user_id, old, new)?;
        inner.session = None;
        Ok(())
    }
    pub fn create_user(
        &self,
        username: &str,
        name: &str,
        password: &str,
        roles: &[Int],
    ) -> Result<Value> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("users.manage")?;
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let id = services::auth::create_user(&tx, username, name, password, roles)?;
        db::audit(&tx, actor.user_id, "create_user", "user", id, "")?;
        tx.commit()?;
        Ok(json!({"id":Int(id)}))
    }
    pub fn reset_password(&self, user: Int, password: &str) -> Result<()> {
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require("users.manage")?;
        ensure(
            actor.user_id != user.0,
            "استخدم تغيير كلمة المرور لحسابك الحالي",
        )?;
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        services::auth::reset_password(&tx, user.0, password)?;
        db::audit(&tx, actor.user_id, "password_reset", "user", user.0, "")?;
        tx.commit()?;
        Ok(())
    }
    pub fn mutate(&self, request: Request) -> Result<Value> {
        ensure(
            uuid::Uuid::parse_str(&request.request_id).is_ok(),
            "معرّف الطلب غير صالح",
        )?;
        let mut inner = self.lock()?;
        let actor = Self::actor(&mut inner)?;
        actor.require(services::permission(&request.mutation))?;
        ensure(
            !inner.db.query_row(
                "SELECT must_change_password FROM users WHERE id=?1",
                [actor.user_id],
                |r| r.get::<_, bool>(0),
            )?,
            "يجب تغيير كلمة المرور أولاً",
        )?;
        let encoded = serde_json::to_string(&request.mutation)?;
        let hash = db::digest(encoded.as_bytes());
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior: Option<(String,i64,String)> = tx.query_row("SELECT payload_sha256,user_id,result_json FROM command_receipts WHERE request_id=?1",[&request.request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((prior_hash, user, result)) = prior {
            ensure(
                prior_hash == hash && user == actor.user_id,
                "معرّف الطلب مستخدم لعملية أخرى",
            )?;
            return Ok(serde_json::from_str(&result)?);
        }
        let result = services::mutate(&tx, &actor, &request.mutation)?;
        ensure(
            tx.query_row(
                "SELECT COUNT(*) FROM documents WHERE status='posting'",
                [],
                |r| r.get::<_, i64>(0),
            )? == 0,
            "لم تكتمل عملية الترحيل",
        )?;
        let command = serde_json::to_value(&request.mutation)?["type"]
            .as_str()
            .unwrap_or("mutation")
            .to_owned();
        tx.execute("INSERT INTO audit_log(user_id,action,entity_type,entity_id,after_json) VALUES (?1,?2,'command',?3,?4)",params![actor.user_id,command,request.request_id,encoded])?;
        tx.execute("INSERT INTO command_receipts(request_id,command,payload_sha256,user_id,result_json) VALUES (?1,?2,?3,?4,?5)",params![request.request_id,command,hash,actor.user_id,serde_json::to_string(&result)?])?;
        tx.commit()?;
        Ok(result)
    }
    pub fn query(&self, query: Query) -> Result<Value> {
        let mut inner = self.lock()?;
        if matches!(query, Query::Bootstrap) {
            if let Some(message) = &inner.recovery_error {
                return Ok(
                    json!({"authenticated":false,"setup_required":false,"recovery_required":true,"message":message}),
                );
            }
        }
        if matches!(query, Query::Bootstrap) && inner.session.is_none() {
            return Ok(
                json!({"setup_required": inner.db.query_row("SELECT setup_completed_at IS NULL FROM store_settings",[],|r|r.get::<_,bool>(0))?,"authenticated":false}),
            );
        }
        let actor = Self::actor(&mut inner)?;
        let tx = inner.db.transaction()?;
        let result = services::reporting::query(&tx, &actor, &query)?;
        tx.commit()?;
        Ok(result)
    }
}
