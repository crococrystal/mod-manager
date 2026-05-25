mod catalog;
mod commands;
mod covers;
mod dependencies;
mod events;
mod instance_registry;
mod jar_deps;
mod mods;
mod remote;
mod settings;
mod tags;
mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::scan_mods,
            commands::bootstrap_instance,
            commands::clear_app_data,
            commands::update_mod_tags,
            commands::upload_cover
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
