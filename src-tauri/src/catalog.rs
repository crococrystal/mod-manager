use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

const COVER_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

pub fn catalog_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("catalog");
    fs::create_dir_all(root.join("covers")).map_err(|error| error.to_string())?;
    Ok(root)
}

/// Стабильный ключ кэша по ID платформы (как mod-panel: modrinth-… / curseforge-…).
#[allow(dead_code)]
pub fn cover_cache_prefix(modrinth_id: Option<&str>, curseforge_id: Option<&str>) -> Option<String> {
    if let Some(id) = modrinth_id.filter(|s| !s.is_empty()) {
        return Some(format!("modrinth-{id}"));
    }
    if let Some(id) = curseforge_id.filter(|s| !s.is_empty()) {
        return Some(format!("curseforge-{id}"));
    }
    None
}

pub fn find_catalog_cover(root: &Path, prefix: &str) -> Option<PathBuf> {
    let dir = root.join("covers");
    if !dir.is_dir() {
        return None;
    }
    let needle = prefix.to_ascii_lowercase();
    let entries = fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.starts_with(&needle) {
            continue;
        }
        let suffix = name.strip_prefix(&needle)?;
        if suffix.starts_with('.') || suffix.starts_with('_') {
            let ext = Path::new(&name).extension().and_then(|value| value.to_str())?;
            if matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "gif") {
                return Some(entry.path());
            }
        }
    }
    None
}

pub fn save_catalog_cover(root: &Path, prefix: &str, bytes: &[u8], ext: &str) -> Result<PathBuf, String> {
    let dir = root.join("covers");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    for old in COVER_EXTENSIONS {
        let _ = fs::remove_file(dir.join(format!("{prefix}.{old}")));
    }
    let path = dir.join(format!("{prefix}.{ext}"));
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}
