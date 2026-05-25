use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tauri::{AppHandle, Manager};

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    instance_root: Option<String>,
    #[serde(default)]
    curseforge_api_key: String,
    #[serde(default = "default_true")]
    auto_prefetch_covers: bool,
    #[serde(default = "default_true")]
    auto_prefetch_dependencies: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            instance_root: None,
            curseforge_api_key: String::new(),
            auto_prefetch_covers: true,
            auto_prefetch_dependencies: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    instance_root: Option<String>,
    mods_dir: Option<String>,
    data_root: Option<String>,
    curseforge_api_key: String,
    curseforge_api_key_set: bool,
    auto_prefetch_covers: bool,
    auto_prefetch_dependencies: bool,
}

#[derive(Clone, Debug)]
struct InstancePaths {
    mods_dir: PathBuf,
    index_dir: PathBuf,
    data_root: PathBuf,
    tags_path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagFile {
    #[serde(default = "tag_file_version")]
    version: u8,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    mods: HashMap<String, ModTags>,
}

fn tag_file_version() -> u8 {
    1
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModTags {
    #[serde(default)]
    side: String,
    #[serde(default)]
    library: bool,
    #[serde(default)]
    technical: bool,
    #[serde(default)]
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Clone, Debug)]
struct IndexInfo {
    index_file: String,
    slug: String,
    name: Option<String>,
    side: Option<String>,
    modrinth_id: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_id: Option<String>,
    curseforge_file_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModEntry {
    key: String,
    filename: String,
    base: String,
    display_name: String,
    side: String,
    library: bool,
    technical: bool,
    description: String,
    dependencies: Vec<String>,
    resolved_dependencies: Vec<String>,
    used_by: Vec<String>,
    source: String,
    source_url: Option<String>,
    has_index: bool,
    has_tags: bool,
    index_file: Option<String>,
    pack_side: Option<String>,
    modrinth_id: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_id: Option<String>,
    curseforge_file_id: Option<String>,
    duplicate: bool,
    modified_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModStats {
    total: usize,
    client: usize,
    universal: usize,
    server: usize,
    no_index: usize,
    tagged: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModListPayload {
    settings: SettingsView,
    mods: Vec<ModEntry>,
    stats: ModStats,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateModTagsRequest {
    key: String,
    side: Option<String>,
    library: Option<bool>,
    technical: Option<bool>,
    description: Option<String>,
    dependencies: Option<Vec<String>>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn system_time_iso(value: SystemTime) -> String {
    let dt: DateTime<Utc> = value.into();
    dt.to_rfc3339()
}

fn app_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("settings.json"))
}

fn read_settings(app: &AppHandle) -> Result<Settings, String> {
    let path = app_settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn write_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = app_settings_path(app)?;
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}

fn resolve_paths(settings: &Settings) -> Result<InstancePaths, String> {
    let selected = settings
        .instance_root
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "Выбери папку сборки в настройках.".to_string())?;

    let selected = selected
        .canonicalize()
        .unwrap_or_else(|_| selected.clone());

    let (instance_root, mods_dir) = if selected.join("minecraft").join("mods").is_dir() {
        (selected.clone(), selected.join("minecraft").join("mods"))
    } else if selected.join("mods").is_dir() {
        let instance = selected
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| selected.clone());
        (instance, selected.join("mods"))
    } else if selected.file_name().and_then(|name| name.to_str()) == Some("mods") {
        let instance = selected
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| selected.clone());
        (instance, selected.clone())
    } else {
        return Err("В выбранной папке не нашлась minecraft/mods.".to_string());
    };

