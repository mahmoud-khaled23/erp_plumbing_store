use crate::{
    error::{Error, Result},
    models::*,
    state::App,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tauri::State;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
pub mod printing;
pub mod startup;

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| Error::new("worker", "تعذر إكمال العملية؛ أعد المحاولة"))?
}
#[tauri::command]
pub async fn erp_setup(app: State<'_, Arc<App>>, input: Setup) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.setup(input)).await
}
#[tauri::command]
pub async fn erp_login(
    app: State<'_, Arc<App>>,
    username: String,
    password: String,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.login(&username, &password)).await
}
#[tauri::command]
pub async fn erp_logout(app: State<'_, Arc<App>>) -> Result<()> {
    let app = app.inner().clone();
    blocking(move || app.logout()).await
}
#[tauri::command]
pub async fn erp_query(app: State<'_, Arc<App>>, query: Query) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.query(query)).await
}
#[tauri::command]
pub async fn erp_mutate(app: State<'_, Arc<App>>, request: Request) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.mutate(request)).await
}
#[tauri::command]
pub async fn erp_change_password(app: State<'_, Arc<App>>, old: String, new: String) -> Result<()> {
    let app = app.inner().clone();
    blocking(move || app.change_password(&old, &new)).await
}
#[tauri::command]
pub async fn erp_create_user(
    app: State<'_, Arc<App>>,
    username: String,
    name: String,
    password: String,
    roles: Vec<Int>,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.create_user(&username, &name, &password, &roles)).await
}
#[tauri::command]
pub async fn erp_reset_password(
    app: State<'_, Arc<App>>,
    user_id: Int,
    password: String,
) -> Result<()> {
    let app = app.inner().clone();
    blocking(move || app.reset_password(user_id, &password)).await
}
#[tauri::command]
pub async fn erp_recover(
    app: State<'_, Arc<App>>,
    handle: tauri::AppHandle,
    username: String,
    password: String,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || {
        let selected = handle
            .dialog()
            .file()
            .set_title("اختيار نسخة لاستعادة المتجر المتعطل")
            .add_filter("نسخة النظام", &["erpbackup"])
            .blocking_pick_file();
        let Some(path) = selected else {
            return Ok(json!({"cancelled":true}));
        };
        let path = path
            .into_path()
            .map_err(|_| Error::validation("اختر ملفاً محلياً"))?;
        app.recover_backup(&path, &username, &password)
    })
    .await
}
#[tauri::command]
pub async fn erp_import_apply(
    app: State<'_, Arc<App>>,
    token: String,
    replace: bool,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.import_products(&token, replace)).await
}
#[tauri::command]
pub async fn erp_import_opening_apply(
    app: State<'_, Arc<App>>,
    token: String,
    balances: bool,
    date: String,
    warehouse_id: Option<Int>,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move || app.import_opening(&token, balances, &date, warehouse_id)).await
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileAction {
    ExportProducts { excel: bool },
    ImportProducts,
    ImportOpening { balances: bool },
    Backup,
    BackupDirectory,
    Restore,
    Image { product_id: Option<Int> },
}
#[tauri::command]
pub async fn erp_file(
    app: State<'_, Arc<App>>,
    handle: tauri::AppHandle,
    action: FileAction,
) -> Result<Value> {
    let app = app.inner().clone();
    blocking(move||{
        {let mut inner=app.lock()?;let actor=App::actor(&mut inner)?;actor.require(match action{FileAction::ExportProducts{..}=>"products.read",FileAction::ImportProducts=>"products.import",FileAction::ImportOpening{balances}=>if balances{"finance.write"}else{"inventory.adjust"},FileAction::Backup=>"backup.create",FileAction::BackupDirectory=>"settings.write",FileAction::Restore=>"backup.restore",FileAction::Image{product_id}=>if product_id.is_some(){"products.write"}else{"settings.write"}})?;}
        let selected=match action{
            FileAction::ExportProducts{excel}=>handle.dialog().file().set_title("تصدير الأصناف").add_filter("ملف الأصناف",if excel{&["xlsx"]}else{&["csv"]}).set_file_name(if excel{"products.xlsx"}else{"products.csv"}).blocking_save_file(),
            FileAction::ImportProducts=>handle.dialog().file().set_title("معاينة استيراد الأصناف").add_filter("CSV وExcel",&["csv","xlsx","xls"]).blocking_pick_file(),
            FileAction::ImportOpening{..}=>handle.dialog().file().set_title("معاينة الأرصدة الافتتاحية").add_filter("CSV وExcel",&["csv","xlsx","xls"]).blocking_pick_file(),
            FileAction::BackupDirectory=>handle.dialog().file().set_title("مجلد النسخ الاحتياطي اليومي").blocking_pick_folder(),
            FileAction::Backup=>handle.dialog().file().set_title("إنشاء نسخة احتياطية").add_filter("نسخة النظام",&["erpbackup"]).set_file_name(format!("backup-{}.erpbackup",chrono::Local::now().format("%Y%m%d-%H%M%S"))).blocking_save_file(),
            FileAction::Restore=>handle.dialog().file().set_title("استعادة نسخة احتياطية").add_filter("نسخة النظام",&["erpbackup"]).blocking_pick_file(),
            FileAction::Image{..}=>handle.dialog().file().set_title("اختيار صورة").add_filter("صور",&["png","jpg","jpeg"]).blocking_pick_file(),
        };
        let Some(path)=selected else{return Ok(json!({"cancelled":true}));};let path=path.into_path().map_err(|_|Error::validation("اختر ملفاً محلياً"))?;
        match action{
            FileAction::ExportProducts{excel}=>app.export_products(&path,excel),FileAction::ImportProducts=>app.preview_products(&path),FileAction::Backup=>app.backup(&path),FileAction::Image{product_id}=>app.attach_image(&path,product_id),
            FileAction::ImportOpening{balances}=>app.preview_opening(&path,balances),FileAction::BackupDirectory=>app.set_backup_directory(&path),
            FileAction::Restore=>{if !handle.dialog().message("ستُستبدل بيانات المتجر بالنسخة المحددة مع حفظ نسخة من البيانات الحالية. هل تريد المتابعة؟").title("تأكيد الاستعادة").kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancel).blocking_show(){return Ok(json!({"cancelled":true}));}app.restore(&path)}
        }
    }).await
}
