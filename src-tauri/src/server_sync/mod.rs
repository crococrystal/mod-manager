mod config;
mod lane;
mod mod_actions;
mod remote;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

pub(crate) use config::sync_upload_side;
pub(crate) use mod_actions::{
    find_mod_for_sync_key, schedule_delete_mod, schedule_disable_mod, schedule_enable_mod,
    schedule_sync_keys, schedule_sync_mod, test_connection,
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
    #[serde(default)]
    pub uploaded_names: Vec<String>,
    #[serde(default)]
    pub skipped_names: Vec<String>,
    #[serde(default)]
    pub deleted_names: Vec<String>,
    #[serde(default)]
    pub deleted_items: Vec<ServerSyncDeleteItem>,
    #[serde(default)]
    pub update_pairs: Vec<ServerSyncUpdatePair>,
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
    pub(super) lane: SyncLane,
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

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn set_checking(&self, total_all: u32) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress {
                active: true,
                phase: "checking".to_string(),
                total_all,
                ..ServerSyncProgress::default()
            };
        }
    }

    pub(super) fn begin_upload(
        &self,
        total: u32,
        already_synced: u32,
        total_all: u32,
        already_synced_names: Vec<String>,
    ) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress {
                active: true,
                phase: "uploading".to_string(),
                total,
                total_all,
                already_synced,
                skipped: already_synced,
                skipped_names: already_synced_names,
                ..ServerSyncProgress::default()
            };
        }
    }

    pub(super) fn set_pruning(&self, to_delete: u32, already_synced: u32, already_synced_names: Vec<String>) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.active = true;
            progress.phase = "pruning".to_string();
            progress.total = to_delete;
            progress.current = 0;
            progress.already_synced = already_synced;
            progress.skipped = already_synced;
            progress.skipped_names = already_synced_names;
            progress.done = false;
            progress.ok = false;
            progress.filename.clear();
        }
    }

    pub(super) fn set_step(&self, current: u32, total: u32, filename: &str) {
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

    pub(super) fn add_result(&self, uploaded: bool, skipped: bool, filename: &str, error: Option<String>) {
        if let Ok(mut progress) = self.inner.lock() {
            if uploaded {
                progress.uploaded += 1;
                progress.uploaded_names.push(filename.to_string());
            }
            if skipped {
                progress.skipped += 1;
                progress.skipped_names.push(filename.to_string());
            }
            if let Some(message) = error {
                progress.errors.push(message);
            }
        }
    }

    pub(super) fn set_prune_details(
        &self,
        deleted: u32,
        deleted_extra: u32,
        replaced_remote: u32,
        delete_names: Vec<String>,
        delete_items: Vec<ServerSyncDeleteItem>,
        update_pairs: Vec<ServerSyncUpdatePair>,
    ) {
        if let Ok(mut progress) = self.inner.lock() {
            progress.deleted = deleted;
            progress.deleted_extra = deleted_extra;
            progress.replaced_remote = replaced_remote;
            progress.deleted_names = delete_names;
            progress.deleted_items = delete_items;
            progress.update_pairs = update_pairs;
        }
    }

    pub(super) fn reset(&self) {
        if let Ok(mut progress) = self.inner.lock() {
            *progress = ServerSyncProgress::default();
        }
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }

    pub(super) fn finish(&self, ok: bool) {
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

    pub(super) fn fail_start(&self, message: String) {
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

pub(super) fn emit_server_sync_progress(app: &AppHandle, state: &ServerSyncState) {
    let _ = app.emit("server-sync-progress", state.progress_with_target());
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
        let settings = crate::settings::read_settings(&app)?;
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
            lane::sync_lane_mods(&app_handle, &sync_state)
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
    tauri::async_runtime::spawn_blocking(move || Ok(lane::preview_lane_mods(&app, lane)))
        .await
        .map_err(|error| format!("Проверка: {error}"))?
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
    use super::lane::{classify_sync_changes, mod_sync_identity_key};

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
