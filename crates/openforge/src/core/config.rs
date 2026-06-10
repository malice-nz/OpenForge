use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub eternal_api_key: Option<String>,
    pub instances_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub max_concurrent_downloads: Option<usize>,
}

impl Config {
    pub fn load_or_default() -> Self {
        let path = super::data_dir().join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }
}
