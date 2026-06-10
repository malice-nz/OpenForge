use async_trait::async_trait;
use crate::models::{ModFile, Paginated};
use reqwest::{Client, StatusCode};

use super::{ApiError, ApiResult, Backend};

pub const WEB_API_BASE: &str = "https://www.curseforge.com/api/v1";

pub struct WebApi {
    client: Client,
}

impl WebApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Backend for WebApi {
    fn name(&self) -> &'static str { "web-api" }

    async fn list_files(&self, mod_id: u64, index: u32, page_size: u32) -> ApiResult<Paginated<ModFile>> {
        let url = format!(
            "{base}/mods/{mod_id}/files?pageIndex={index}&pageSize={page_size}",
            base = WEB_API_BASE
        );
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::Status(status.as_u16(), body));
        }
        let body = resp.text().await?;
        let parsed: Paginated<ModFile> = serde_json::from_str(&body)?;
        Ok(parsed)
    }

    async fn download_url(&self, mod_id: u64, file_id: u64) -> ApiResult<String> {
        let url = format!(
            "{base}/mods/{mod_id}/files/{file_id}/download",
            base = WEB_API_BASE
        );
        let resp = self.client
            .get(&url)
            .send()
            .await?;

        if resp.status() == StatusCode::TEMPORARY_REDIRECT || resp.status() == StatusCode::FOUND || resp.status() == StatusCode::MOVED_PERMANENTLY {
            if let Some(loc) = resp.headers().get(reqwest::header::LOCATION) {
                return Ok(loc.to_str().map_err(|e| ApiError::Rejected(e.to_string()))?.to_string());
            }
        }
        if resp.status().is_success() {
            return Ok(resp.url().to_string());
        }
        let s = resp.status().as_u16();
        Err(ApiError::Status(s, resp.text().await.unwrap_or_default()))
    }
}
