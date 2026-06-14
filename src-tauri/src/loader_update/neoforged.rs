use std::cmp::Ordering;

pub(crate) const MAVEN_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

pub(crate) fn neoforge_prefix_for_mc(minecraft_version: &str) -> Option<String> {
    let parts: Vec<&str> = minecraft_version.trim().split('.').collect();
    match parts.as_slice() {
        ["1", minor, patch] => Some(format!("{minor}.{patch}")),
        ["1", minor] => Some(format!("{minor}.0")),
        _ => None,
    }
}

pub(crate) fn compare_versions(left: &str, right: &str) -> Ordering {
    let parse = |value: &str| -> Vec<u32> {
        value
            .split(|ch| ch == '.' || ch == '-')
            .filter_map(|part| {
                let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                digits.parse().ok()
            })
            .collect()
    };
    parse(left).cmp(&parse(right))
}

pub(crate) fn parse_maven_versions(xml: &str) -> Vec<String> {
    let mut versions = Vec::new();
    let mut in_versions = false;
    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed == "<versions>" {
            in_versions = true;
            continue;
        }
        if trimmed == "</versions>" {
            break;
        }
        if !in_versions {
            continue;
        }
        if let Some(inner) = trimmed
            .strip_prefix("<version>")
            .and_then(|rest| rest.strip_suffix("</version>"))
        {
            versions.push(inner.trim().to_string());
        }
    }
    versions
}

pub(crate) fn filter_versions_for_mc(all: &[String], minecraft_version: &str) -> Vec<String> {
    let Some(prefix) = neoforge_prefix_for_mc(minecraft_version) else {
        return Vec::new();
    };
    let needle = format!("{prefix}.");
    all.iter()
        .filter(|version| {
            version.starts_with(&needle)
                && !version.contains("beta")
                && !version.contains("alpha")
                && !version.contains("snapshot")
        })
        .cloned()
        .collect()
}

pub(crate) fn latest_version(versions: &[String]) -> Option<String> {
    versions
        .iter()
        .max_by(|left, right| compare_versions(left, right))
        .cloned()
}

pub(crate) fn installer_url(version: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
    )
}

pub(crate) fn artifact_url(version: &str, artifact: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-{artifact}.jar"
    )
}

pub(crate) fn fetch_versions_for_mc(
    client: &reqwest::blocking::Client,
    minecraft_version: &str,
) -> Result<Vec<String>, String> {
    let xml = client
        .get(MAVEN_METADATA_URL)
        .send()
        .map_err(|error| format!("Maven: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Maven: {error}"))?
        .text()
        .map_err(|error| format!("Maven: {error}"))?;
    let all = parse_maven_versions(&xml);
    let mut filtered = filter_versions_for_mc(&all, minecraft_version);
    if filtered.is_empty() {
        return Err(format!(
            "Не найдены версии NeoForge для Minecraft {minecraft_version}."
        ));
    }
    filtered.sort_by(|left, right| compare_versions(left, right));
    Ok(filtered)
}

pub(crate) fn fetch_latest_for_mc(
    client: &reqwest::blocking::Client,
    minecraft_version: &str,
) -> Result<String, String> {
    fetch_versions_for_mc(client, minecraft_version)?
        .into_iter()
        .max_by(|left, right| compare_versions(left, right))
        .ok_or_else(|| {
            format!(
                "Не найдены версии NeoForge для Minecraft {minecraft_version}."
            )
        })
}

pub(crate) fn versions_newest_first(versions: &[String]) -> Vec<String> {
    let mut ordered = versions.to_vec();
    ordered.sort_by(|left, right| compare_versions(right, left));
    ordered
}

pub(crate) fn download_http(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = client
        .get(url)
        .send()
        .map_err(|error| format!("Загрузка: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Загрузка: {error}"))?
        .bytes()
        .map_err(|error| format!("Загрузка: {error}"))?;
    std::fs::write(dest, bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_minecraft_to_neoforge_prefix() {
        assert_eq!(
            neoforge_prefix_for_mc("1.21.1").as_deref(),
            Some("21.1")
        );
    }

    #[test]
    fn compares_patch_versions() {
        assert_eq!(
            compare_versions("21.1.231", "21.1.233"),
            Ordering::Less
        );
    }
}
