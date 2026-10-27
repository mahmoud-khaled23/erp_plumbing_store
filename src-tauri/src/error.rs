use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }
    pub fn validation(message: &str) -> Self {
        Self::new("validation", message)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::{Error as E, ErrorCode};
        match e {
            E::SqliteFailure(ref c, ref msg) if c.code == ErrorCode::ConstraintViolation => {
                let text = msg
                    .as_deref()
                    .filter(|s| s.chars().any(|c| ('\u{0600}'..='\u{06ff}').contains(&c)))
                    .unwrap_or("البيانات مكررة أو غير متوافقة مع قيود قاعدة البيانات");
                Self::new("constraint", text)
            }
            E::SqliteFailure(ref c, _)
                if matches!(c.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) =>
            {
                Self {
                    code: "busy".into(),
                    message: "قاعدة البيانات مشغولة؛ أعد المحاولة".into(),
                    retryable: true,
                }
            }
            E::QueryReturnedNoRows => Self::new("not_found", "السجل المطلوب غير موجود"),
            _ => Self::new("database", "تعذر إتمام عملية قاعدة البيانات"),
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::new(
            "file",
            "تعذر قراءة أو كتابة الملف؛ تحقق من المساحة والصلاحيات",
        )
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::validation("تنسيق البيانات غير صالح")
    }
}
pub fn ensure(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::validation(message))
    }
}
