use async_trait::async_trait;
use crate::models::{ModFile, ModInfo, Paginated};
use reqwest::Client;

use super::{
    cfwidget::CfWidget,
    eternal::Eternal,
    web::WebApi,
    ApiError, ApiResult, Backend, SearchQuery,
};

pub struct Aggregator {
    pub web: WebApi,
    pub widget: CfWidget,
    pub eternal: Option<Eternal>,
}

impl Aggregator {
    pub fn new(client: Client, eternal_key: Option<String>) -> Self {
        let web = WebApi::new(client.clone());
        let widget = CfWidget::new(client.clone());
        let eternal = eternal_key.map(|k| Eternal::new(client, k));
        Self { web, widget, eternal }
    }
}

#[async_trait]
impl Backend for Aggregator {
    fn name(&self) -> &'static str { "aggregator" }

    async fn search(&self, q: &SearchQuery) -> ApiResult<Paginated<ModInfo>> {
        if let Some(ref e) = self.eternal { return e.search(q).await; }
        Err(ApiError::Rejected("search requires Eternal API key (set eternal_api_key in config) or a cfwidget search route".into()))
    }

    async fn mod_info(&self, mod_id: u64) -> ApiResult<ModInfo> {
        if let Some(ref e) = self.eternal {
            if let Ok(v) = e.mod_info(mod_id).await { return Ok(v); }
        }
        let w = self.widget.project(&mod_id.to_string()).await?;
        Ok(ModInfo {
            id: w.id,
            game_id: 0,
            name: w.title,
            slug: String::new(),
            summary: w.summary,
            download_count: None,
            categories: vec![],
            latest_files: vec![],
            logo: None,
        })
    }

    async fn list_files(&self, mod_id: u64, index: u32, page_size: u32) -> ApiResult<Paginated<ModFile>> {
        self.web.list_files(mod_id, index, page_size).await
    }

    async fn file_info(&self, mod_id: u64, file_id: u64) -> ApiResult<ModFile> {
        if let Some(ref e) = self.eternal { return e.file_info(mod_id, file_id).await; }
        let p = self.web.list_files(mod_id, 0, 50).await?;
        p.data.into_iter().find(|f| f.id == file_id)
            .ok_or_else(|| ApiError::Rejected(format!("file {} not in first page; provide Eternal key for direct lookup", file_id)))
    }

    async fn download_url(&self, mod_id: u64, file_id: u64) -> ApiResult<String> {
        self.web.download_url(mod_id, file_id).await
    }
}
