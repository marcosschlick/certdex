mod database;
mod document;
pub mod document_analysis;
mod entities;
mod managed_file;
pub mod ollama;
pub mod pdf;
mod profile;
mod regex_extraction;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            profile::get_profile,
            profile::save_profile,
            profile::save_profile_photo,
            profile::get_profile_photo,
            document::hash_pdf,
            document::document_hash_exists,
            document::store_document_pdf,
            document::list_documents,
            document::list_academic_options,
            document::update_document,
            pdf::read_selected_pdf,
            ollama::check_ollama,
            ollama::analyze_pdf_text,
            document::open_document_pdf
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
