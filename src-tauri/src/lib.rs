mod database;
mod document;
mod profile;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            profile::get_profile,
            profile::save_profile,
            document::hash_pdf,
            document::document_hash_exists,
            document::store_document_pdf
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let pool =
                tauri::async_runtime::block_on(database::initialize(&data_dir.join("app.db")))?;
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
