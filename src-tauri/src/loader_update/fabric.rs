use super::neoforged;

const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2";

pub(crate) fn fetch_loader_versions_for_mc(
    client: &reqwest::blocking::Client,
    minecraft_version: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{FABRIC_META_BASE}/versions/loader/{minecraft_version}");
    let payload: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .map_err(|error| format!("Fabric Meta: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Fabric Meta: {error}"))?
        .json()
        .map_err(|error| format!("Fabric Meta: {error}"))?;

    let mut versions = Vec::new();
    for entry in payload {
        let Some(version) = entry
            .get("loader")
            .and_then(|loader| loader.get("version"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !versions.iter().any(|existing| existing == version) {
            versions.push(version.to_string());
        }
    }

    if versions.is_empty() {
        return Err(format!(
            "Не найдены версии Fabric Loader для Minecraft {minecraft_version}."
        ));
    }

    versions.sort_by(|left, right| neoforged::compare_versions(left, right));
    Ok(neoforged::versions_newest_first(&versions))
}

pub(crate) fn fetch_version_catalog(
    client: &reqwest::blocking::Client,
    minecraft_version: &str,
) -> Result<(String, Vec<String>), String> {
    let versions = fetch_loader_versions_for_mc(client, minecraft_version)?;
    let latest = versions
        .first()
        .cloned()
        .ok_or_else(|| "Не найдены версии Fabric Loader.".to_string())?;
    Ok((latest, versions))
}
