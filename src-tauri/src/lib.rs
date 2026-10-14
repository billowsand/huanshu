mod commands;
mod fonts;
mod render;
mod text;

use huanshu_core::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(AppState::load()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|_app| {
            // Scanning system fonts takes ~0.5 s; warm the shared renderer in the background
            // so the first thumbnail or export does not pay for it.
            std::thread::spawn(|| {
                huanshu_core::services::export::renderer();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            text::segment_title,
            fonts::list_system_fonts,
            commands::generate::generate_slides,
            commands::generate::detect_granularity,
            commands::generate::optimize_markdown_headings,
            commands::generate::get_blueprints,
            commands::generate::get_generation_status,
            commands::generate::get_latest_generation_run,
            commands::generate::get_generation_logs,
            commands::generate::repair_slide,
            commands::generate::retry_generation_page,
            commands::generate::get_retry_layouts,
            commands::generate::ensure_icon_embeddings,
            commands::generate::recommend_icons_for_query,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::list_models,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project_content,
            commands::projects::update_project_blueprints,
            commands::projects::delete_project,
            commands::projects::open_project,
            commands::projects::set_active_project,
            commands::projects::update_project_aspect_ratio,
            commands::projects::get_project_aspect_ratio,
            commands::storage::get_app_settings,
            commands::storage::save_app_settings,
            commands::storage::is_first_run,
            commands::storage::complete_first_run,
            commands::media::import_media_asset,
            commands::media::delete_media_asset,
            commands::media::get_project_media,
            commands::media::update_media_caption,
            commands::storage::export_project,
            commands::pdf::export_pdf,
            commands::pdf::reveal_exported_file,
            commands::thumbnails::render_slide_thumbnails,
            commands::storage::import_project,
            commands::storage::is_encrypted_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
