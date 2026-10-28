//! Use the platform webview already shipped by Tauri; never spawn a browser or server.
use super::blocking;
use crate::{
    error::{Error, Result, ensure},
    models::{PrintPaper, PrintSource},
    state::App,
};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use tauri::{State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

static PDF_BUSY: AtomicBool = AtomicBool::new(false);
struct ExportGuard;
impl Drop for ExportGuard {
    fn drop(&mut self) {
        PDF_BUSY.store(false, Ordering::Release);
    }
}

fn validate_paper(p: &PrintPaper) -> Result<()> {
    ensure(
        matches!((p.width_mm, p.height_mm), (210, 297) | (297, 210))
            || (p.width_mm == 80 && (60..=800).contains(&p.height_mm)),
        "مقاس ورق الطباعة غير صالح",
    )
}
fn native_error() -> Error {
    Error::new(
        "printing",
        "تعذر إتمام الطباعة؛ تحقق من محرك العرض والطابعة ثم أعد المحاولة",
    )
}

#[tauri::command]
pub async fn erp_print(
    app: State<'_, Arc<App>>,
    window: WebviewWindow,
    source: PrintSource,
    paper: PrintPaper,
) -> Result<()> {
    ensure(window.label() == "main", "نافذة الطباعة غير مسموحة")?;
    validate_paper(&paper)?;
    let app = app.inner().clone();
    blocking(move || app.prepare_print(source, false)).await?;
    // Windows delegates to WebView2's print preview, including physical printers.
    // Linux uses WebKit's dialog with an explicit page setup for A4/80 mm.
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = mpsc::channel();
        window
            .with_webview(move |view| {
                use webkit2gtk::PrintOperationExt;
                let operation = webkit2gtk::PrintOperation::new(&view.inner());
                operation.set_page_setup(&linux_page(&paper));
                operation.run_dialog(None::<&gtk::Window>);
                let _ = tx.send(());
            })
            .map_err(|_| native_error())?;
        blocking(move || rx.recv().map_err(|_| native_error())).await
    }
    #[cfg(not(target_os = "linux"))]
    window.print().map_err(|_| native_error())
}

#[tauri::command]
pub async fn erp_export_pdf(
    app: State<'_, Arc<App>>,
    window: WebviewWindow,
    source: PrintSource,
    paper: PrintPaper,
) -> Result<Value> {
    ensure(window.label() == "main", "نافذة الطباعة غير مسموحة")?;
    validate_paper(&paper)?;
    ensure(
        !PDF_BUSY.swap(true, Ordering::AcqRel),
        "يوجد تصدير PDF جارٍ؛ انتظر اكتماله",
    )?;
    let _guard = ExportGuard;
    let app = app.inner().clone();
    let dialog_window = window.clone();
    let prepared = blocking(move || {
        app.prepare_print(source.clone(), true)?;
        let name = match source {
            PrintSource::Document { id } => format!("document-{}.pdf", id.0),
            PrintSource::Labels { .. } => "barcode-labels.pdf".into(),
            _ => "report.pdf".into(),
        };
        let selected = dialog_window
            .dialog()
            .file()
            .set_title("حفظ نسخة PDF")
            .add_filter("PDF", &["pdf"])
            .set_file_name(name)
            .blocking_save_file();
        let Some(path) = selected else {
            return Ok(None);
        };
        let path = path
            .into_path()
            .map_err(|_| Error::validation("اختر مساراً محلياً لملف PDF"))?;
        ensure(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf")),
            "يجب أن ينتهي اسم الملف بالامتداد pdf",
        )?;
        ensure(
            !path.exists(),
            "الملف موجود؛ اختر اسماً جديداً للحفاظ على النسخة السابقة",
        )?;
        let parent = path
            .parent()
            .ok_or_else(|| Error::validation("مسار الملف غير صالح"))?
            .canonicalize()?;
        ensure(
            !parent.starts_with(&app.root),
            "احفظ ملف PDF خارج مجلد قاعدة بيانات التطبيق",
        )?;
        let temp = tempfile::Builder::new()
            .prefix(".erp-print-")
            .suffix(".pdf")
            .tempfile_in(&parent)?
            .into_temp_path();
        Ok(Some((path, temp)))
    })
    .await?;
    let Some((path, temp)) = prepared else {
        return Ok(json!({"cancelled":true}));
    };
    let native_path = temp.to_path_buf();
    let (tx, rx) = mpsc::channel();
    window
        .with_webview(move |view| export_native(view, &native_path, &paper, tx))
        .map_err(|_| native_error())?;
    blocking(move || {
        let completed = match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(native_error()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // The engine may still own the output file. Keep the guard and temporary
                // path alive until its callback, even after returning control to the UI.
                std::thread::spawn(move || {
                    let _guard = _guard;
                    let _temp = temp;
                    let _ = rx.recv();
                });
                return Err(Error::new(
                    "printing_timeout",
                    "استغرق التصدير وقتاً طويلاً؛ أعد المحاولة بعد اكتمال العملية الحالية",
                ));
            }
        };
        let _guard = _guard;
        completed?;
        let mut file = std::fs::File::open(&temp)?;
        let mut magic = [0; 5];
        file.read_exact(&mut magic)?;
        ensure(&magic == b"%PDF-", "لم ينتج محرك العرض ملف PDF صالحاً")?;
        file.sync_all()?;
        drop(file);
        temp.persist_noclobber(&path)
            .map_err(|_| Error::new("file", "تعذر حفظ ملف PDF؛ تحقق من الاسم والمساحة والصلاحيات"))?;
        Ok(json!({"path":path.to_string_lossy()}))
    })
    .await
}

