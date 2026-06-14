use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::{
    catalog,
    mod_names::{normalized_match_key, strip_filename_decorations, strip_version_suffixes},
    mods::scan_mods_for_settings,
    provider_labels::resolve_side,
    settings::{read_settings, resolve_paths, ServerSyncSettings, Settings},
    tags::read_tags,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncBulkResult {
    pub uploaded: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncUpdatePair {
    pub remote: String,
    pub local: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncDeleteItem {
    pub filename: String,
    pub side: String,
    #[serde(default)]
    pub library: bool,
    #[serde(default)]
    pub technical: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncLanePreview {
    pub ok: bool,
    pub local: u32,
    pub remote: u32,
    pub to_upload: u32,
    pub already_synced: u32,
    pub to_delete: u32,
    #[serde(default)]
    pub to_update: u32,
    #[serde(default)]
    pub to_upload_names: Vec<String>,
    #[serde(default)]
    pub to_update_pairs: Vec<ServerSyncUpdatePair>,
    #[serde(default)]
    pub to_delete_names: Vec<String>,
    #[serde(default)]
    pub to_delete_items: Vec<ServerSyncDeleteItem>,
    pub errors: Vec<String>,
}

struct LaneSyncAnalysis {
    config: ServerSyncSettings,
    mods: Vec<crate::mods::ModEntry>,
    pending: Vec<(crate::mods::ModEntry, std::path::PathBuf)>,
    already_synced: u32,
    total_all: u32,
    remote_count: u32,
    to_delete: u32,
    remote_index: RemoteDirIndex,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncProgress {
    #[serde(default)]
    pub target: String,
    pub active: bool,
    #[serde(default)]
    pub phase: String,
    pub current: u32,
    pub total: u32,
    #[serde(default)]
    pub total_all: u32,
    #[serde(default)]
    pub already_synced: u32,
    pub filename: String,
    pub uploaded: u32,
    pub skipped: u32,
    pub deleted: u32,
    #[serde(default)]
    pub deleted_extra: u32,
    #[serde(default)]
    pub replaced_remote: u32,
    pub errors: Vec<String>,
    pub done: bool,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncStartResult {
    pub started: bool,
    #[serde(default)]
    pub already_running: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyncLane {
    Server,
    Distribution,
}

impl SyncLane {
    fn as_str(self) -> &'static str {
        match self {
            SyncLane::Server => "server",
            SyncLane::Distribution => "distribution",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "server" => Some(SyncLane::Server),
            "distribution" => Some(SyncLane::Distribution),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ServerSyncState {
    lane: SyncLane,
    pub(crate) inner: Arc<Mutex<ServerSyncProgress>>,
    pub(crate) running: Arc<Mutex<bool>>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct ServerSyncLanes {
    pub server: ServerSyncState,
    pub distribution: ServerSyncState,
}

impl Default for ServerSyncLanes {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerSyncLanes {
    pub(crate) fn new() -> Self {
        Self {
            server: ServerSyncState::new(SyncLane::Server),
            distribution: ServerSyncState::new(SyncLane::Distribution),
        }
    }

    pub(crate) fn get(&self, lane: &str) -> Option<&ServerSyncState> {
        SyncLane::from_str(lane).map(|value| self.get_lane(value))
    }

    pub(crate) fn get_lane(&self, lane: SyncLane) -> &ServerSyncState {
        match lane {
            SyncLane::Server => &self.server,
            SyncLane::Distribution => &self.distribution,
        }
    }
}

impl ServerSyncState {
    pub(crate) fn new(lane: SyncLane) -> Self {
        Self {
            lane,
            inner: Arc::new(Mutex::new(ServerSyncProgress::default())),
            running: Arc::new(Mutex::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn progress_with_target(&self) -> ServerSyncProgress {
        let mut progress = self.snapshot();
        progress.target = self.lane.as_str().to_string();
        progress
    }

    pub(crate) fn snapshot(&self) -> ServerSyncProgress {
        self.inner.lock().map(|value| value.clone()).unwrap_or_default()
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.lock().map(|value| *value).unwrap_or(false)
    }

    pub(crate) fn try_acquire(&self) -> bool {
        let mut running = match self.running.lock() {
            Ok(value) => value,
            Err(_) => return false,
        };
        if *running {
            return false;
        }
        *running = true;
        self.cancelled.store(false, Ordering::Relaxed);
        true
    }

    pub(crate) fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn set_checking(&self, total_all: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress {
                active: true,
                phase: "checking".to_string(),
                total_all,
                ..ServerSyncProgress::default()
            };
        }
    }

    fn begin_upload(&self, total: u32, already_synced: u32, total_all: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress {
                active: true,
                phase: "uploading".to_string(),
                total,
                total_all,
                already_synced,
                skipped: already_synced,
                ..ServerSyncProgress::default()
            };
        }
    }

    fn set_pruning(&self, to_delete: u32, already_synced: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.active = true;
            progress.phase = "pruning".to_string();
            progress.total = to_delete;
            progress.current = 0;
            progress.already_synced = already_synced;
            progress.skipped = already_synced;
            progress.done = false;
            progress.ok = false;
            progress.filename.clear();
        }
    }

    fn set_step(&self, current: u32, total: u32, filename: &str) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.active = true;
            progress.phase = "uploading".to_string();
            progress.current = current;
            progress.total = total;
            progress.filename = filename.to_string();
            progress.done = false;
            progress.ok = false;
        }
    }

    fn add_result(&self, uploaded: bool, skipped: bool, error: Option<String>) {
        if let Ok(mut progress) = self.inner.lock() {
            if uploaded {
                progress.uploaded += 1;
            }
            if skipped {
                progress.skipped += 1;
            }
            if let Some(message) = error {
                progress.errors.push(message);
            }
        }
    }

    fn set_deleted(&self, deleted: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.deleted = deleted;
        }
    }

    fn set_deleted_breakdown(&self, deleted: u32, deleted_extra: u32, replaced_remote: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.deleted = deleted;
            progress.deleted_extra = deleted_extra;
            progress.replaced_remote = replaced_remote;
        }
    }

    fn reset(&self) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress::default();
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }

    fn finish(&self, ok: bool) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.active = false;
            progress.done = true;
            progress.ok = ok;
            progress.phase.clear();
            progress.filename.clear();
            progress.current = 0;
            progress.total = 0;
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }

    fn fail_start(&self, message: String) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress {
                active: false,
                done: true,
                ok: false,
                errors: vec![message],
                ..ServerSyncProgress::default()
            };
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }
}

fn emit_server_sync_progress(app: &AppHandle, state: &ServerSyncState) {
    let _ = app.emit("server-sync-progress", state.progress_with_target());
}

fn clean(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_remote_dir(value: &str) -> String {
    let mut trimmed = value.trim();
    while trimmed.len() >= 2 {
        let starts = trimmed.starts_with('"') || trimmed.starts_with('\'');
        let ends = trimmed.ends_with('"') || trimmed.ends_with('\'');
        if starts && ends {
            trimmed = trimmed[1..trimmed.len() - 1].trim();
        } else {
            break;
        }
    }
    trimmed.replace('\\', "/")
}

fn normalize_remote_path(path: &str) -> String {
    normalize_remote_dir(path)
}

fn clean_remote_dir(value: &str) -> Option<String> {
    let normalized = normalize_remote_dir(value);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn join_remote_path(dir: &str, filename: &str) -> String {
    let base = normalize_remote_path(dir).trim_end_matches('/').to_string();
    format!("{base}/{filename}")
}

fn short_msg(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..max.saturating_sub(1)])
    }
}

fn ssh_control_path(host: &str) -> std::path::PathBuf {
    let safe: String = host
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("mod-manager-ssh-{safe}.sock"))
}

fn ssh_command(host: &str, remote_command: &str) -> Result<std::process::Output, String> {
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=15",
        ])
        .arg(host)
        .arg(remote_command)
        .output()
        .map_err(|error| format!("ssh: {error}"))
}

