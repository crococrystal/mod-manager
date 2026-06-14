use tauri::AppHandle;

use crate::{
    catalog,
    mods::scan_mods_for_settings,
    settings::{read_settings, resolve_paths},
};

use super::{
    config::{
        ensure_sync_side_labels, mod_side_for_key, resolve_ssh_host, sync_config, sync_upload_side,
    },
    remote::{delete_remote_jar, disable_remote_mod, enable_remote_mod, ssh_command, upload_mod},
    ServerSyncTestResult,
};

pub(crate) fn test_connection(
    settings: &crate::settings::Settings,
    ssh_host: Option<&str>,
) -> ServerSyncTestResult {
    let Some(host) = resolve_ssh_host(settings, ssh_host) else {
        return ServerSyncTestResult {
            ok: false,
            message: "Укажите SSH host.".to_string(),
        };
    };

    if crate::ssh_util::ssh_config_hostname(&host).is_none() {
        return ServerSyncTestResult {
            ok: false,
            message: format!("«{host}» не в ~/.ssh/config."),
        };
    }

    match ssh_command(&host, "echo connected") {
        Ok(output) if output.status.success() => ServerSyncTestResult {
            ok: true,
            message: format!("«{host}» подключён."),
        },
        Ok(output) => ServerSyncTestResult {
            ok: false,
            message: crate::ssh_util::ssh_command_failed(&host, &output),
        },
        Err(error) => ServerSyncTestResult {
            ok: false,
            message: error,
        },
    }
}

pub(crate) fn sync_mod_file(
    app: &AppHandle,
    key: &str,
    filename: &str,
    previous_filename: Option<&str>,
) -> Result<(), String> {
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Ok(());
    };
    let paths = resolve_paths(&settings)?;
    ensure_sync_side_labels(app, &paths, &settings, key);
    let local_path = paths
        .resolve_mod_jar(filename)
        .ok_or_else(|| format!("Нет файла: {filename}"))?;
    let side = mod_side_for_key(&paths, key);
    let upload_side = sync_upload_side(&paths, key, &side);
    upload_mod(
        &config,
        &local_path,
        filename,
        upload_side,
        previous_filename,
        None,
        None,
    )?;
    Ok(())
}

pub(crate) fn delete_mod_file_from_server(
    app: &AppHandle,
    filename: &str,
    side: &str,
) -> Result<(), String> {
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Ok(());
    };
    delete_remote_jar(&config, side, filename)
}

pub(crate) fn schedule_delete_mod(app: &AppHandle, filename: String, side: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            delete_mod_file_from_server(&app, &filename, &side)
        })
        .await;
    });
}

pub(crate) fn disable_mod_file_on_server(app: &AppHandle, filename: &str) -> Result<(), String> {
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Ok(());
    };
    disable_remote_mod(&config, filename)
}

pub(crate) fn schedule_disable_mod(app: &AppHandle, filename: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            disable_mod_file_on_server(&app, &filename)
        })
        .await;
    });
}

pub(crate) fn enable_mod_file_on_server(app: &AppHandle, filename: &str) -> Result<(), String> {
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Ok(());
    };
    enable_remote_mod(&config, filename)
}

pub(crate) fn schedule_enable_mod(app: &AppHandle, filename: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            enable_mod_file_on_server(&app, &filename)
        })
        .await;
    });
}

pub(crate) fn schedule_sync_mod(
    app: &AppHandle,
    key: &str,
    filename: &str,
    previous_filename: Option<String>,
) {
    let app = app.clone();
    let key = key.to_string();
    let filename = filename.to_string();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            sync_mod_file(
                &app,
                &key,
                &filename,
                previous_filename.as_deref(),
            )
        })
        .await;
    });
}

pub(crate) fn find_mod_for_sync_key<'a>(
    mods: &'a [crate::mods::ModEntry],
    key: &str,
) -> Option<&'a crate::mods::ModEntry> {
    if let Some(entry) = mods.iter().find(|item| item.key == key) {
        return Some(entry);
    }
    if let Some(project_id) = key.strip_prefix("modrinth:") {
        if let Some(entry) = mods
            .iter()
            .find(|item| item.modrinth_id.as_deref() == Some(project_id))
        {
            return Some(entry);
        }
    }
    if let Some(project_id) = key.strip_prefix("curseforge:") {
        if let Some(entry) = mods
            .iter()
            .find(|item| item.curseforge_id.as_deref() == Some(project_id))
        {
            return Some(entry);
        }
    }
    None
}

pub(crate) fn schedule_sync_keys(app: &AppHandle, keys: &[String]) {
    let app = app.clone();
    let keys = keys.to_vec();
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let settings = read_settings(&app)?;
            if sync_config(&settings).is_none() {
                return Ok(());
            }
            let catalog_root = catalog::catalog_root(&app).ok();
            let mods = scan_mods_for_settings(&settings, catalog_root)?;
            for key in keys {
                let Some(entry) = find_mod_for_sync_key(&mods, &key) else {
                    continue;
                };
                let _ = sync_mod_file(&app, &entry.key, &entry.filename, None);
            }
            Ok::<(), String>(())
        })
        .await;
    });
}
