use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledFile {
    pub project_id: u64,
    pub file_id: u64,
    pub file_name: String,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub bytes: u64,
    pub installed_at: chrono::DateTime<chrono::Utc>,
    pub path: PathBuf,
}

pub fn sha1_hex(buf: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(buf);
    hex::encode(h.finalize())
}

pub fn sha256_hex(buf: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(buf);
    hex::encode(h.finalize())
}

pub async fn hash_file_sha1<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(sha1_hex(&bytes))
}

pub async fn atomic_install_bytes<P: AsRef<Path>>(target: P, bytes: &[u8]) -> std::io::Result<()> {
    let target = target.as_ref();
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = target.with_extension("part");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, target).await?;
    Ok(())
}
