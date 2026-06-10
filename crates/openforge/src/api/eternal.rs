use async_trait::async_trait;
use crate::models::{ModFile, ModInfo, Paginated};
use reqwest::Client;
use serde::Deserialize;

use super::{ApiError, ApiResult, Backend, SearchQuery};

pub const ETERNAL_BASE: &str = "https://api.curseforge.com/v1";

pub struct Eternal {
    client: Client,
    api_key: String,
}

impl Eternal {
    pub fn new(client: Client, api_key: String) -> Self { Self { client, api_key } }

    fn req(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .get(url)
            .header("x-api-key", &self.api_key)
            .header("Accept", "application/json")
    }
}

#[derive(Debug, Deserialize)]
struct EnvelopeOne<T> { data: T }

#[async_trait]
impl Backend for Eternal {
    fn name(&self) -> &'static str { "eternal" }

    async fn search(&self, q: &SearchQuery) -> ApiResult<Paginated<ModInfo>> {
        let mut url = format!("{}/mods/search?gameId={}&index={}&pageSize={}", ETERNAL_BASE, q.game_id, q.index, q.page_size);
        if let Some(ref s) = q.search { url.push_str(&format!("&searchFilter={}", urlencoding(s))); }
        if let Some(c) = q.category_id { url.push_str(&format!("&categoryId={}", c)); }
        if let Some(sf) = q.sort_field { url.push_str(&format!("&sortField={}", sf)); }
        if let Some(ml) = q.mod_loader { url.push_str(&format!("&modLoaderType={}", ml)); }
        if let Some(ref gv) = q.game_version { url.push_str(&format!("&gameVersion={}", urlencoding(gv))); }
        let resp = self.req(&url).send().await?;
        let s = resp.status();
        if !s.is_success() { return Err(ApiError::Status(s.as_u16(), resp.text().await.unwrap_or_default())); }
        Ok(resp.json().await?)
    }

    async fn mod_info(&self, mod_id: u64) -> ApiResult<ModInfo> {
        let url = format!("{}/mods/{}", ETERNAL_BASE, mod_id);
        let resp = self.req(&url).send().await?;
        let s = resp.status();
        if !s.is_success() { return Err(ApiError::Status(s.as_u16(), resp.text().await.unwrap_or_default())); }
        let env: EnvelopeOne<ModInfo> = resp.json().await?;
        Ok(env.data)
    }

    async fn file_info(&self, mod_id: u64, file_id: u64) -> ApiResult<ModFile> {
        let url = format!("{}/mods/{}/files/{}", ETERNAL_BASE, mod_id, file_id);
        let resp = self.req(&url).send().await?;
        let s = resp.status();
        if !s.is_success() { return Err(ApiError::Status(s.as_u16(), resp.text().await.unwrap_or_default())); }
        let env: EnvelopeOne<ModFile> = resp.json().await?;
        Ok(env.data)
    }

    async fn download_url(&self, mod_id: u64, file_id: u64) -> ApiResult<String> {
        let url = format!("{}/mods/{}/files/{}/download-url", ETERNAL_BASE, mod_id, file_id);
        let resp = self.req(&url).send().await?;
        let s = resp.status();
        if !s.is_success() { return Err(ApiError::Status(s.as_u16(), resp.text().await.unwrap_or_default())); }
        let env: EnvelopeOne<String> = resp.json().await?;
        Ok(env.data)
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
