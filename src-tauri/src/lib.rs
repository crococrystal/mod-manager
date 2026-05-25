mod catalog;
mod commands;
mod covers;
mod dependencies;
mod events;
mod file_identity;
mod instance_meta;
mod instance_registry;
mod jar_deps;
mod mod_names;
mod mods;
mod mods_watch;
mod prefetch;
mod providers;
mod remote;
mod settings;
mod tags;
mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            if let Ok(settings) = settings::read_settings(&handle) {
                if let Ok(paths) = settings::resolve_paths(&settings) {
                    mods_watch::sync_mods_watch(&handle, Some(paths.mods_dir));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::scan_mods,
            commands::bootstrap_instance,
            commands::clear_app_data,
            commands::update_mod_tags,
            commands::search_provider_candidates,
            commands::lookup_provider_fingerprint,
            commands::switch_mod_source,
            commands::list_provider_versions,
            commands::install_provider_version,
            commands::copy_mod_files,
            commands::upload_cover,
            commands::delete_custom_cover
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
