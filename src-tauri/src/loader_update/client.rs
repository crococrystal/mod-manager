use std::path::Path;
use std::process::Command;

use chrono::Local;
use serde_json::Value;

use super::{cleanup, neoforged};

const LAUNCHER_PROFILES_STUB: &str = r#"{
  "profiles": {},
  "selectedProfile": "",
  "clientToken": "00000000000000000000000000000000",
  "authenticationDatabase": {},
  "settings": {}
}"#;

pub(crate) struct ClientApplyResult {
    pub downloaded_files: Vec<String>,
}

pub(crate) fn apply_client(
    client: &reqwest::blocking::Client,
    instance_root: &Path,
    libraries_root: &Path,
    target_version: &str,
) -> Result<ClientApplyResult, String> {
    update_mmc_pack(instance_root, target_version)?;
    let downloaded = materialize_client_artifacts(client, libraries_root, instance_root, target_version)?;
    Ok(ClientApplyResult { downloaded_files: downloaded })
}

fn materialize_client_artifacts(
    client: &reqwest::blocking::Client,
    libraries_root: &Path,
    instance_root: &Path,
    target_version: &str,
) -> Result<Vec<String>, String> {
    let version_dir = libraries_root
        .join("net")
        .join("neoforged")
        .join("neoforge")
        .join(target_version);
    std::fs::create_dir_all(&version_dir).map_err(|error| error.to_string())?;

    let mut downloaded = Vec::new();
    for artifact in ["universal", "installer"] {
        let filename = format!("neoforge-{target_version}-{artifact}.jar");
        let dest = version_dir.join(&filename);
        neoforged::download_http(client, &neoforged::artifact_url(target_version, artifact), &dest)?;
        downloaded.push(filename);
    }

    let client_filename = format!("neoforge-{target_version}-client.jar");
    let client_dest = version_dir.join(&client_filename);
    if client_dest.is_file() {
        downloaded.push(client_filename);
        return Ok(downloaded);
    }

    generate_client_jar(&version_dir, target_version)?;
    downloaded.push(client_filename);

    let removed = cleanup::remove_client_installers(libraries_root, instance_root)?;
    for name in removed {
        downloaded.push(format!("removed:{name}"));
    }

    Ok(downloaded)
}

fn generate_client_jar(version_dir: &Path, target_version: &str) -> Result<(), String> {
    let installer = version_dir.join(format!("neoforge-{target_version}-installer.jar"));
    if !installer.is_file() {
        return Err("Installer NeoForge не найден.".to_string());
    }

    let temp = std::env::temp_dir().join(format!(
        "mod-manager-neoforge-client-{}-{target_version}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp).map_err(|error| error.to_string())?;

    let result = (|| {
        std::fs::write(
            temp.join("launcher_profiles.json"),
            LAUNCHER_PROFILES_STUB,
        )
        .map_err(|error| error.to_string())?;

        let temp_installer = temp.join(installer.file_name().ok_or("installer")?);
        std::fs::copy(&installer, &temp_installer).map_err(|error| error.to_string())?;

        let output = Command::new("java")
            .arg("-jar")
            .arg(&temp_installer)
            .arg("--installClient")
            .arg(&temp)
            .output()
            .map_err(|error| format!("java: {error}"))?;

        if !output.status.success() {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
            .trim()
            .to_string();
            return Err(if combined.is_empty() {
                "NeoForge installer не смог создать client.jar.".to_string()
            } else if combined.chars().count() > 240 {
                format!(
                    "{}…",
                    combined.chars().take(240).collect::<String>()
                )
            } else {
                combined
            });
        }

        let generated = temp
            .join("libraries")
            .join("net")
            .join("neoforged")
            .join("neoforge")
            .join(target_version)
            .join(format!("neoforge-{target_version}-client.jar"));
        if !generated.is_file() {
            return Err("Installer завершился, но client.jar не найден.".to_string());
        }

        std::fs::copy(
            &generated,
            version_dir.join(format!("neoforge-{target_version}-client.jar")),
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&temp);
    result
}

fn update_mmc_pack(instance_root: &Path, target_version: &str) -> Result<(), String> {
    let path = instance_root.join("mmc-pack.json");
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut value: Value =
        serde_json::from_str(&text).map_err(|error| format!("mmc-pack.json: {error}"))?;
    let components = value
        .get_mut("components")
        .and_then(|item| item.as_array_mut())
        .ok_or_else(|| "В mmc-pack.json нет components.".to_string())?;

    let mut found = false;
    for component in components {
        if component
            .get("uid")
            .and_then(|item| item.as_str())
            .is_some_and(|uid| uid == "net.neoforged")
        {
            component["version"] = Value::String(target_version.to_string());
            if component.get("cachedVersion").is_some() {
                component["cachedVersion"] = Value::String(target_version.to_string());
            }
            found = true;
            break;
        }
    }
    if !found {
        return Err("В mmc-pack.json нет компонента net.neoforged.".to_string());
    }

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup = instance_root.join(format!("mmc-pack.json.backup-neoforge-{stamp}"));
    std::fs::copy(&path, &backup).map_err(|error| error.to_string())?;

    let updated = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    std::fs::write(&path, format!("{updated}\n")).map_err(|error| error.to_string())?;
    Ok(())
}
