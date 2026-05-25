use reqwest::header::CONTENT_TYPE;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::catalog;
use crate::mods::ModEntry;
use crate::settings::InstancePaths;
use crate::util::{file_mtime_millis, path_string};

pub(crate) const COVER_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

pub(crate) fn safe_file_stem(value: &str) -> String {
    let stem: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim_matches('_').to_string();
    if stem.is_empty() {
        "cover".to_string()
    } else {
        stem
    }
}

pub(crate) fn cover_ext_from_mime(mime: &str) -> Option<&'static str> {
    match mime.split(';').next().unwrap_or("").trim() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn cover_ext_from_url(url: &str) -> &'static str {
    let clean = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url);
    match Path::new(clean)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "jpg",
        Some("webp") => "webp",
        Some("gif") => "gif",
        _ => "png",
    }
}

pub(crate) fn cover_dir(data_root: &Path, manual: bool) -> PathBuf {
    data_root
        .join("covers")
        .join(if manual { "manual" } else { "cache" })
}

pub(crate) fn hash_cover_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(key.as_bytes());
    format!("{:x}", digest)[..24].to_string()
}

fn cover_platform_prefixes(modrinth_id: Option<&str>, curseforge_id: Option<&str>) -> Vec<String> {
    let mut prefixes = Vec::new();
    if let Some(id) = modrinth_id.filter(|value| !value.is_empty()) {
        prefixes.push(format!("modrinth_{id}"));
        prefixes.push(format!("modrinth-{id}"));
    }
    if let Some(id) = curseforge_id.filter(|value| !value.is_empty()) {
        prefixes.push(format!("curseforge_{id}"));
        prefixes.push(format!("curseforge-{id}"));
    }
    prefixes
}

fn has_cover_extension(name: &str) -> bool {
    let Some(ext) = Path::new(name).extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif"
    )
}

fn find_cover_by_prefix(dir: &Path, prefix: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let needle = prefix.to_ascii_lowercase();
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.starts_with(&needle) || !has_cover_extension(&name) {
            continue;
        }
        let suffix = name.strip_prefix(&needle)?;
        if suffix.starts_with('.') || suffix.starts_with('_') {
            return Some(entry.path());
        }
    }
    None
}

fn find_cover_file(dir: &Path, key: &str) -> Option<PathBuf> {
    let stem = safe_file_stem(key);
    COVER_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
}

fn find_cover_in_dir(dir: &Path, prefixes: &[String], key: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    for prefix in prefixes {
        if let Some(path) = find_cover_by_prefix(dir, prefix) {
            return Some(path);
        }
    }
    find_cover_file(dir, key)
}

pub(crate) fn remove_cover_variants(dir: &Path, key: &str) {
    let stem = safe_file_stem(key);
    for ext in COVER_EXTENSIONS {
        let _ = fs::remove_file(dir.join(format!("{stem}.{ext}")));
    }
}

pub(crate) fn remove_cover_prefix_variants(dir: &Path, prefixes: &[String]) {
    if !dir.is_dir() {
        return;
    }
    let entries = fs::read_dir(dir).ok();
    let Some(entries) = entries else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        for prefix in prefixes {
            let needle = prefix.to_ascii_lowercase();
            if name.starts_with(&needle) && has_cover_extension(&name) {
                let _ = fs::remove_file(entry.path());
                break;
            }
        }
    }
}

