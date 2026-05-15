use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsQuery {
    pub keyword: String,
    pub from: Option<DateTime<Utc>>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub url: String,
    pub publisher: String,
    pub published_at: DateTime<Utc>,
    pub snippet: Option<String>,
}

#[async_trait]
pub trait NewsProvider: Send + Sync {
    async fn search_news(&self, query: NewsQuery) -> Result<Vec<NewsItem>>;
}
