use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::symbol::Symbol;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureItem {
    pub title: String,
    pub corp_name: String,
    pub filed_at: DateTime<Utc>,
    pub doc_type: String,
    pub url: Option<String>,
}

#[async_trait]
pub trait DisclosureProvider: Send + Sync {
    async fn recent_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>>;
}