fn scp_upload(host: &str, local_path: &Path, remote_file: &str) -> Result<(), String> {
    let remote = format!("{host}:{}", normalize_remote_path(remote_file));
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    let output = Command::new("scp")
        .args([
            "-q",
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=60",
        ])
        .arg(local_path)
        .arg(remote)
        .output()
        .map_err(|error| format!("scp: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("No such file or directory") {
        return Err("Папка не найдена.".to_string());
    }
    if stderr.is_empty() {
        Err("Ошибка scp.".to_string())
    } else {
        Err(short_msg(&stderr, 48))
    }
}

fn sync_config(settings: &Settings) -> Option<ServerSyncSettings> {
    let config = settings.server_sync.clone();
    if !config.enabled {
        return None;
    }
    let host = clean(&config.ssh_host)?.to_ascii_lowercase();
    if config.server_mods_path.trim().is_empty() && config.distribution_mods_path.trim().is_empty() {
        return None;
    }
    Some(ServerSyncSettings {
        enabled: true,
        ssh_host: host,
        server_mods_path: normalize_remote_dir(&config.server_mods_path),
        distribution_mods_path: normalize_remote_dir(&config.distribution_mods_path),
        delete_extra_remote_jars: config.delete_extra_remote_jars,
    })
}

fn sync_config_error(settings: &Settings) -> String {
    let config = &settings.server_sync;
    if !config.enabled {
        return "Включите синхронизацию.".to_string();
    }
    if clean(&config.ssh_host).is_none() {
        return "Укажите SSH host.".to_string();
    }
    if config.server_mods_path.trim().is_empty() && config.distribution_mods_path.trim().is_empty() {
        return "Укажите путь mods.".to_string();
    }
    "Синхронизация не настроена.".to_string()
}

fn resolve_ssh_host(settings: &Settings, override_host: Option<&str>) -> Option<String> {
    override_host
        .and_then(clean)
        .or_else(|| clean(&settings.server_sync.ssh_host))
        .map(|host| host.to_ascii_lowercase())
}

fn ssh_config_hostname(host: &str) -> Option<String> {
    let output = Command::new("ssh")
        .args(["-G", host])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next()? != "hostname" {
            continue;
        }
        let hostname = parts.next()?.trim();
        if hostname.is_empty() || hostname.eq_ignore_ascii_case(host) {
            return None;
        }
        return Some(hostname.to_string());
    }
    None
}

