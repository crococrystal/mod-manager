use serde::Serialize;
use std::{collections::HashMap, fs, path::Path};

use crate::{
    mod_names::{loader_hint_from_filename, minecraft_version_hint_from_filename},
    settings::InstancePaths,
};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstanceTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader: Option<String>,
}

pub(crate) fn detect_instance_target(paths: &InstancePaths) -> InstanceTarget {
    let mut target = detect_from_mmc_pack(paths).unwrap_or_default();
    merge_target(&mut target, detect_from_instance_cfg(paths));

    if target.minecraft_version.is_none() {
        target.minecraft_version = detect_from_versions_dir(paths);
    }
    if target.minecraft_version.is_none() || target.loader.is_none() {
        let mut from_mods = InstanceTarget::default();
        for mods_dir in paths.all_mods_dirs() {
            merge_target(&mut from_mods, detect_from_mods_folder(mods_dir));
        }
        merge_target(&mut target, from_mods);
    }

    target
}

pub(crate) fn game_version_matches(candidate: &str, target: &str) -> bool {
    candidate.trim().eq_ignore_ascii_case(target.trim())
}

pub(crate) fn loader_matches(candidate: &str, target: &str) -> bool {
    candidate.trim().eq_ignore_ascii_case(target.trim())
}

pub(crate) fn version_matches_target(
    game_versions: &[String],
    loaders: &[String],
    target: &InstanceTarget,
) -> bool {
    if let Some(mc) = target
        .minecraft_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !game_versions
            .iter()
            .any(|version| game_version_matches(version, mc))
        {
            return false;
        }
    }
    if let Some(loader) = target
        .loader
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !loaders.iter().any(|value| loader_matches(value, loader)) {
            return false;
        }
    }
    true
}

pub(crate) fn target_has_filters(target: &InstanceTarget) -> bool {
    target
        .minecraft_version
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || target
            .loader
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
}

fn merge_target(target: &mut InstanceTarget, other: InstanceTarget) {
    if target.minecraft_version.is_none() {
        target.minecraft_version = other.minecraft_version;
    }
    if target.loader.is_none() {
        target.loader = other.loader;
    }
}

fn detect_from_mmc_pack(paths: &InstancePaths) -> Option<InstanceTarget> {
    let text = fs::read_to_string(paths.instance_root.join("mmc-pack.json")).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let components = value.get("components")?.as_array()?;
    let mut target = InstanceTarget::default();

    for component in components {
        let uid = component
            .get("uid")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let version = component
            .get("version")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        match uid {
            "net.minecraft" => target.minecraft_version = version,
            "net.neoforged" => target.loader = Some("neoforge".to_string()),
            "net.minecraftforge" => target.loader = Some("forge".to_string()),
            "net.fabricmc.fabric-loader" => target.loader = Some("fabric".to_string()),
            "org.quiltmc.quilt-loader" => target.loader = Some("quilt".to_string()),
            _ => {}
        }
    }

    (target.minecraft_version.is_some() || target.loader.is_some()).then_some(target)
}

fn detect_from_instance_cfg(paths: &InstancePaths) -> InstanceTarget {
    let text = fs::read_to_string(paths.instance_root.join("instance.cfg")).unwrap_or_default();
    let mut target = InstanceTarget::default();

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "IntendedVersion" && !value.trim().is_empty() {
            target.minecraft_version = Some(value.trim().to_string());
        }
        let lowered = value.to_ascii_lowercase();
        if lowered.contains("neoforge") {
            target.loader = Some("neoforge".to_string());
        } else if lowered.contains("forge") {
            target.loader = Some("forge".to_string());
        } else if lowered.contains("fabric") {
            target.loader = Some("fabric".to_string());
        } else if lowered.contains("quilt") {
            target.loader = Some("quilt".to_string());
        }
    }

    target
}

fn detect_from_versions_dir(paths: &InstancePaths) -> Option<String> {
    for versions_dir in [
        paths.instance_root.join("minecraft").join("versions"),
        paths.instance_root.join("versions"),
    ] {
        if let Some(version) = minecraft_version_from_versions_dir(&versions_dir) {
            return Some(version);
        }
    }
    None
}

fn minecraft_version_from_versions_dir(versions_dir: &Path) -> Option<String> {
    if !versions_dir.is_dir() {
        return None;
    }
    let mut versions = Vec::new();
    for entry in fs::read_dir(versions_dir).ok()?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("1.") {
            versions.push(name);
        }
    }
    if versions.len() == 1 {
        return versions.into_iter().next();
    }
    versions.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| right.cmp(left)));
    versions.into_iter().next()
}

fn detect_from_mods_folder(mods_dir: &Path) -> InstanceTarget {
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return InstanceTarget::default();
    };

    let mut mc_votes: HashMap<String, u32> = HashMap::new();
    let mut loader_votes: HashMap<String, u32> = HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jar") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if let Some(loader) = loader_hint_from_filename(name) {
            let weight = if name.to_ascii_lowercase().contains("-loader") {
                100
            } else {
                1
            };
            *loader_votes.entry(loader).or_insert(0) += weight;
        }
        if let Some(version) = minecraft_version_hint_from_filename(name) {
            *mc_votes.entry(version).or_insert(0) += 1;
        }
    }

    InstanceTarget {
        minecraft_version: pick_majority(&mc_votes, 3),
        loader: pick_majority(&loader_votes, 1),
    }
}

fn pick_majority(votes: &HashMap<String, u32>, min_votes: u32) -> Option<String> {
    let best = votes.iter().max_by(|left, right| {
        left.1
            .cmp(right.1)
            .then_with(|| left.0.len().cmp(&right.0.len()))
            .then_with(|| left.0.cmp(right.0))
    })?;
    (*best.1 >= min_votes).then(|| best.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_target_requires_both_filters() {
        let target = InstanceTarget {
            minecraft_version: Some("1.21.1".to_string()),
            loader: Some("neoforge".to_string()),
        };
        assert!(version_matches_target(
            &["1.21.1".to_string()],
            &["neoforge".to_string()],
            &target
        ));
        assert!(!version_matches_target(
            &["1.21.1".to_string()],
            &["forge".to_string()],
            &target
        ));
        assert!(!version_matches_target(
            &["1.21".to_string()],
            &["neoforge".to_string()],
            &target
        ));
    }
}