pub(crate) fn apply_existing_cover(
    item: &mut ModEntry,
    paths: &InstancePaths,
    catalog_root: Option<&Path>,
) {
    let prefixes =
        cover_platform_prefixes(item.modrinth_id.as_deref(), item.curseforge_id.as_deref());
    let manual_hash = hash_cover_key(&item.key);

    let manual_dir = cover_dir(&paths.data_root, true);
    if let Some(path) = find_cover_by_prefix(&manual_dir, &manual_hash)
        .or_else(|| find_cover_file(&manual_dir, &item.key))
    {
        item.cover_modified_at = file_mtime_millis(&path);
        item.cover_path = Some(path_string(path));
        item.cover_manual = true;
        return;
    }

    let cache_dir = cover_dir(&paths.data_root, false);
    if let Some(path) = find_cover_in_dir(&cache_dir, &prefixes, &item.key) {
        item.cover_modified_at = file_mtime_millis(&path);
        item.cover_path = Some(path_string(path));
        item.cover_manual = false;
        return;
    }

    if let Some(root) = catalog_root {
        for prefix in &prefixes {
            if let Some(path) = catalog::find_catalog_cover(root, prefix) {
                item.cover_modified_at = file_mtime_millis(&path);
                item.cover_path = Some(path_string(path));
                item.cover_manual = false;
                return;
            }
        }
    }
}

pub(crate) fn delete_manual_cover(paths: &InstancePaths, key: &str) -> Result<bool, String> {
    let dir = cover_dir(&paths.data_root, true);
    if !dir.exists() {
        return Ok(false);
    }
    let hash = hash_cover_key(key);
    let mut removed = false;
    let entries = std::fs::read_dir(&dir).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !has_cover_extension(&name) {
            continue;
        }
        let matches_hash = name.starts_with(&hash.to_ascii_lowercase()) && {
            let suffix = name.trim_start_matches(&hash.to_ascii_lowercase());
            suffix.starts_with('.') || suffix.starts_with('_')
        };
        if matches_hash {
            let _ = std::fs::remove_file(entry.path());
            removed = true;
        }
    }
    remove_cover_variants(&dir, key);
    Ok(removed)
}

pub(crate) fn cache_remote_cover(
    client: &reqwest::blocking::Client,
    paths: &InstancePaths,
    catalog_root: Option<&Path>,
    key: &str,
    modrinth_id: Option<&str>,
    curseforge_id: Option<&str>,
    url: &str,
) -> Option<PathBuf> {
    let prefixes = cover_platform_prefixes(modrinth_id, curseforge_id);

    if let Some(root) = catalog_root {
        for prefix in &prefixes {
            if let Some(path) = catalog::find_catalog_cover(root, prefix) {
                return Some(path);
            }
        }
    }

    let dir = cover_dir(&paths.data_root, false);
    if let Some(path) = find_cover_in_dir(&dir, &prefixes, key) {
        return Some(path);
    }

    let response = client.get(url).send().ok()?.error_for_status().ok()?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes().ok()?;
    if bytes.is_empty() {
        return None;
    }

    let ext = cover_ext_from_mime(&content_type).unwrap_or_else(|| cover_ext_from_url(url));

    if let Some(root) = catalog_root {
        for prefix in &prefixes {
            if let Ok(path) = catalog::save_catalog_cover(root, prefix, &bytes, ext) {
                return Some(path);
            }
        }
    }

    fs::create_dir_all(&dir).ok()?;
    if let Some(prefix) = prefixes.first() {
        remove_cover_prefix_variants(&dir, &prefixes);
        let path = dir.join(format!("{prefix}.{ext}"));
        if fs::write(&path, &bytes).is_ok() {
            return Some(path);
        }
    }

    remove_cover_variants(&dir, key);
    let path = dir.join(format!("{}.{}", safe_file_stem(key), ext));
    fs::write(&path, &bytes).ok()?;
    Some(path)
}

pub(crate) fn store_uploaded_cover(
    paths: &InstancePaths,
    key: &str,
    bytes: &[u8],
    ext: &str,
) -> Result<PathBuf, String> {
    let dir = cover_dir(&paths.data_root, true);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let hash = hash_cover_key(key);
    remove_cover_prefix_variants(&dir, std::slice::from_ref(&hash));
    remove_cover_variants(&dir, key);
    let path = dir.join(format!("{hash}.{ext}"));
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}
