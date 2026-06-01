use serde::Serialize;
use tauri::{AppHandle, Emitter};

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
struct LabelsReadyPayload {
    key: String,
    side: String,
    library: bool,
    technical: bool,
    side_mode: String,
    manual_side: String,
    manual_library: bool,
    manual_technical: bool,
    provider_side: String,
    provider_library: bool,
    provider_technical: bool,
}

pub(crate) fn emit_labels_ready(
    app: &AppHandle,
    key: &str,
    side: &str,
    library: bool,
    technical: bool,
    side_mode: &str,
    manual_side: &str,
    manual_library: bool,
    manual_technical: bool,
    provider_side: &str,
    provider_library: bool,
    provider_technical: bool,
) {
    let _ = app.emit(
        "labels-ready",
        LabelsReadyPayload {
            key: key.to_string(),
            side: side.to_string(),
            library,
            technical,
            side_mode: side_mode.to_string(),
            manual_side: manual_side.to_string(),
            manual_library,
            manual_technical,
            provider_side: provider_side.to_string(),
            provider_library,
            provider_technical,
        },
    );
}
