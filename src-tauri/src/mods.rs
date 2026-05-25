use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::covers::apply_existing_cover;
use crate::dependencies::apply_jar_dependencies;
use crate::settings::{resolve_paths, Settings, SettingsView};
use crate::tags::{read_tags, write_tags, ModTags};
use crate::util::{now_iso, system_time_iso};

#[derive(Clone, Debug)]
pub(crate) struct IndexInfo {
    pub index_file: String,
    pub slug: String,
    pub name: Option<String>,
    pub side: Option<String>,
    pub modrinth_id: Option<String>,
    pub modrinth_version_id: Option<String>,
    pub curseforge_id: Option<String>,
    pub curseforge_file_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModEntry {
    pub key: String,
    pub filename: String,
    pub base: String,
    pub display_name: String,
    pub side: String,
    pub library: bool,
    pub technical: bool,
    pub description: String,
    pub dependencies: Vec<String>,
    pub resolved_dependencies: Vec<String>,
    pub jar_dependencies: Vec<String>,
    pub used_by: Vec<String>,
    pub cover_url: Option<String>,
    pub cover_path: Option<String>,
    pub cover_manual: bool,
    pub source: String,
    pub source_url: Option<String>,
    pub has_index: bool,
    pub has_tags: bool,
    pub index_file: Option<String>,
    pub pack_side: Option<String>,
    pub modrinth_id: Option<String>,
    pub modrinth_version_id: Option<String>,
    pub curseforge_id: Option<String>,
    pub curseforge_file_id: Option<String>,
    pub duplicate: bool,
    pub modified_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModStats {
    pub total: usize,
    pub client: usize,
    pub universal: usize,
    pub server: usize,
    pub no_index: usize,
    pub tagged: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModListPayload {
    pub settings: SettingsView,
    pub mods: Vec<ModEntry>,
    pub stats: ModStats,
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

pub(crate) fn stable_key(filename: &str, info: Option<&IndexInfo>) -> String {
    if let Some(id) = info.and_then(|info| info.modrinth_id.as_ref()) {
        return format!("modrinth:{id}");
    }
    if let Some(id) = info.and_then(|info| info.curseforge_id.as_ref()) {
        return format!("curseforge:{id}");
    }
    format!("manual:{}", slug_from_filename(filename))
}

pub(crate) fn normalize_side(side: &str) -> String {
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

pub(crate) fn merge_keys(lists: &[&[String]]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for list in lists {
        for key in *list {
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            result.push(key.clone());
        }
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
    let mut buckets: HashMap<String, Vec<String>> = mods
        .iter()
        .map(|item| (item.key.clone(), Vec::new()))
        .collect();

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

pub(crate) fn scan_mods_for_settings(
    settings: &Settings,
    catalog_root: Option<PathBuf>,
) -> Result<Vec<ModEntry>, String> {
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
        let resolved_dependencies = merge_keys(&[&dependencies, &[]]);

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
            jar_dependencies: Vec::new(),
            used_by: Vec::new(),
            cover_url: None,
            cover_path: None,
            cover_manual: false,
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

    apply_jar_dependencies(&mut mods, &paths)?;
    for item in mods.iter_mut() {
        apply_existing_cover(item, &paths, catalog_root.as_deref());
        item.resolved_dependencies = merge_keys(&[&item.dependencies, &item.jar_dependencies]);
    }
    attach_used_by(&mut mods);
    Ok(mods)
}

pub(crate) fn stats_for(mods: &[ModEntry]) -> ModStats {
    ModStats {
        total: mods.len(),
        client: mods.iter().filter(|item| item.side == "client").count(),
        universal: mods.iter().filter(|item| item.side == "universal").count(),
        server: mods.iter().filter(|item| item.side == "server").count(),
        no_index: mods.iter().filter(|item| !item.has_index).count(),
        tagged: mods.iter().filter(|item| item.has_tags).count(),
    }
}