fn explain_ssh_error(host: &str, stderr: &str) -> String {
    if stderr.contains("Could not resolve hostname") {
        return format!("«{host}» не в ~/.ssh/config.");
    }
    if stderr.contains("Permission denied") {
        return format!("SSH отказал: «{host}».");
    }
    if stderr.contains("Connection refused") || stderr.contains("Operation timed out") {
        return format!("Нет связи с «{host}».");
    }
    let trimmed = stderr.trim();
    short_msg(trimmed, 48)
}

fn mod_side(paths: &crate::settings::InstancePaths, key: &str) -> String {
    read_tags(&paths.tags_path)
        .ok()
        .and_then(|tags| tags.mods.get(key).cloned())
        .map(|tag| resolve_side(&tag))
        .unwrap_or_else(|| "universal".to_string())
}

fn powershell_literal(path: &str) -> String {
    normalize_remote_path(path).replace('\'', "''")
}

struct RemoteDirIndex {
    files: HashMap<String, u64>,
}

fn format_index_dir_error(host: &str, remote_dir: &str, stderr: &str) -> String {
    if !stderr.is_empty() {
        return explain_ssh_error(host, stderr);
    }
    let normalized = remote_dir.to_ascii_lowercase();
    if normalized.contains(".ssh/config") || normalized.ends_with("/config") {
        return "Не папка mods.".to_string();
    }
    "Папка недоступна.".to_string()
}

fn index_remote_dir(host: &str, remote_dir: &str) -> Result<RemoteDirIndex, String> {
    let path = powershell_literal(remote_dir);
    let cmd = format!(
        "powershell -NoProfile -Command \"Get-ChildItem -LiteralPath '{}' -Filter *.jar -File -ErrorAction SilentlyContinue | ForEach-Object {{ Write-Output ($_.Name + '|' + $_.Length) }}\"",
        path
    );
    let output = ssh_command(host, &cmd)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format_index_dir_error(host, remote_dir, &stderr));
    }

    let mut files = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, size_raw)) = line.rsplit_once('|') else {
            continue;
        };
        let Ok(size) = size_raw.trim().parse::<u64>() else {
            continue;
        };
        if !name.is_empty() {
            files.insert(name.to_string(), size);
        }
    }
    Ok(RemoteDirIndex { files })
}

fn remote_file_matches(index: &RemoteDirIndex, filename: &str, local_size: u64) -> bool {
    index
        .files
        .get(filename)
        .map(|size| *size == local_size)
        .unwrap_or(false)
}

fn remote_dir_for_lane(config: &ServerSyncSettings, lane: SyncLane) -> Option<String> {
    match lane {
        SyncLane::Server => clean_remote_dir(&config.server_mods_path),
        SyncLane::Distribution => clean_remote_dir(&config.distribution_mods_path),
    }
}

fn lane_config_error(lane: SyncLane, config: &ServerSyncSettings) -> Option<String> {
    if remote_dir_for_lane(config, lane).is_some() {
        return None;
    }
    Some(match lane {
        SyncLane::Server => "Укажите путь.".to_string(),
        SyncLane::Distribution => "Укажите путь.".to_string(),
    })
}

fn mod_applies_to_lane(lane: SyncLane, side: &str) -> bool {
    match lane {
        SyncLane::Server => side != "client",
        SyncLane::Distribution => true,
    }
}

fn mod_needs_upload_for_lane(
    lane: SyncLane,
    config: &ServerSyncSettings,
    side: &str,
    filename: &str,
    local_size: u64,
    index: Option<&RemoteDirIndex>,
) -> bool {
    if !mod_applies_to_lane(lane, side) {
        return false;
    }
    if remote_dir_for_lane(config, lane).is_none() {
        return false;
    }
    !index
        .map(|value| remote_file_matches(value, filename, local_size))
        .unwrap_or(false)
}

