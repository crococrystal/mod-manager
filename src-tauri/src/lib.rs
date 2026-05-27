mod bootstrap;
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
mod provider_labels;
mod providers;
mod remote;
mod settings;
mod tags;
mod util;
mod window_chrome;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(bootstrap::BootstrapState::new())
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    use tauri::TitleBarStyle;
                    let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
                }

                #[cfg(any(windows, target_os = "linux"))]
                {
                    let _ = window.set_decorations(false);
                }

                window_chrome::attach(&window);
            }

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
            commands::identify_mod_sources,
            commands::refresh_mod_assets,
            commands::refresh_provider_labels,
            commands::bootstrap_instance,
            commands::cancel_background_task,
            commands::clear_app_data,
            commands::sync_provider_data,
            commands::get_data_usage,
            commands::update_mod_tags,
            commands::search_provider_candidates,
            commands::lookup_provider_fingerprint,
            commands::switch_mod_source,
            commands::list_provider_versions,
            commands::install_provider_version,
            commands::copy_mod_files,
            commands::upload_cover,
            commands::delete_custom_cover,
            commands::refresh_window_shadow
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