    Ok(InstancePaths {
        index_dir: mods_dir.join(".index"),
        data_root: instance_root.join(".mod-manager"),
        tags_path: instance_root.join(".mod-manager").join("mod-tags.json"),
        mods_dir,
    })
}

fn settings_view(settings: Settings) -> SettingsView {
    let paths = resolve_paths(&settings).ok();
    SettingsView {
        instance_root: settings.instance_root.clone(),
        mods_dir: paths
            .as_ref()
            .map(|paths| paths.mods_dir.to_string_lossy().to_string()),
        data_root: paths
            .as_ref()
            .map(|paths| paths.data_root.to_string_lossy().to_string()),
        curseforge_api_key_set: !settings.curseforge_api_key.trim().is_empty(),
        curseforge_api_key: settings.curseforge_api_key.clone(),
        auto_prefetch_covers: settings.auto_prefetch_covers,
        auto_prefetch_dependencies: settings.auto_prefetch_dependencies,
    }
}

fn toml_value_at<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn toml_string(value: &toml::Value, path: &[&str]) -> Option<String> {
    let value = toml_value_at(value, path)?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_integer() {
        return Some(number.to_string());
    }
    None
}

fn read_index(index_dir: &Path) -> HashMap<String, IndexInfo> {
    let mut map = HashMap::new();
    let Ok(entries) = fs::read_dir(index_dir) else {
        return map;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".pw.toml") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = text.parse::<toml::Value>() else {
            continue;
        };
        let Some(filename) = toml_string(&value, &["filename"]) else {
            continue;
        };
        map.insert(
            filename,
            IndexInfo {
                index_file: file_name.to_string(),
                slug: file_name.trim_end_matches(".pw.toml").to_string(),
                name: toml_string(&value, &["name"]),
                side: toml_string(&value, &["side"]),
                modrinth_id: toml_string(&value, &["update", "modrinth", "mod-id"]),
                modrinth_version_id: toml_string(&value, &["update", "modrinth", "version"]),
                curseforge_id: toml_string(&value, &["update", "curseforge", "project-id"]),
                curseforge_file_id: toml_string(&value, &["update", "curseforge", "file-id"]),
            },
        );
    }

    map
}

fn slug_from_filename(filename: &str) -> String {
    let base = filename.trim_end_matches(".jar");
    let mut result = String::new();
    let mut previous_dash = false;

    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            result.push('-');
            previous_dash = true;
        }
    }

    result.trim_matches('-').to_string()
}

fn stable_key(filename: &str, info: Option<&IndexInfo>) -> String {
    if let Some(id) = info.and_then(|info| info.modrinth_id.as_ref()) {
        return format!("modrinth:{id}");
    }
    if let Some(id) = info.and_then(|info| info.curseforge_id.as_ref()) {
        return format!("curseforge:{id}");
    }
    format!("manual:{}", slug_from_filename(filename))
}

fn normalize_side(side: &str) -> String {
    match side {
        "client" => "client".to_string(),
        "server" => "server".to_string(),
        "universal" | "both" => "universal".to_string(),
        _ => "universal".to_string(),
    }
}

fn source_url(source: &str, info: Option<&IndexInfo>) -> Option<String> {
    match source {
        "modrinth" => info
            .and_then(|info| info.modrinth_id.as_ref())
            .map(|id| format!("https://modrinth.com/mod/{id}")),
        "curseforge" => info.map(|info| {
            format!(
                "https://www.curseforge.com/minecraft/mc-mods/{}",
                info.slug
            )
        }),
        _ => None,
    }
}

