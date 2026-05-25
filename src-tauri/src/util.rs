use chrono::{DateTime, Utc};
use std::time::SystemTime;

pub(crate) fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn system_time_iso(value: SystemTime) -> String {
    let dt: DateTime<Utc> = value.into();
    dt.to_rfc3339()
}

pub(crate) fn path_string(path: std::path::PathBuf) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn file_mtime_millis(path: &std::path::Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_millis() as u64)
}
