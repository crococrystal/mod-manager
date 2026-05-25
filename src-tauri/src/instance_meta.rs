use serde::Serialize;
use std::fs;

use crate::settings::InstancePaths;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstanceTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader: Option<String>,
}

pub(crate) fn detect_instance_target(paths: &InstancePaths) -> InstanceTarget {
    detect_from_mmc_pack(paths).unwrap_or_else(|| detect_from_instance_cfg(paths))
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
