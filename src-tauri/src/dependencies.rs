use crate::jar_deps;
use crate::mods::{merge_keys, ModEntry};
use crate::settings::InstancePaths;
use std::collections::{HashMap, HashSet};

pub(crate) fn apply_jar_dependencies(
    mods: &mut [ModEntry],
    paths: &InstancePaths,
) -> Result<(), String> {
    let refs: Vec<jar_deps::ModRef> = mods
        .iter()
        .map(|item| jar_deps::ModRef {
            key: item.key.clone(),
            filename: item.filename.clone(),
            display_name: item.display_name.clone(),
            base: item.base.clone(),
            modrinth_id: item.modrinth_id.clone(),
        })
        .collect();
    let cache_path = paths.data_root.join("cache").join("jar-dependencies.json");
    let map = jar_deps::jar_info_for_mods(
        |filename| paths.resolve_mod_jar(filename).unwrap_or_else(|| paths.mods_dir.join(filename)),
        &cache_path,
        &refs,
    )?;
    for item in mods.iter_mut() {
        if let Some(info) = map.get(&item.key) {
            if !item.display_name_locked {
                if let Some(display_name) = info.display_name.as_ref() {
                    item.display_name = display_name.clone();
                    item.display_name_locked = true;
                }
            }
            if let Some(version) = info.version.as_ref() {
                item.installed_version = Some(version.clone());
            }
            item.jar_dependencies = info.dependency_keys.clone();
        } else {
            item.jar_dependencies = Vec::new();
        }
        item.resolved_dependencies = merge_keys(&[&item.dependencies, &item.jar_dependencies]);
    }
    Ok(())
}

pub(crate) fn same_dependency_list(left: &[String], right: &[String]) -> bool {
    left == right
}

pub(crate) fn jar_dependencies_by_key(mods: &[ModEntry]) -> HashMap<String, HashSet<String>> {
    mods.iter()
        .map(|item| {
            (
                item.key.clone(),
                item.jar_dependencies.iter().cloned().collect(),
            )
        })
        .collect()
}

fn has_reverse_jar_dependency(
    source_key: &str,
    source_jar_dependencies: &[String],
    dependency_key: &str,
    jar_dependencies: &HashMap<String, HashSet<String>>,
) -> bool {
    if source_jar_dependencies
        .iter()
        .any(|key| key == dependency_key)
    {
        return false;
    }
    jar_dependencies
        .get(dependency_key)
        .is_some_and(|deps| deps.contains(source_key))
}

pub(crate) fn filter_reverse_jar_dependency_keys(
    source_key: &str,
    source_jar_dependencies: &[String],
    keys: &[String],
    jar_dependencies: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    keys.iter()
        .filter(|key| {
            !has_reverse_jar_dependency(source_key, source_jar_dependencies, key, jar_dependencies)
        })
        .cloned()
        .collect()
}

pub(crate) fn prune_reverse_jar_dependencies(mods: &mut [ModEntry]) -> bool {
    let jar_dependencies = jar_dependencies_by_key(mods);
    let mut changed = false;

    for item in mods.iter_mut() {
        let next = filter_reverse_jar_dependency_keys(
            &item.key,
            &item.jar_dependencies,
            &item.dependencies,
            &jar_dependencies,
        );
        if next != item.dependencies {
            item.dependencies = next;
            item.resolved_dependencies = merge_keys(&[&item.dependencies, &item.jar_dependencies]);
            changed = true;
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &[&str])]) -> HashMap<String, HashSet<String>> {
        entries
            .iter()
            .map(|(key, deps)| {
                (
                    (*key).to_string(),
                    deps.iter().map(|dep| (*dep).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn filters_provider_dependency_when_jar_points_back() {
        let jar_dependencies = map(&[("base", &[]), ("addon", &["base"])]);

        let filtered = filter_reverse_jar_dependency_keys(
            "base",
            &[],
            &["addon".to_string()],
            &jar_dependencies,
        );

        assert!(filtered.is_empty());
    }

    #[test]
    fn keeps_provider_dependency_when_source_jar_also_requires_it() {
        let jar_dependencies = map(&[("base", &["addon"]), ("addon", &["base"])]);

        let filtered = filter_reverse_jar_dependency_keys(
            "base",
            &["addon".to_string()],
            &["addon".to_string()],
            &jar_dependencies,
        );

        assert_eq!(filtered, vec!["addon"]);
    }
}
