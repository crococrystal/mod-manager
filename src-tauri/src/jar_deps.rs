use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path, process::Command, time::SystemTime};

const SKIP_MOD_IDS: &[&str] = &[
    "minecraft",
    "neoforge",
    "forge",
    "javafml",
    "fabric",
    "fabricloader",
    "fabric-api",
    "fml",
    "kotlinthedivisionbyzero",
];

const TOML_PATHS: &[&str] = &["META-INF/neoforge.mods.toml", "META-INF/mods.toml"];
const JSON_METADATA_PATHS: &[&str] = &["fabric.mod.json", "quilt.mod.json"];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JarCacheEntry {
    mtime_ms: u64,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    mod_id: Option<String>,
    #[serde(default)]
    dependency_mod_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct JarCacheFile {
    #[serde(default = "jar_cache_version")]
    version: u8,
    #[serde(default)]
    entries: HashMap<String, JarCacheEntry>,
}

fn jar_cache_version() -> u8 {
    5
}

#[derive(Clone, Debug, Default)]
pub struct JarInfo {
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub dependency_keys: Vec<String>,
}

fn read_jar_entry(jar_path: &Path, entry: &str) -> Option<String> {
    let path = jar_path.to_string_lossy();
    let output = Command::new("unzip")
        .args(["-p", &path, entry])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    (!text.trim().is_empty()).then_some(text)
}

fn read_jar_toml(jar_path: &Path) -> Option<String> {
    TOML_PATHS
        .iter()
        .find_map(|entry| read_jar_entry(jar_path, entry))
}

fn read_jar_json_metadata(jar_path: &Path) -> Option<String> {
    JSON_METADATA_PATHS
        .iter()
        .find_map(|entry| read_jar_entry(jar_path, entry))
}

fn clean_metadata_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('$') {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn clean_metadata_version(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with('$') {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_toml_display_name(text: &str) -> Option<String> {
    let block = text.split("[[mods]]").nth(1)?;
    let head = block.split("[[dependencies.").next()?;
    for line in head.lines() {
        if let Some(value) = extract_quoted_value(line, "displayName") {
            return clean_metadata_name(&value);
        }
    }
    None
}

fn parse_toml_version(text: &str) -> Option<String> {
    let block = text.split("[[mods]]").nth(1)?;
    let head = block.split("[[dependencies.").next()?;
    for line in head.lines() {
        if let Some(value) = extract_quoted_value(line, "version") {
            return clean_metadata_version(&value);
        }
    }
    None
}

fn extract_quoted_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with(key) {
        return None;
    }
    let rest = trimmed[key.len()..].trim_start_matches(['=', ' ']);
    if let Some(inner) = rest.strip_prefix('"').and_then(|s| s.split('"').next()) {
        return Some(inner.to_string());
    }
    None
}

fn parse_json_metadata(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return (None, None, None);
    };
    let display_name = value
        .get("name")
        .and_then(|value| value.as_str())
        .and_then(clean_metadata_name);
    let version = value
        .get("version")
        .and_then(|value| value.as_str())
        .and_then(clean_metadata_version);
    let mod_id = value
        .get("id")
        .and_then(|value| value.as_str())
        .and_then(clean_metadata_name);
    (display_name, version, mod_id)
}

fn parse_own_mod_id(text: &str) -> Option<String> {
    let block = text.split("[[mods]]").nth(1)?;
    let head = block.split("[[dependencies.").next()?;
    for line in head.lines() {
        if let Some(value) = extract_quoted_value(line, "modId") {
            return Some(value);
        }
    }
    None
}

fn parse_dependency_mod_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for block in text.split("[[dependencies.").skip(1) {
        let mut mod_id = None;
        let mut dep_type = "required".to_string();
        for line in block.lines() {
            if mod_id.is_none() {
                mod_id = extract_quoted_value(line, "modId");
            }
            if let Some(value) = extract_quoted_value(line, "type") {
                dep_type = value.to_ascii_lowercase();
            }
        }
        let Some(mod_id) = mod_id else { continue };
        if SKIP_MOD_IDS.contains(&mod_id.as_str()) {
            continue;
        }
        if dep_type != "required" {
            continue;
        }
        if !ids.contains(&mod_id) {
            ids.push(mod_id);
        }
    }
    ids
}

fn normalize_token(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

pub struct ModRef {
    pub key: String,
    pub filename: String,
    pub display_name: String,
    pub base: String,
    pub modrinth_id: Option<String>,
}

pub fn build_registry(mods: &[ModRef]) -> HashMap<String, String> {
    let mut registry = HashMap::new();
    for item in mods {
        let mut register = |token: &str, key: &str| {
            let t = token.trim().to_ascii_lowercase();
            if !t.is_empty() && !registry.contains_key(&t) {
                registry.insert(t, key.to_string());
            }
            let norm = normalize_token(token);
            if !norm.is_empty() && !registry.contains_key(&norm) {
                registry.insert(norm, key.to_string());
            }
        };
        if let Some(id) = item.modrinth_id.as_deref() {
            register(id, &item.key);
        }
        register(&item.base, &item.key);
        register(&item.display_name, &item.key);
    }
    registry
}

fn resolve_mod_id(
    mod_id: &str,
    registry: &HashMap<String, String>,
    mods: &[ModRef],
) -> Option<String> {
    let lower = mod_id.to_ascii_lowercase();
    if let Some(key) = registry.get(&lower) {
        return Some(key.clone());
    }
    let needle = normalize_token(mod_id);
    if needle.is_empty() {
        return None;
    }
    if let Some(key) = registry.get(&needle) {
        return Some(key.clone());
    }
    for (token, key) in registry {
        if token.contains(&needle) || needle.contains(token.as_str()) {
            return Some(key.clone());
        }
    }
    for item in mods {
        let hay = normalize_token(&format!("{} {}", item.base, item.display_name));
        if hay.contains(&needle) {
            return Some(item.key.clone());
        }
    }
    None
}

fn load_cache(path: &Path) -> JarCacheFile {
    if !path.exists() {
        return JarCacheFile::default();
    }
    let Ok(text) = fs::read_to_string(path) else {
        return JarCacheFile::default();
    };
    let Ok(raw) = serde_json::from_str::<JarCacheFile>(&text) else {
        return JarCacheFile::default();
    };
    if raw.version != jar_cache_version() {
        return JarCacheFile::default();
    }
    raw
}

fn save_cache(path: &Path, cache: &JarCacheFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|e| e.to_string())
}

pub fn jar_info_for_mods(
    mods_dir: &Path,
    cache_path: &Path,
    refs: &[ModRef],
) -> Result<HashMap<String, JarInfo>, String> {
    let mut cache = load_cache(cache_path);
    let mut dirty = false;
    let mut result = HashMap::new();
    let mut mod_ids_by_key = HashMap::new();

    for item in refs {
        let jar_path = mods_dir.join(&item.filename);
        let mtime_ms = fs::metadata(&jar_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let entry = cache.entries.entry(item.filename.clone()).or_default();
        if entry.mtime_ms != mtime_ms
            || (entry.display_name.is_none()
                && entry.version.is_none()
                && entry.mod_id.is_none()
                && entry.dependency_mod_ids.is_empty())
        {
            entry.mtime_ms = mtime_ms;
            entry.display_name = None;
            entry.version = None;
            entry.mod_id = None;
            entry.dependency_mod_ids.clear();

            if let Some(toml) = read_jar_toml(&jar_path) {
                entry.display_name = parse_toml_display_name(&toml);
                entry.version = parse_toml_version(&toml);
                entry.mod_id = parse_own_mod_id(&toml);
                entry.dependency_mod_ids = parse_dependency_mod_ids(&toml);
            }
            if (entry.display_name.is_none() || entry.version.is_none() || entry.mod_id.is_none())
                && entry.dependency_mod_ids.is_empty()
            {
                if let Some(json) = read_jar_json_metadata(&jar_path) {
                    let (display_name, version, mod_id) = parse_json_metadata(&json);
                    if entry.display_name.is_none() {
                        entry.display_name = display_name;
                    }
                    if entry.version.is_none() {
                        entry.version = version;
                    }
                    if entry.mod_id.is_none() {
                        entry.mod_id = mod_id;
                    }
                }
            }
            dirty = true;
        }
        if let Some(mod_id) = entry.mod_id.as_deref() {
            mod_ids_by_key.insert(item.key.clone(), mod_id.to_string());
        }
    }

    let mut registry = build_registry(refs);
    for item in refs {
        let Some(mod_id) = mod_ids_by_key.get(&item.key) else {
            continue;
        };
        let lower = mod_id.trim().to_ascii_lowercase();
        if !lower.is_empty() {
            registry.insert(lower.clone(), item.key.clone());
        }
        let norm = normalize_token(&lower);
        if !norm.is_empty() {
            registry.insert(norm, item.key.clone());
        }
    }

    for item in refs {
        let entry = cache.entries.entry(item.filename.clone()).or_default();
        let keys: Vec<String> = entry
            .dependency_mod_ids
            .iter()
            .filter_map(|id| resolve_mod_id(id, &registry, refs))
            .filter(|k| k != &item.key)
            .collect();
        result.insert(
            item.key.clone(),
            JarInfo {
                display_name: entry.display_name.clone(),
                version: entry.version.clone(),
                dependency_keys: keys,
            },
        );
    }

    if dirty {
        cache.version = jar_cache_version();
        save_cache(cache_path, &cache)?;
    }
    Ok(result)
}
