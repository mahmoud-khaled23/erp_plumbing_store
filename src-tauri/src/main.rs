#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use plumbing_erp::{commands::*, state::App};
use std::sync::Arc;
use tauri::Manager;

fn main() {
    let started = std::time::Instant::now();
    let result = tauri::Builder::default()
        .manage(startup::Startup::new(started))
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_local_data_dir()?.join("store");
            let state = Arc::new(App::open_with_recovery(root)?);
            app.manage(state.clone());
            let weak = Arc::downgrade(&state);
            std::thread::spawn(move || {
                loop {
                    let Some(state) = weak.upgrade() else {
                        break;
                    };
                    let _ = state.daily_backup();
                    drop(state);
                    std::thread::sleep(std::time::Duration::from_secs(60));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            erp_setup,
            erp_login,
            erp_logout,
            erp_query,
            erp_mutate,
            erp_change_password,
            erp_create_user,
            erp_reset_password,
            erp_recover,
            erp_file,
            erp_import_apply,
            erp_import_opening_apply,
            printing::erp_print,
            printing::erp_export_pdf,
            startup::erp_frontend_ready
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("تعذر بدء التطبيق: {error}");
        #[cfg(windows)]
        startup::show_startup_failure();
        std::process::exit(1);
    }
}