fn read_tags(path: &Path) -> Result<TagFile, String> {
    if !path.exists() {
        return Ok(TagFile {
            version: 1,
            updated_at: now_iso(),
            mods: HashMap::new(),
        });
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn write_tags(path: &Path, tags: &TagFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(tags).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}

fn merge_keys(left: &[String], right: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for key in left.iter().chain(right.iter()) {
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        result.push(key.clone());
    }
    result.sort();
    result
}

fn attach_used_by(mods: &mut [ModEntry]) {
    let known: HashSet<String> = mods.iter().map(|item| item.key.clone()).collect();
    let names: HashMap<String, String> = mods
        .iter()
        .map(|item| (item.key.clone(), item.display_name.clone()))
        .collect();
    let mut buckets: HashMap<String, Vec<String>> =
        mods.iter().map(|item| (item.key.clone(), Vec::new())).collect();

    for item in mods.iter() {
        for dependency in &item.resolved_dependencies {
            if dependency == &item.key || !known.contains(dependency) {
                continue;
            }
            let bucket = buckets.entry(dependency.clone()).or_default();
            if !bucket.contains(&item.key) {
                bucket.push(item.key.clone());
            }
        }
    }

    for item in mods.iter_mut() {
        let mut used = buckets.remove(&item.key).unwrap_or_default();
        used.sort_by(|a, b| {
            names
                .get(a)
                .unwrap_or(a)
                .cmp(names.get(b).unwrap_or(b))
        });
        item.used_by = used;
    }
}

fn scan_mods_for_settings(settings: &Settings) -> Result<Vec<ModEntry>, String> {
    let paths = resolve_paths(settings)?;
    let index = read_index(&paths.index_dir);
    let mut tags = read_tags(&paths.tags_path)?;
    let mut changed = false;
    let mut base_counts: HashMap<String, usize> = HashMap::new();
    let mut jars = Vec::new();

    let entries = fs::read_dir(&paths.mods_dir).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jar") {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let filename = filename.to_string();
        *base_counts.entry(filename.clone()).or_default() += 1;
        jars.push(filename);
    }

    jars.sort();
    let mut mods = Vec::with_capacity(jars.len());

    for filename in jars {
        let path = paths.mods_dir.join(&filename);
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        let info = index.get(&filename);
        let key = stable_key(&filename, info);
        let source = if info.and_then(|info| info.modrinth_id.as_ref()).is_some() {
            "modrinth"
        } else if info.and_then(|info| info.curseforge_id.as_ref()).is_some() {
            "curseforge"
        } else if info.is_some() {
            "index"
        } else {
            "manual"
        };

        if !tags.mods.contains_key(&key) {
            tags.mods.insert(
                key.clone(),
                ModTags {
                    side: info
                        .and_then(|info| info.side.as_deref())
                        .map(normalize_side)
                        .unwrap_or_else(|| "universal".to_string()),
                    aliases: vec![filename.clone()],
                    source: source.to_string(),
                    updated_at: now_iso(),
                    ..ModTags::default()
                },
            );
            changed = true;
        }

        let tag = tags.mods.get(&key).cloned().unwrap_or_default();
        let side = normalize_side(if tag.side.is_empty() {
            "universal"
        } else {
            tag.side.as_str()
        });
        let dependencies = tag.dependencies.clone();
        let resolved_dependencies = merge_keys(&dependencies, &[]);

        mods.push(ModEntry {
            key,
            filename: filename.clone(),
            base: filename.clone(),
            display_name: info
                .and_then(|info| info.name.clone())
                .unwrap_or_else(|| filename.trim_end_matches(".jar").to_string()),
            side,
            library: tag.library,
            technical: tag.technical,
            description: tag.description,
            dependencies,
            resolved_dependencies,
            used_by: Vec::new(),
            source: source.to_string(),
            source_url: source_url(source, info),
            has_index: info.is_some(),
            has_tags: true,
            index_file: info.map(|info| info.index_file.clone()),
            pack_side: info.and_then(|info| info.side.clone()),
            modrinth_id: info.and_then(|info| info.modrinth_id.clone()),
            modrinth_version_id: info.and_then(|info| info.modrinth_version_id.clone()),
            curseforge_id: info.and_then(|info| info.curseforge_id.clone()),
            curseforge_file_id: info.and_then(|info| info.curseforge_file_id.clone()),
            duplicate: base_counts.get(&filename).copied().unwrap_or_default() > 1,
            modified_at: metadata
                .modified()
                .map(system_time_iso)
                .unwrap_or_else(|_| now_iso()),
        });
    }

    if changed {
        tags.updated_at = now_iso();
        write_tags(&paths.tags_path, &tags)?;
    }

    attach_used_by(&mut mods);
    Ok(mods)
}

fn stats_for(mods: &[ModEntry]) -> ModStats {
    ModStats {
        total: mods.len(),
        client: mods.iter().filter(|item| item.side == "client").count(),
        universal: mods.iter().filter(|item| item.side == "universal").count(),
        server: mods.iter().filter(|item| item.side == "server").count(),
        no_index: mods.iter().filter(|item| !item.has_index).count(),
        tagged: mods.iter().filter(|item| item.has_tags).count(),
    }
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<SettingsView, String> {
    read_settings(&app).map(settings_view)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<SettingsView, String> {
    write_settings(&app, &settings)?;
    Ok(settings_view(settings))
}

#[tauri::command]
fn scan_mods(app: AppHandle) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let mods = scan_mods_for_settings(&settings)?;
    let stats = stats_for(&mods);
    Ok(ModListPayload {
        settings: settings_view(settings),
        mods,
        stats,
    })
}

#[tauri::command]
fn update_mod_tags(app: AppHandle, patch: UpdateModTagsRequest) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let mut tags = read_tags(&paths.tags_path)?;
    let current = tags.mods.entry(patch.key).or_default();

    if let Some(side) = patch.side {
        current.side = normalize_side(&side);
    }
    if let Some(library) = patch.library {
        current.library = library;
    }
    if let Some(technical) = patch.technical {
        current.technical = technical;
    }
    if let Some(description) = patch.description {
        current.description = description;
    }
    if let Some(dependencies) = patch.dependencies {
        current.dependencies = dependencies;
    }
    current.updated_at = now_iso();
    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)?;

    let mods = scan_mods_for_settings(&settings)?;
    let stats = stats_for(&mods);
    Ok(ModListPayload {
        settings: settings_view(settings),
        mods,
        stats,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            scan_mods,
            update_mod_tags
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
