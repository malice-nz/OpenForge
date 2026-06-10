use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game_slug: String,
    pub instance_dir: PathBuf,
    pub jvm_args: Vec<String>,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
}

pub fn launch(_profile: &Profile) -> anyhow::Result<()> {
    anyhow::bail!("launcher: not implemented in scaffold")
}