fn upload_mod_for_lane(
    lane: SyncLane,
    config: &ServerSyncSettings,
    local_path: &Path,
    filename: &str,
    side: &str,
    index: Option<&RemoteDirIndex>,
) -> Result<bool, String> {
    if !mod_applies_to_lane(lane, side) {
        return Ok(false);
    }
    let Some(dir) = remote_dir_for_lane(config, lane) else {
        return Ok(false);
    };
    let local_size = fs::metadata(local_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let needs_upload = index
        .map(|value| !remote_file_matches(value, filename, local_size))
        .unwrap_or(true);
    if !needs_upload {
        return Ok(false);
    }
    let remote = join_remote_path(&dir, filename);
    scp_upload(&config.ssh_host, local_path, &remote)?;
    Ok(true)
}

fn lane_allowed_names(mods: &[crate::mods::ModEntry], lane: SyncLane) -> HashSet<String> {
    mods
        .iter()
        .filter(|entry| mod_applies_to_lane(lane, &entry.side))
        .map(|entry| entry.filename.clone())
        .collect()
}

fn classify_lane_orphans(
    mods: &[crate::mods::ModEntry],
    lane: SyncLane,
    pending_names: &[String],
    orphan_names: &[String],
) -> SyncChangeDetails {
    let local_lane_names: Vec<String> = lane_allowed_names(mods, lane).into_iter().collect();
    classify_sync_changes(pending_names, orphan_names, &local_lane_names)
}

fn prune_lane(
    config: &ServerSyncSettings,
    lane: SyncLane,
    mods: &[crate::mods::ModEntry],
    pending_names: &[String],
) -> Result<(usize, u32, u32), String> {
    if !config.delete_extra_remote_jars {
        return Ok((0, 0, 0));
    }
    let Some(dir) = remote_dir_for_lane(config, lane) else {
        return Ok((0, 0, 0));
    };
    let allowed = lane_allowed_names(mods, lane);
    let orphan_names = remote_orphan_names(&config.ssh_host, &dir, &allowed)?;
    let changes = classify_lane_orphans(mods, lane, pending_names, &orphan_names);
    let deleted = prune_remote_orphans(&config.ssh_host, &dir, &allowed)?;
    Ok((
        deleted,
        changes.to_delete,
        changes.to_update,
    ))
}

fn list_remote_jars(host: &str, remote_dir: &str) -> Result<Vec<String>, String> {
    Ok(index_remote_dir(host, remote_dir)?
        .files
        .into_keys()
        .collect())
}

fn delete_remote_file(host: &str, remote_file: &str) -> Result<(), String> {
    delete_remote_files(host, &[remote_file.to_string()])?;
    Ok(())
}

fn delete_remote_files(host: &str, remote_files: &[String]) -> Result<usize, String> {
    if remote_files.is_empty() {
        return Ok(0);
    }

    const BATCH: usize = 50;
    let mut deleted = 0usize;

    for chunk in remote_files.chunks(BATCH) {
        let paths = chunk
            .iter()
            .map(|file| format!("'{}'", powershell_literal(file)))
            .collect::<Vec<_>>()
            .join(",");
        let cmd = format!(
            "powershell -NoProfile -Command \"Remove-Item -LiteralPath {paths} -Force -ErrorAction SilentlyContinue\""
        );
        let output = ssh_command(host, &cmd)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.contains("Cannot find path") || stderr.contains("does not exist") {
                deleted += chunk.len();
                continue;
            }
            return Err(if stderr.is_empty() {
                "Не удалено.".to_string()
            } else {
                stderr
            });
        }
        deleted += chunk.len();
    }

    Ok(deleted)
}

fn delete_remote_jar(config: &ServerSyncSettings, side: &str, filename: &str) -> Result<(), String> {
    if side != "client" {
        if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
            let remote = join_remote_path(&dir, filename);
            delete_remote_file(&config.ssh_host, &remote)?;
        }
    }
    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let remote = join_remote_path(&dir, filename);
        delete_remote_file(&config.ssh_host, &remote)?;
    }
    Ok(())
}

