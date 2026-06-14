mod backend;
mod launch_script;
mod os;
pub(crate) mod readiness;
pub(crate) mod rcon;
pub(crate) mod start_config;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::loader_update::detect;
use crate::settings::{read_settings, Settings};

use self::backend::backend_for;
use self::os::{resolve_remote_os, RemoteOs};
use self::start_config::ServerStartConfig;

fn clean_host(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn resolve_ssh_host(settings: &Settings, override_host: Option<&str>) -> Option<String> {
    override_host
        .and_then(clean_host)
        .or_else(|| clean_host(&settings.server_sync.ssh_host))
}

struct ServerControlContext {
    host: String,
    server_root: String,
    remote_os: RemoteOs,
    start: ServerStartConfig,
}

fn resolve_server_root(settings: &Settings, override_path: Option<&str>) -> Result<String, String> {
    let raw = override_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let explicit = settings.server_sync.server_root_path.trim();
            if explicit.is_empty() {
                None
            } else {
                Some(explicit.to_string())
            }
        })
        .or_else(|| {
            let mods = settings.server_sync.server_mods_path.trim();
            if mods.is_empty() {
                None
            } else {
                Some(mods.to_string())
            }
        });
    detect::server_root_from_mods_path(raw.as_deref().unwrap_or(""))
        .ok_or_else(|| "Укажите корень сервера.".to_string())
}

