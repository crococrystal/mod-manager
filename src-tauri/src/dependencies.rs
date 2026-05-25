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
    let map = jar_deps::jar_deps_for_mods(&paths.mods_dir, &cache_path, &refs)?;
    for item in mods.iter_mut() {
        item.jar_dependencies = map.get(&item.key).cloned().unwrap_or_default();
        item.resolved_dependencies = merge_keys(&[&item.dependencies, &item.jar_dependencies]);
    }
    Ok(())
}

pub(crate) fn same_dependency_list(left: &[String], right: &[String]) -> bool {
    left == right
}
