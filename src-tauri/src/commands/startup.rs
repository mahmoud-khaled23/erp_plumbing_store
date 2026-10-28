//! Opt-in, local startup measurements. No telemetry or network access.
use crate::error::{Result, ensure};
use serde_json::json;
use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tauri::{State, WebviewWindow};

pub struct Startup {
    pub started: Instant,
    pub reported: AtomicBool,
}
impl Startup {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            reported: AtomicBool::new(false),
        }
    }
}

#[tauri::command]
pub async fn erp_frontend_ready(window: WebviewWindow, startup: State<'_, Startup>) -> Result<()> {
    ensure(window.label() == "main", "نافذة القياس غير مسموحة")?;
    if startup.reported.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let elapsed_ms = startup.started.elapsed().as_millis();
    // Only the launching OS process can choose a report file; IPC accepts no path.
    let Some(path) = std::env::var_os("PLUMBING_ERP_STARTUP_REPORT") else {
        return Ok(());
    };
    super::blocking(move || {
        let mut report = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let value = json!({"pid":std::process::id(),"version":env!("CARGO_PKG_VERSION"),
            "frontend_ready_ms":elapsed_ms,"measured_at":chrono::Utc::now().to_rfc3339()});
        report.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
        report.sync_all()?;
        Ok(())
    })
    .await
}

#[cfg(windows)]
pub fn show_startup_failure() {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::PCWSTR,
    };
    let labels: serde_json::Value = serde_json::from_str(include_str!("../../../src/i18n/ar.json"))
        .expect("validated Arabic dictionary");
    let wide = |key: &str| {
        labels[key]
            .as_str()
            .unwrap_or_default()
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let title = wide("app");
    let message = wide("startupFailure");
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