fn prune_remote_orphans(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<usize, String> {
    let to_delete: Vec<String> = list_remote_jars(host, remote_dir)?
        .into_iter()
        .filter(|name| !allowed.contains(name))
        .map(|name| join_remote_path(remote_dir, &name))
        .collect();
    delete_remote_files(host, &to_delete)
}

fn upload_mod(
    config: &ServerSyncSettings,
    local_path: &Path,
    filename: &str,
    side: &str,
    previous_filename: Option<&str>,
    server_index: Option<&RemoteDirIndex>,
    distribution_index: Option<&RemoteDirIndex>,
) -> Result<(bool, bool), String> {
    let local_size = fs::metadata(local_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut server_uploaded = false;
    let mut distribution_uploaded = false;

    if side != "client" {
        if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
            let needs_upload = server_index
                .map(|index| !remote_file_matches(index, filename, local_size))
                .unwrap_or(true);
            if needs_upload {
                let remote = join_remote_path(&dir, filename);
                scp_upload(&config.ssh_host, local_path, &remote)?;
                server_uploaded = true;
            }
        }
    }

    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let needs_upload = distribution_index
            .map(|index| !remote_file_matches(index, filename, local_size))
            .unwrap_or(true);
        if needs_upload {
            let remote = join_remote_path(&dir, filename);
            scp_upload(&config.ssh_host, local_path, &remote)?;
            distribution_uploaded = true;
        }
    }

    if let Some(old_filename) = previous_filename.and_then(clean) {
        if old_filename != filename {
            delete_remote_jar(config, side, &old_filename)?;
        }
    }

    Ok((server_uploaded, distribution_uploaded))
}

pub(crate) fn test_connection(settings: &Settings, ssh_host: Option<&str>) -> ServerSyncTestResult {
    let Some(host) = resolve_ssh_host(settings, ssh_host) else {
        return ServerSyncTestResult {
            ok: false,
            message: "Укажите SSH host.".to_string(),
        };
    };

    if ssh_config_hostname(&host).is_none() {
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
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            ServerSyncTestResult {
                ok: false,
                message: if stderr.is_empty() {
                    "SSH недоступен.".to_string()
                } else {
                    explain_ssh_error(&host, &stderr)
                },
            }
        }
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
    let local_path = paths
        .resolve_mod_jar(filename)
        .ok_or_else(|| format!("Нет файла: {filename}"))?;
    let side = mod_side(&paths, key);
    upload_mod(
        &config,
        &local_path,
        filename,
        &side,
        previous_filename,
        None,
        None,
    )?;
    Ok(())
}

fn count_remote_orphans(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<usize, String> {
    Ok(remote_orphan_names(host, remote_dir, allowed)?.len())
}

fn remote_orphan_names(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<Vec<String>, String> {
    Ok(list_remote_jars(host, remote_dir)?
        .into_iter()
        .filter(|name| !allowed.contains(name))
        .collect())
}

fn mod_sync_identity_key(filename: &str) -> String {
    let stem = filename.trim_end_matches(".jar");
    let clean = strip_filename_decorations(stem);
    normalized_match_key(&strip_version_suffixes(&clean))
}

#[derive(Clone, Debug)]
struct SyncChangeDetails {
    to_update: u32,
    to_upload: u32,
    to_delete: u32,
    upload_names: Vec<String>,
    update_pairs: Vec<ServerSyncUpdatePair>,
    delete_names: Vec<String>,
}

fn mod_entry_for_sync_filename<'a>(
    mods: &'a [crate::mods::ModEntry],
    filename: &str,
) -> Option<&'a crate::mods::ModEntry> {
    if let Some(exact) = mods.iter().find(|entry| entry.filename == filename) {
        return Some(exact);
    }
    let key = mod_sync_identity_key(filename);
    mods
        .iter()
        .find(|entry| mod_sync_identity_key(&entry.filename) == key)
}

fn delete_items_for_names(mods: &[crate::mods::ModEntry], names: &[String]) -> Vec<ServerSyncDeleteItem> {
    names
        .iter()
        .map(|name| {
            if let Some(entry) = mod_entry_for_sync_filename(mods, name) {
                ServerSyncDeleteItem {
                    filename: name.clone(),
                    side: entry.side.clone(),
                    library: entry.library,
                    technical: entry.technical,
                }
            } else {
                ServerSyncDeleteItem {
                    filename: name.clone(),
                    side: "universal".to_string(),
                    library: false,
                    technical: false,
                }
            }
        })
        .collect()
}

fn classify_sync_changes(
    pending: &[String],
    orphans: &[String],
    local_lane_names: &[String],
) -> SyncChangeDetails {
    let mut pending = pending.to_vec();
    let mut update_pairs = Vec::new();
    let mut delete_names = Vec::new();

    for orphan in orphans {
        let key = mod_sync_identity_key(orphan);
        if let Some(pos) = pending.iter().position(|name| mod_sync_identity_key(name) == key) {
            let local = pending.remove(pos);
            update_pairs.push(ServerSyncUpdatePair {
                remote: orphan.clone(),
                local,
            });
            continue;
        }
        if let Some(local) = local_lane_names
            .iter()
            .find(|name| mod_sync_identity_key(name.as_str()) == key)
        {
            update_pairs.push(ServerSyncUpdatePair {
                remote: orphan.clone(),
                local: local.clone(),
            });
            continue;
        }
        delete_names.push(orphan.clone());
    }

    SyncChangeDetails {
        to_update: update_pairs.len() as u32,
        to_upload: pending.len() as u32,
        to_delete: delete_names.len() as u32,
        upload_names: pending,
        update_pairs,
        delete_names,
    }
}