fn resolve_context(
    settings: &Settings,
    ssh_override: Option<&str>,
    server_root_override: Option<&str>,
) -> Result<ServerControlContext, String> {
    let host = resolve_ssh_host(settings, ssh_override)
        .ok_or_else(|| "Укажите SSH host.".to_string())?;
    let server_root = resolve_server_root(settings, server_root_override)?;
    let remote_os = resolve_remote_os(&host, &settings.server_sync.server_os)?;
    let start = ServerStartConfig::from_settings(&settings.server_sync.server_start_script);
    Ok(ServerControlContext {
        host,
        server_root,
        remote_os,
        start,
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerControlStatusResult {
    pub ok: bool,
    pub running: bool,
    pub ready: bool,
    pub remote_os: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerControlActionResult {
    pub ok: bool,
    pub running: bool,
    pub ready: bool,
    pub remote_os: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerControlRequest {
    #[serde(default)]
    pub ssh_host: Option<String>,
    #[serde(default)]
    pub server_root_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchScriptWriteRequest {
    #[serde(default)]
    pub ssh_host: Option<String>,
    #[serde(default)]
    pub server_root_path: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchScriptReadResult {
    pub ok: bool,
    pub path: String,
    pub content: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchScriptWriteResult {
    pub ok: bool,
    pub path: String,
    pub message: String,
}

fn read_launch_script_inner(
    settings: &Settings,
    request: ServerControlRequest,
) -> Result<LaunchScriptReadResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    ctx.start.validate_for_start()?;
    let path = launch_script::script_remote_path(&ctx.server_root, &ctx.start.launch_script)?;
    let content = launch_script::read_launch_script(
        &ctx.host,
        ctx.remote_os,
        &ctx.server_root,
        &ctx.start.launch_script,
    )?;
    Ok(LaunchScriptReadResult {
        ok: true,
        path,
        content,
        message: "Скрипт загружен.".to_string(),
    })
}

fn write_launch_script_inner(
    settings: &Settings,
    request: LaunchScriptWriteRequest,
) -> Result<LaunchScriptWriteResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    ctx.start.validate_for_start()?;
    let path = launch_script::script_remote_path(&ctx.server_root, &ctx.start.launch_script)?;
    launch_script::write_launch_script(
        &ctx.host,
        ctx.remote_os,
        &ctx.server_root,
        &ctx.start.launch_script,
        &request.content,
    )?;
    Ok(LaunchScriptWriteResult {
        ok: true,
        path,
        message: "Скрипт сохранён.".to_string(),
    })
}

fn status_message(running: bool, ready: bool) -> String {
    match (running, ready) {
        (false, _) => "Сервер выключен.".to_string(),
        (true, true) => "Сервер запущен.".to_string(),
        (true, false) => "Сервер запускается.".to_string(),
    }
}

fn already_running_message(ready: bool) -> String {
    if ready {
        "Сервер уже запущен.".to_string()
    } else {
        "Сервер запускается.".to_string()
    }
}

fn resolve_runtime_status(
    backend: &dyn backend::RemoteServerBackend,
    host: &str,
    server_root: &str,
) -> Result<(bool, bool), String> {
    let running = backend.is_running(host, server_root)?;
    let ready = if running {
        backend.is_ready(host, server_root)?
    } else {
        false
    };
    Ok((running, ready))
}

fn check_inner(settings: &Settings, request: ServerControlRequest) -> Result<ServerControlStatusResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    let backend = backend_for(ctx.remote_os);
    backend.validate_server_root(&ctx.host, &ctx.server_root)?;
    let (running, ready) = resolve_runtime_status(backend.as_ref(), &ctx.host, &ctx.server_root)?;
    Ok(ServerControlStatusResult {
        ok: true,
        running,
        ready,
        remote_os: ctx.remote_os.as_str().to_string(),
        message: status_message(running, ready),
    })
}

fn start_inner(settings: &Settings, request: ServerControlRequest) -> Result<ServerControlActionResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    let backend = backend_for(ctx.remote_os);
    backend.validate_server_root(&ctx.host, &ctx.server_root)?;
    if backend.is_running(&ctx.host, &ctx.server_root)? {
        let ready = backend.is_ready(&ctx.host, &ctx.server_root)?;
        return Ok(ServerControlActionResult {
            ok: true,
            running: true,
            ready,
            remote_os: ctx.remote_os.as_str().to_string(),
            message: already_running_message(ready),
        });
    }
    ctx.start.validate_for_start()?;
    backend.start(&ctx.host, &ctx.server_root, &ctx.start)?;
    Ok(ServerControlActionResult {
        ok: true,
        running: false,
        ready: false,
        remote_os: ctx.remote_os.as_str().to_string(),
        message: "Инициализация сервера…".to_string(),
    })
}

fn stop_inner(settings: &Settings, request: ServerControlRequest) -> Result<ServerControlActionResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    let backend = backend_for(ctx.remote_os);
    backend.validate_server_root(&ctx.host, &ctx.server_root)?;
    if !backend.is_running(&ctx.host, &ctx.server_root)? {
        return Ok(ServerControlActionResult {
            ok: true,
            running: false,
            ready: false,
            remote_os: ctx.remote_os.as_str().to_string(),
            message: "Сервер уже выключен.".to_string(),
        });
    }
    if let Err(stop_error) = backend.stop(&ctx.host, &ctx.server_root, &ctx.start) {
        std::thread::sleep(std::time::Duration::from_millis(800));
        if backend.is_running(&ctx.host, &ctx.server_root)? {
            return Err(stop_error);
        }
    } else {
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
    let running = backend.is_running(&ctx.host, &ctx.server_root)?;
    Ok(ServerControlActionResult {
        ok: !running,
        running,
        ready: false,
        remote_os: ctx.remote_os.as_str().to_string(),
        message: if running {
            "Не удалось остановить сервер.".to_string()
        } else {
            "Сервер остановлен.".to_string()
        },
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RconCheckResult {
    pub ok: bool,
    pub port: Option<u16>,
    pub connect_host: Option<String>,
    pub ssh_alias: Option<String>,
    pub via_tunnel: bool,
    pub properties_path: Option<String>,
    pub detail: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RconCommandRequest {
    #[serde(default)]
    pub ssh_host: Option<String>,
    #[serde(default)]
    pub server_root_path: Option<String>,
    pub command: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RconCommandResult {
    pub ok: bool,
    pub output: String,
    pub message: String,
}

fn check_rcon_inner(
    settings: &Settings,
    request: ServerControlRequest,
) -> Result<RconCheckResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    let info = rcon::test_rcon(&ctx.host, &ctx.server_root, ctx.remote_os)?;
    Ok(RconCheckResult {
        ok: true,
        port: Some(info.port),
        connect_host: Some(info.connect_host),
        ssh_alias: Some(info.ssh_alias),
        via_tunnel: info.via_tunnel,
        properties_path: Some(info.properties_path),
        detail: info.detail,
        message: info.message,
    })
}

fn send_rcon_inner(settings: &Settings, request: RconCommandRequest) -> Result<RconCommandResult, String> {
    let ctx = resolve_context(
        settings,
        request.ssh_host.as_deref(),
        request.server_root_path.as_deref(),
    )?;
    let output = rcon::send_rcon_command(
        &ctx.host,
        &ctx.server_root,
        ctx.remote_os,
        &request.command,
    )?;
    Ok(RconCommandResult {
        ok: true,
        output: output.clone(),
        message: if output.is_empty() {
            "Команда выполнена.".to_string()
        } else {
            output
        },
    })
}

#[tauri::command]
pub(crate) async fn check_server_rcon(
    app: AppHandle,
    request: Option<ServerControlRequest>,
) -> Result<RconCheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        check_rcon_inner(&settings, request.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("{error}"))?
}

#[tauri::command]
pub(crate) async fn send_server_rcon_command(
    app: AppHandle,
    request: RconCommandRequest,
) -> Result<RconCommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        send_rcon_inner(&settings, request)
    })
    .await
    .map_err(|error| format!("{error}"))?
}

#[tauri::command]
pub(crate) async fn read_server_launch_script(
    app: AppHandle,
    request: Option<ServerControlRequest>,
) -> Result<LaunchScriptReadResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        read_launch_script_inner(&settings, request.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("Сервер: {error}"))?
}

#[tauri::command]
pub(crate) async fn write_server_launch_script(
    app: AppHandle,
    request: LaunchScriptWriteRequest,
) -> Result<LaunchScriptWriteResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        write_launch_script_inner(&settings, request)
    })
    .await
    .map_err(|error| format!("Сервер: {error}"))?
}

#[tauri::command]
pub(crate) async fn check_server_control_status(
    app: AppHandle,
    request: Option<ServerControlRequest>,
) -> Result<ServerControlStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        check_inner(&settings, request.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("Сервер: {error}"))?
}

#[tauri::command]
pub(crate) async fn start_server_control(
    app: AppHandle,
    request: Option<ServerControlRequest>,
) -> Result<ServerControlActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        start_inner(&settings, request.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("Сервер: {error}"))?
}

#[tauri::command]
pub(crate) async fn stop_server_control(
    app: AppHandle,
    request: Option<ServerControlRequest>,
) -> Result<ServerControlActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        stop_inner(&settings, request.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("Сервер: {error}"))?
}

impl Default for ServerControlRequest {
    fn default() -> Self {
        Self {
            ssh_host: None,
            server_root_path: None,
        }
    }
}
