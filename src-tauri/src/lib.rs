mod catalog;
mod commands;
mod covers;
mod dependencies;
mod events;
mod file_identity;
mod instance_registry;
mod jar_deps;
mod mods;
mod mods_watch;
mod prefetch;
mod remote;
mod settings;
mod tags;
mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            commands::switch_mod_source,
            commands::upload_cover,
            commands::delete_custom_cover
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
