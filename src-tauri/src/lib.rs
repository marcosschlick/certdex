mod database;
mod document;
pub mod document_analysis;
mod entities;
mod managed_file;
pub mod ollama;
pub mod pdf;
mod profile;
mod regex_extraction;
mod search;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            profile::get_profile,
            profile::save_profile,
            profile::save_profile_photo,
            profile::get_profile_photo,
            document::hash_document,
            document::document_hash_exists,
            document::store_document,
            document::list_documents,
            document::list_academic_options,
            document::update_document,
            document::delete_document,
            pdf::read_selected_document,
            ollama::check_ollama,
            ollama::analyze_pdf_text,
            document::open_document,
            document::reveal_document,
            search::search_documents_chat
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let pool =
                tauri::async_runtime::block_on(database::initialize(&data_dir.join("app.db")))?;
            if let Ok(home_dir) = app.path().home_dir()
                && let Ok(documents_dir) =
                    tauri::async_runtime::block_on(profile::load_documents_dir(&pool, &home_dir))
            {
                let legacy_dir = data_dir.join("documents");
                let _ = tauri::async_runtime::block_on(document::reorganize_documents(
                    &pool,
                    &documents_dir,
                    &[&legacy_dir],
                ));
            }
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
