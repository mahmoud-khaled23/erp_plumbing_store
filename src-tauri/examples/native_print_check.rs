//! Test-only: render the actual exported React print markup using WebKitGTK's PDF adapter.
#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use plumbing_erp::{commands::printing::export_webkit, models::PrintPaper};
    use std::{cell::Cell, rc::Rc, sync::mpsc, time::Duration};
    use webkit2gtk::{LoadEvent, WebView, WebViewExt};
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(
        args.len(),
        5,
        "native_print_check INPUT.html OUTPUT.pdf WIDTH_MM HEIGHT_MM"
    );
    gtk::init().expect("GTK display");
    let html = std::fs::read_to_string(&args[1]).expect("HTML fixture");
    let path = std::path::PathBuf::from(&args[2]);
    let paper = PrintPaper {
        width_mm: args[3].parse().unwrap(),
        height_mm: args[4].parse().unwrap(),
    };
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_default_size(1280, 900);
    let webview = WebView::new();
    window.add(&webview);
    window.show_all();
    let (tx, rx) = mpsc::channel();
    let started = Rc::new(Cell::new(false));
    webview.connect_load_changed(|view, event| {
        if event == LoadEvent::Finished {
            #[allow(deprecated)]
            view.run_javascript(
                "document.fonts.ready.then(()=>{document.title='ERP_PRINT_READY'})",
                None::<&gtk::gio::Cancellable>,
                |_| {},
            );
        }
    });
    webview.connect_title_notify(move |view| {
        if view.title().as_deref() == Some("ERP_PRINT_READY") && !started.replace(true) {
            export_webkit(view, &path, &paper, tx.clone());
        }
    });
    let success = Rc::new(Cell::new(false));
    let result = success.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
        if let Ok(value) = rx.try_recv() {
            result.set(value.is_ok());
            if let Err(e) = value {
                eprintln!("{e}");
            }
            gtk::main_quit();
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
    gtk::glib::timeout_add_local_once(Duration::from_secs(90), || {
        eprintln!("Native PDF timed out");
        gtk::main_quit();
    });
    webview.load_html(&html, None);
    gtk::main();
    assert!(success.get(), "Native PDF export failed");
    assert!(std::fs::read(&args[2]).unwrap().starts_with(b"%PDF-"));
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This rendering check runs on Linux; Windows uses WebView2.");
}
