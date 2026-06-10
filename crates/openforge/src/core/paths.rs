use std::path::PathBuf;

pub fn instances_dir() -> PathBuf {
    super::data_dir().join("instances")
}

pub fn cache_dir() -> PathBuf {
    super::data_dir().join("cache")
}

pub fn downloads_dir() -> PathBuf {
    super::data_dir().join("downloads")
}
