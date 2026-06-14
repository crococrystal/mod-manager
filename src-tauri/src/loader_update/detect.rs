use std::path::{Path, PathBuf};

use serde_json::Value;

use super::neoforged;

#[derive(Clone, Debug, Default)]
pub(crate) struct ClientLoaderInfo {
    pub minecraft_version: Option<String>,
    pub loader: String,
    pub loader_version: Option<String>,
}

pub(crate) fn detect_client(instance_root: &Path) -> ClientLoaderInfo {
    let mmc_path = instance_root.join("mmc-pack.json");
    let Ok(text) = std::fs::read_to_string(&mmc_path) else {
        return ClientLoaderInfo::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return ClientLoaderInfo::default();
    };
    let Some(components) = value.get("components").and_then(|item| item.as_array()) else {
        return ClientLoaderInfo::default();
    };

    let mut info = ClientLoaderInfo::default();
    for component in components {
        let uid = component
            .get("uid")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let version = component
            .get("version")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string);
        match uid {
            "net.minecraft" => info.minecraft_version = version,
            "net.neoforged" => {
                info.loader = "neoforge".to_string();
                info.loader_version = version;
            }
            "net.fabricmc.fabric-loader" => {
                info.loader = "fabric".to_string();
                info.loader_version = version;
            }
            _ => {}
        }
    }
    info
}

pub(crate) fn resolve_libraries_root(instance_root: &Path) -> PathBuf {
    if let Some(root) = prism_root_from_instance(instance_root) {
        let libraries = root.join("libraries");
        if libraries.is_dir() {
            return libraries;
        }
    }
    for candidate in default_prism_roots() {
        let libraries = candidate.join("libraries");
        if libraries.is_dir() {
            return libraries;
        }
    }
    instance_root
        .join("minecraft")
        .join("libraries")
}

fn prism_root_from_instance(instance_root: &Path) -> Option<PathBuf> {
    for ancestor in instance_root.ancestors() {
        if ancestor
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("instances"))
        {
            return ancestor.parent().map(Path::to_path_buf);
        }
        if ancestor
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("PrismLauncher"))
        {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn default_prism_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        roots.push(
            home.join("Library")
                .join("Application Support")
                .join("PrismLauncher"),
        );
        roots.push(home.join(".local").join("share").join("PrismLauncher"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        roots.push(PathBuf::from(appdata).join("PrismLauncher"));
    }
    roots
}

pub(crate) fn server_root_from_mods_path(server_mods_path: &str) -> Option<String> {
    let mut path = server_mods_path.trim().replace('\\', "/");
    while path.ends_with('/') {
        path.pop();
    }
    if path.is_empty() {
        return None;
    }
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("/mods") {
        path = path[..path.len() - 5].trim_end_matches('/').to_string();
    }
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

pub(crate) fn detect_server_version(host: &str, server_root: &str) -> Result<Option<String>, String> {
    let ps_root = server_root.replace('/', "\\");
    let cmd = format!(
        "powershell -NoProfile -Command \"\
         $p = Join-Path -Path '{ps_root}' -ChildPath 'libraries\\net\\neoforged\\neoforge'; \
         if (Test-Path -LiteralPath $p) {{ \
           Get-ChildItem -LiteralPath $p -Directory | Sort-Object Name -Descending | \
           Select-Object -First 1 -ExpandProperty Name \
         }}\""
    );
    let output = super::ssh::ssh_command(host, &cmd)?;
    if !output.status.success() {
        return Err(crate::ssh_util::ssh_command_failed(host, &output));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        Ok(None)
    } else {
        Ok(Some(version))
    }
}

pub(crate) fn needs_update(current: Option<&str>, latest: &str) -> bool {
    match current {
        Some(current) => neoforged::compare_versions(current, latest) == std::cmp::Ordering::Less,
        None => true,
    }
}