fn prepare_lane_sync(
    app: &AppHandle,
    lane: SyncLane,
) -> Result<(ServerSyncSettings, Vec<crate::mods::ModEntry>, crate::settings::InstancePaths), String>
{
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Err(sync_config_error(&settings));
    };
    if let Some(message) = lane_config_error(lane, &config) {
        return Err(message);
    }
    if ssh_config_hostname(&config.ssh_host).is_none() {
        return Err(format!("«{}» не в ~/.ssh/config.", config.ssh_host));
    }
    let paths = resolve_paths(&settings)?;
    let catalog_root = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(&settings, catalog_root)?;
    Ok((config, mods, paths))
}

fn analyze_lane_sync(app: &AppHandle, lane: SyncLane) -> Result<LaneSyncAnalysis, String> {
    let (config, mods, paths) = prepare_lane_sync(app, lane)?;

    let jobs: Vec<_> = mods
        .iter()
        .filter(|entry| mod_applies_to_lane(lane, &entry.side))
        .filter_map(|entry| {
            paths
                .resolve_mod_jar(&entry.filename)
                .map(|local_path| (entry.clone(), local_path))
        })
        .collect();
    let total_all = jobs.len() as u32;

    let remote_dir = remote_dir_for_lane(&config, lane).expect("lane path checked");
    let remote_index = index_remote_dir(&config.ssh_host, &remote_dir)?;
    let remote_count = remote_index.files.len() as u32;

    let mut pending = Vec::new();
    let mut already_synced = 0u32;
    for (entry, local_path) in jobs {
        let local_size = fs::metadata(&local_path).map(|metadata| metadata.len()).unwrap_or(0);
        if mod_needs_upload_for_lane(
            lane,
            &config,
            &entry.side,
            &entry.filename,
            local_size,
            Some(&remote_index),
        ) {
            pending.push((entry, local_path));
        } else {
            already_synced += 1;
        }
    }

    let to_delete = if config.delete_extra_remote_jars {
        let allowed: HashSet<String> = mods
            .iter()
            .filter(|entry| mod_applies_to_lane(lane, &entry.side))
            .map(|entry| entry.filename.clone())
            .collect();
        count_remote_orphans(&config.ssh_host, &remote_dir, &allowed)?
    } else {
        0
    };

    Ok(LaneSyncAnalysis {
        config,
        mods,
        pending,
        already_synced,
        total_all,
        remote_count,
        to_delete: to_delete as u32,
        remote_index,
    })
}

fn preview_lane_mods(app: &AppHandle, lane: SyncLane) -> ServerSyncLanePreview {
    match analyze_lane_sync(app, lane) {
        Ok(analysis) => {
            let allowed: HashSet<String> = analysis
                .mods
                .iter()
                .filter(|entry| mod_applies_to_lane(lane, &entry.side))
                .map(|entry| entry.filename.clone())
                .collect();
            let remote_dir = remote_dir_for_lane(&analysis.config, lane).expect("lane path checked");
            let orphan_names = if analysis.config.delete_extra_remote_jars {
                remote_orphan_names(&analysis.config.ssh_host, &remote_dir, &allowed).unwrap_or_default()
            } else {
                Vec::new()
            };
            let pending_names: Vec<String> = analysis
                .pending
                .iter()
                .map(|(entry, _)| entry.filename.clone())
                .collect();
            let changes = classify_lane_orphans(
                &analysis.mods,
                lane,
                &pending_names,
                &orphan_names,
            );
            let to_delete_items = delete_items_for_names(&analysis.mods, &changes.delete_names);

            ServerSyncLanePreview {
                ok: true,
                local: analysis.total_all,
                remote: analysis.remote_count,
                to_upload: changes.to_upload,
                already_synced: analysis.already_synced,
                to_delete: changes.to_delete,
                to_update: changes.to_update,
                to_upload_names: changes.upload_names,
                to_update_pairs: changes.update_pairs,
                to_delete_names: changes.delete_names,
                to_delete_items,
                errors: Vec::new(),
            }
        }
        Err(error) => ServerSyncLanePreview {
            ok: false,
            local: 0,
            remote: 0,
            to_upload: 0,
            already_synced: 0,
            to_delete: 0,
            to_update: 0,
            to_upload_names: Vec::new(),
            to_update_pairs: Vec::new(),
            to_delete_names: Vec::new(),
            to_delete_items: Vec::new(),
            errors: vec![error],
        },
    }
}

