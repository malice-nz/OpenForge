use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackManifest {
    pub minecraft: Option<MinecraftBlock>,
    pub manifest_type: String,
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub files: Vec<ManifestFile>,
    pub overrides: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftBlock {
    pub version: String,
    pub mod_loaders: Vec<ModLoader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool { true }

pub fn parse_manifest(json: &str) -> Result<ModpackManifest, serde_json::Error> {
    serde_json::from_str(json)
}
