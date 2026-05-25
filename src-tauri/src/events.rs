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
}

pub(crate) fn emit_cover_ready(app: &AppHandle, key: &str, cover_path: &str) {
    let _ = app.emit(
        "cover-ready",
        CoverReadyPayload {
            key: key.to_string(),
            cover_path: cover_path.to_string(),
        },
    );
}
