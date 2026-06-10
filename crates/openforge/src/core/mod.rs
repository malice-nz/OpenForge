use std::path::{Path, PathBuf};

pub mod config;
pub mod error;
pub mod paths;

pub use error::{Error, Result};

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "OpenForge", "OpenForge")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./openforge-data"))
}

pub fn ensure_dir(p: &Path) -> Result<()> {
    if !p.exists() {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}