#[cfg(target_os = "linux")]
fn linux_page(paper: &PrintPaper) -> gtk::PageSetup {
    let page = gtk::PageSetup::new();
    page.set_paper_size(&gtk::PaperSize::new_custom(
        "erp",
        "ERP",
        f64::from(paper.width_mm),
        f64::from(paper.height_mm),
        gtk::Unit::Mm,
    ));
    page.set_top_margin(0., gtk::Unit::Mm);
    page.set_bottom_margin(0., gtk::Unit::Mm);
    page.set_left_margin(0., gtk::Unit::Mm);
    page.set_right_margin(0., gtk::Unit::Mm);
    page
}

#[cfg(target_os = "linux")]
fn export_native(
    view: tauri::webview::PlatformWebview,
    path: &Path,
    paper: &PrintPaper,
    tx: mpsc::Sender<Result<()>>,
) {
    export_webkit(&view.inner(), path, paper, tx);
}

/// Shared with the headless native-rendering check; not exposed over IPC.
#[cfg(target_os = "linux")]
#[doc(hidden)]
pub fn export_webkit(
    view: &webkit2gtk::WebView,
    path: &Path,
    paper: &PrintPaper,
    tx: mpsc::Sender<Result<()>>,
) {
    use webkit2gtk::PrintOperationExt;
    let Ok(uri) = tauri::Url::from_file_path(path) else {
        let _ = tx.send(Err(native_error()));
        return;
    };
    let operation = webkit2gtk::PrintOperation::new(view);
    operation.set_page_setup(&linux_page(paper));
    let settings = gtk::PrintSettings::new();
    settings.set_printer("Print to File");
    settings.set("output-file-format", Some("pdf"));
    settings.set("output-uri", Some(uri.as_str()));
    settings.set("print-backgrounds", Some("true"));
    settings.set_n_copies(1);
    operation.set_print_settings(&settings);
    let failed = std::rc::Rc::new(std::cell::Cell::new(false));
    let error_flag = failed.clone();
    operation.connect_failed(move |_, _| error_flag.set(true));
    operation.connect_finished(move |_| {
        let _ = tx.send(if failed.get() {
            Err(native_error())
        } else {
            Ok(())
        });
    });
    operation.print();
}

#[cfg(windows)]
fn export_native(
    view: tauri::webview::PlatformWebview,
    path: &Path,
    paper: &PrintPaper,
    tx: mpsc::Sender<Result<()>>,
) {
    use std::os::windows::ffi::OsStrExt;
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::{ICoreWebView2_7, ICoreWebView2Environment6},
        PrintToPdfCompletedHandler,
    };
    use windows::core::{Interface, PCWSTR};
    let on_complete = tx.clone();
    let run = || -> windows::core::Result<()> {
        unsafe {
            let settings = view
                .environment()
                .cast::<ICoreWebView2Environment6>()?
                .CreatePrintSettings()?;
            settings.SetPageWidth(f64::from(paper.width_mm) / 25.4)?;
            settings.SetPageHeight(f64::from(paper.height_mm) / 25.4)?;
            settings.SetMarginTop(0.)?;
            settings.SetMarginBottom(0.)?;
            settings.SetMarginLeft(0.)?;
            settings.SetMarginRight(0.)?;
            settings.SetScaleFactor(1.)?;
            settings.SetShouldPrintBackgrounds(true)?;
            settings.SetShouldPrintHeaderAndFooter(false)?;
            let file = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            view.controller()
                .CoreWebView2()?
                .cast::<ICoreWebView2_7>()?
                .PrintToPdf(
                    PCWSTR(file.as_ptr()),
                    &settings,
                    &PrintToPdfCompletedHandler::create(Box::new(move |status, success| {
                        let _ = on_complete.send(if status.is_ok() && success {
                            Ok(())
                        } else {
                            Err(native_error())
                        });
                        Ok(())
                    })),
                )?;
            Ok(())
        }
    };
    if run().is_err() {
        let _ = tx.send(Err(native_error()));
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn export_native(
    _: tauri::webview::PlatformWebview,
    _: &Path,
    _: &PrintPaper,
    tx: mpsc::Sender<Result<()>>,
) {
    let _ = tx.send(Err(Error::new(
        "platform",
        "تصدير PDF متاح على Windows وLinux",
    )));
}
