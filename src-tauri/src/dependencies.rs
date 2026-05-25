use crate::jar_deps;
use crate::mods::{merge_keys, ModEntry};
use crate::settings::InstancePaths;

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
    let map = jar_deps::jar_info_for_mods(&paths.mods_dir, &cache_path, &refs)?;
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