pub(crate) fn sync_lane_mods(app: &AppHandle, state: &ServerSyncState) -> Result<ServerSyncBulkResult, String> {
    match sync_lane_mods_inner(app, state) {
        Ok(result) => Ok(result),
        Err(message) => {
            if !state.snapshot().done {
                state.fail_start(message.clone());
                emit_server_sync_progress(app, state);
            }
            Err(message)
        }
    }
}

fn sync_lane_mods_inner(app: &AppHandle, state: &ServerSyncState) -> Result<ServerSyncBulkResult, String> {
    let lane = state.lane;
    let analysis = match analyze_lane_sync(app, lane) {
        Ok(value) => value,
        Err(message) => {
            state.fail_start(message.clone());
            emit_server_sync_progress(app, state);
            return Err(message);
        }
    };

    let LaneSyncAnalysis {
        config,
        mods,
        pending,
        already_synced,
        total_all,
        to_delete,
        remote_index,
        ..
    } = analysis;

    state.set_checking(total_all);
    emit_server_sync_progress(app, state);

    if state.is_cancelled() {
        state.reset();
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped: 0,
            deleted: 0,
            errors: vec!["Отменено.".to_string()],
        });
    }

    let total = pending.len() as u32;
    let pending_names: Vec<String> = pending
        .iter()
        .map(|(entry, _)| entry.filename.clone())
        .collect();
    if total == 0 && to_delete > 0 && config.delete_extra_remote_jars {
        state.set_pruning(to_delete, already_synced);
    } else {
        state.begin_upload(total, already_synced, total_all);
    }
    emit_server_sync_progress(app, state);

    if total_all == 0 {
        let mut errors = Vec::new();
        let mut deleted = 0usize;
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(to_delete, already_synced);
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, deleted_extra, replaced_remote)) => {
                deleted = count;
                state.set_deleted_breakdown(deleted as u32, deleted_extra, replaced_remote);
            }
            Err(error) => errors.push(error),
        }
        let ok = errors.is_empty();
        state.finish(ok);
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped: already_synced as usize,
            deleted,
            errors,
        });
    }

    let mut uploaded = 0usize;
    let mut skipped = already_synced as usize;
    let mut errors = Vec::new();

    if total == 0 {
        let mut deleted = 0usize;
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(to_delete, already_synced);
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, deleted_extra, replaced_remote)) => {
                deleted = count;
                state.set_deleted_breakdown(deleted as u32, deleted_extra, replaced_remote);
            }
            Err(error) => errors.push(error),
        }
        let ok = errors.is_empty();
        state.finish(ok);
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped,
            deleted,
            errors,
        });
    }

    for (index, (entry, local_path)) in pending.iter().enumerate() {
        if state.is_cancelled() {
            errors.push("Отменено.".to_string());
            break;
        }

        let current = index as u32 + 1;
        state.set_step(current, total, &entry.filename);
        emit_server_sync_progress(app, state);

        match upload_mod_for_lane(
            lane,
            &config,
            local_path,
            &entry.filename,
            &entry.side,
            Some(&remote_index),
        ) {
            Ok(true) => {
                uploaded += 1;
                state.add_result(true, false, None);
            }
            Ok(false) => {
                skipped += 1;
                state.add_result(false, true, None);
            }
            Err(error) => {
                let message = short_msg(&format!("{}: {error}", entry.filename), 48);
                errors.push(message.clone());
                state.add_result(false, false, Some(message));
            }
        }
    }

    let mut deleted = 0usize;
    if !state.is_cancelled() {
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(to_delete, skipped as u32);
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, deleted_extra, replaced_remote)) => {
                deleted = count;
                state.set_deleted_breakdown(deleted as u32, deleted_extra, replaced_remote);
            }
            Err(error) => errors.push(error),
        }
    }

    if state.is_cancelled() {
        state.reset();
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded,
            skipped,
            deleted: 0,
            errors: vec!["Отменено.".to_string()],
        });
    }

    let ok = errors.is_empty();
    state.finish(ok);
    emit_server_sync_progress(app, state);

    Ok(ServerSyncBulkResult {
        uploaded,
        skipped,
        deleted,
        errors,
    })
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
                if let Some(entry) = mods.iter().find(|item| item.key == key) {
                    let _ = sync_mod_file(&app, &key, &entry.filename, None);
                }
            }
            Ok::<(), String>(())
        })
        .await;
    });
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestServerSyncRequest {
    #[serde(default)]
    pub ssh_host: Option<String>,
}

