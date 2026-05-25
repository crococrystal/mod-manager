use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

use crate::util::now_iso;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagFile {
    #[serde(default = "tag_file_version")]
    pub version: u8,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub mods: HashMap<String, ModTags>,
}

fn tag_file_version() -> u8 {
    1
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModTags {
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub library: bool,
    #[serde(default)]
    pub technical: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub modrinth_id: String,
    #[serde(default)]
    pub modrinth_version_id: String,
    #[serde(default)]
    pub curseforge_id: String,
    #[serde(default)]
    pub curseforge_file_id: String,
    #[serde(default)]
    pub curseforge_slug: String,
    #[serde(default)]
    pub updated_at: String,
}

pub(crate) fn read_tags(path: &Path) -> Result<TagFile, String> {
    if !path.exists() {
        return Ok(TagFile {
            version: 1,
            updated_at: now_iso(),
            mods: HashMap::new(),
        });
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub(crate) fn write_tags(path: &Path, tags: &TagFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(tags).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}
