use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{ApiError, ApiResult, Backend};

pub const CFWIDGET_BASE: &str = "https://api.cfwidget.com";

pub struct CfWidget {
    client: Client,
}

impl CfWidget {
    pub fn new(client: Client) -> Self { Self { client } }
}

#[derive(Debug, Deserialize)]
pub struct WidgetProject {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub urls: serde_json::Value,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub files: Vec<WidgetFile>,
    #[serde(default)]
    pub download: serde_json::Value,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub game: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WidgetFile {
    pub id: u64,
    pub url: Option<String>,
    pub display: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub version: Option<String>,
    pub uploaded_at: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    pub downloads: Option<u64>,
}

impl CfWidget {
    pub async fn project(&self, id_or_slug: &str) -> ApiResult<WidgetProject> {
        let url = format!("{}/{}", CFWIDGET_BASE, id_or_slug);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Status(status.as_u16(), resp.text().await.unwrap_or_default()));
        }
        Ok(resp.json().await?)
    }
}

#[async_trait]
impl Backend for CfWidget {
    fn name(&self) -> &'static str { "cfwidget" }
}
