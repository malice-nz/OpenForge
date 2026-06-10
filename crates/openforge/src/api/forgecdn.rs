use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use std::path::Path;
use tokio::io::AsyncWriteExt;

use super::{ApiError, ApiResult};

pub const FORGECDN_BASE: &str = "https://edge.forgecdn.net";
pub const FORGECDN_API_KEY: &str = "267C6CA3";

pub struct ForgeCdn {
    client: Client,
}

impl ForgeCdn {
    pub fn new(client: Client) -> Self { Self { client } }

    pub fn url_for(file_id: u64, file_name: &str) -> String {
        let hi = file_id / 1000;
        let lo = file_id % 1000;
        format!("{}/files/{}/{:03}/{}?api-key={}", FORGECDN_BASE, hi, lo, file_name, FORGECDN_API_KEY)
    }

    pub async fn download_to<P: AsRef<Path>>(
        &self,
        url: &str,
        dest: P,
        expected_len: Option<u64>,
        mut progress: impl FnMut(u64, Option<u64>) + Send,
    ) -> ApiResult<u64> {
        let resp = self.client.get(url).send().await?;
        let s = resp.status();
        if !s.is_success() {
            return Err(ApiError::Status(s.as_u16(), resp.text().await.unwrap_or_default()));
        }
        let total = resp.content_length().or(expected_len);

        if let Some(parent) = dest.as_ref().parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Rejected(e.to_string()))?;
        }
        let mut file = tokio::fs::File::create(&dest).await.map_err(|e| ApiError::Rejected(e.to_string()))?;
        let mut written: u64 = 0;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk: Bytes = chunk?;
            file.write_all(&chunk).await.map_err(|e| ApiError::Rejected(e.to_string()))?;
            written += chunk.len() as u64;
            progress(written, total);
        }
        file.flush().await.map_err(|e| ApiError::Rejected(e.to_string()))?;
        if let Some(expected) = expected_len {
            if written != expected {
                return Err(ApiError::Rejected(format!("length mismatch: expected {}, got {}", expected, written)));
            }
        }
        Ok(written)
    }
}
