use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationLevel {
    Info,
    Warning,
    Alert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub level: NotificationLevel,
    pub title: String,
    pub body: String,
    pub metadata: Option<serde_json::Value>,
}

impl Notification {
    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Info,
            title: title.into(),
            body: body.into(),
            metadata: None,
        }
    }

    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Warning,
            title: title.into(),
            body: body.into(),
            metadata: None,
        }
    }

    pub fn alert(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Alert,
            title: title.into(),
            body: body.into(),
            metadata: None,
        }
    }
}

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    async fn send(&self, notification: Notification) -> Result<()>;
}
