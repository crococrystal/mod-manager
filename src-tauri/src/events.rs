use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrefetchReport {
    pub downloaded: u32,
    pub skipped: u32,
    pub failed: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub added_links: u32,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl PrefetchReport {
    pub fn new() -> Self {
        Self {
            downloaded: 0,
            skipped: 0,
            failed: 0,
            updated: 0,
            unchanged: 0,
            added_links: 0,
            errors: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrefetchProgressPayload {
    kind: String,
    index: u32,
    total: u32,
    name: String,
    status: String,
    detail: String,
}

pub(crate) fn emit_prefetch_progress(
    app: &AppHandle,
    kind: &str,
    index: u32,
    total: u32,
    name: &str,
    status: &str,
    detail: &str,
) {
    let _ = app.emit(
        "prefetch-progress",
        PrefetchProgressPayload {
            kind: kind.to_string(),
            index,
            total,
            name: name.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
        },
    );
}

pub(crate) fn emit_prefetch_done(app: &AppHandle, kind: &str) {
    emit_prefetch_progress(app, kind, 0, 0, "", "done", "");
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverReadyPayload {
    key: String,
    cover_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_modified_at: Option<u64>,
}

pub(crate) fn emit_cover_ready(
    app: &AppHandle,
    key: &str,
    cover_path: &str,
    cover_modified_at: Option<u64>,
) {
    let _ = app.emit(
        "cover-ready",
        CoverReadyPayload {
            key: key.to_string(),
            cover_path: cover_path.to_string(),
            cover_modified_at,
        },
    );
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModSourceReadyPayload {
    key: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modrinth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modrinth_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    curseforge_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    curseforge_file_id: Option<String>,
}

pub(crate) fn emit_mod_source_ready(
    app: &AppHandle,
    key: &str,
    source: &str,
    display_name: Option<String>,
    source_url: Option<String>,
    modrinth_id: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_id: Option<String>,
    curseforge_file_id: Option<String>,
) {
    let _ = app.emit(
        "mod-source-ready",
        ModSourceReadyPayload {
            key: key.to_string(),
            source: source.to_string(),
            display_name,
            source_url,
            modrinth_id,
            modrinth_version_id,
            curseforge_id,
            curseforge_file_id,
        },
    );
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DependenciesReadyPayload {
    key: String,
    dependencies: Vec<String>,
}

pub(crate) fn emit_dependencies_ready(app: &AppHandle, key: &str, dependencies: &[String]) {
    let _ = app.emit(
        "dependencies-ready",
        DependenciesReadyPayload {
            key: key.to_string(),
            dependencies: dependencies.to_vec(),
        },
    );
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProgressPayload {
    phase: String,
    index: u32,
    total: u32,
    name: String,
}

pub(crate) fn emit_sync_progress(
    app: &AppHandle,
    phase: &str,
    index: u32,
    total: u32,
    name: &str,
) {
    let _ = app.emit(
        "sync-progress",
        SyncProgressPayload {
            phase: phase.to_string(),
            index,
            total,
            name: name.to_string(),
        },
    );
}