#[tauri::command]
pub(crate) async fn test_server_sync(
    app: AppHandle,
    request: Option<TestServerSyncRequest>,
) -> Result<ServerSyncTestResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        let ssh_host = request.as_ref().and_then(|item| item.ssh_host.as_deref());
        Ok(test_connection(&settings, ssh_host))
    })
    .await
    .map_err(|error| format!("Проверка: {error}"))?
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncLaneRequest {
    pub lane: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncStatuses {
    pub server: ServerSyncProgress,
    pub distribution: ServerSyncProgress,
}

async fn start_lane_sync(app: &AppHandle, state: &ServerSyncState) -> Result<ServerSyncStartResult, String> {
    if !state.try_acquire() {
        return Ok(ServerSyncStartResult {
            started: false,
            already_running: true,
        });
    }

    let sync_state = state.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let app_for_panic = app_handle.clone();
        let state_for_panic = sync_state.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            sync_lane_mods(&app_handle, &sync_state)
        })
        .await;

        if let Err(_) = result {
            if !state_for_panic.snapshot().done {
                state_for_panic.fail_start("Прервано.".to_string());
                emit_server_sync_progress(&app_for_panic, &state_for_panic);
            }
        }
    });

    Ok(ServerSyncStartResult {
        started: true,
        already_running: false,
    })
}

#[tauri::command]
pub(crate) fn cancel_server_sync_lane(
    lanes: State<'_, ServerSyncLanes>,
    request: ServerSyncLaneRequest,
) -> Result<(), String> {
    let Some(state) = lanes.get(&request.lane) else {
        return Err("Неизвестный тип.".to_string());
    };
    state.request_cancel();
    Ok(())
}

#[tauri::command]
pub(crate) fn get_server_sync_statuses(lanes: State<'_, ServerSyncLanes>) -> ServerSyncStatuses {
    ServerSyncStatuses {
        server: lanes.server.progress_with_target(),
        distribution: lanes.distribution.progress_with_target(),
    }
}

#[tauri::command]
pub(crate) async fn preview_server_sync_lane(
    app: AppHandle,
    lanes: State<'_, ServerSyncLanes>,
    request: ServerSyncLaneRequest,
) -> Result<ServerSyncLanePreview, String> {
    let Some(lane) = SyncLane::from_str(&request.lane) else {
        return Err("Неизвестный тип.".to_string());
    };
    if lanes.get_lane(lane).is_running() {
        return Err("Синхронизация идёт.".to_string());
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || preview_lane_mods(&app, lane))
        .await
        .map_err(|error| format!("Проверка: {error}"))
}

#[tauri::command]
pub(crate) async fn sync_mods_to_server_lane(
    app: AppHandle,
    lanes: State<'_, ServerSyncLanes>,
    request: ServerSyncLaneRequest,
) -> Result<ServerSyncStartResult, String> {
    let Some(state) = lanes.get(&request.lane) else {
        return Err("Неизвестный тип.".to_string());
    };
    start_lane_sync(&app, state).await
}

#[cfg(test)]
mod sync_identity_tests {
    use super::{classify_sync_changes, mod_sync_identity_key};

    #[test]
    fn identity_key_ignores_version_suffix() {
        assert_eq!(
            mod_sync_identity_key("appleskin-neoforge-mc1.21-3.0.9.jar"),
            mod_sync_identity_key("appleskin-neoforge-mc1.21-3.0.7.jar")
        );
        assert_eq!(
            mod_sync_identity_key("AE2NetworkAnalyzer-1.21-2.1.5-neoforge.jar"),
            mod_sync_identity_key("AE2NetworkAnalyzer-1.21-2.1.3-neoforge.jar")
        );
    }

    #[test]
    fn classify_pairs_version_updates() {
        let pending = vec![
            "appleskin-neoforge-mc1.21-3.0.9.jar".to_string(),
            "brand-new-mod-1.0.0.jar".to_string(),
        ];
        let orphans = vec![
            "appleskin-neoforge-mc1.21-3.0.7.jar".to_string(),
            "removed-mod-old.jar".to_string(),
        ];
        let local_lane_names = pending.clone();
        let counts = classify_sync_changes(&pending, &orphans, &local_lane_names);
        assert_eq!(counts.to_update, 1);
        assert_eq!(counts.to_upload, 1);
        assert_eq!(counts.to_delete, 1);
    }

    #[test]
    fn classify_pairs_already_synced_replacement() {
        let pending = vec!["brand-new-mod-1.0.0.jar".to_string()];
        let orphans = vec!["ae2wtlib-19.3.0.jar".to_string()];
        let local_lane_names = vec![
            "ae2wtlib-19.5.0.jar".to_string(),
            "brand-new-mod-1.0.0.jar".to_string(),
        ];
        let counts = classify_sync_changes(&pending, &orphans, &local_lane_names);
        assert_eq!(counts.to_update, 1);
        assert_eq!(counts.to_upload, 1);
        assert_eq!(counts.to_delete, 0);
        assert_eq!(counts.update_pairs[0].remote, "ae2wtlib-19.3.0.jar");
        assert_eq!(counts.update_pairs[0].local, "ae2wtlib-19.5.0.jar");
    }
}
