use async_trait::async_trait;
use crate::models::{ModFile, ModInfo, Paginated};

pub mod web;
pub mod cfwidget;
pub mod eternal;
pub mod forgecdn;
pub mod aggregator;

pub use aggregator::Aggregator;
pub use forgecdn::{ForgeCdn, FORGECDN_API_KEY};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("status {0}: {1}")]
    Status(u16, String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("backend rejected request: {0}")]
    Rejected(String),
    #[error("not implemented for this backend")]
    NotImplemented,
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub game_id: u64,
    pub category_id: Option<u64>,
    pub search: Option<String>,
    pub sort_field: Option<u8>,
    pub mod_loader: Option<u8>,
    pub game_version: Option<String>,
    pub index: u32,
    pub page_size: u32,
}

#[async_trait]
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    async fn search(&self, q: &SearchQuery) -> ApiResult<Paginated<ModInfo>> {
        let _ = q;
        Err(ApiError::NotImplemented)
    }

    async fn mod_info(&self, mod_id: u64) -> ApiResult<ModInfo> {
        let _ = mod_id;
        Err(ApiError::NotImplemented)
    }

    async fn list_files(&self, mod_id: u64, index: u32, page_size: u32) -> ApiResult<Paginated<ModFile>> {
        let _ = (mod_id, index, page_size);
        Err(ApiError::NotImplemented)
    }

    async fn file_info(&self, mod_id: u64, file_id: u64) -> ApiResult<ModFile> {
        let _ = (mod_id, file_id);
        Err(ApiError::NotImplemented)
    }

    async fn download_url(&self, mod_id: u64, file_id: u64) -> ApiResult<String> {
        let _ = (mod_id, file_id);
        Err(ApiError::NotImplemented)
    }
}

pub fn default_user_agent() -> String {
    format!("OpenForge/{}", env!("CARGO_PKG_VERSION"))
}

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(default_user_agent())
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build()
        .expect("reqwest client")
}
