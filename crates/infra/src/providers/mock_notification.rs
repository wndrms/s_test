use anyhow::Result;
use async_trait::async_trait;

use lumos_domain::port::notification::{Notification, NotificationProvider};

pub struct MockNotificationProvider;

impl MockNotificationProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockNotificationProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationProvider for MockNotificationProvider {
    async fn send(&self, notification: Notification) -> Result<()> {
        tracing::info!(
            "[MOCK Notification] {} — {}",
            notification.title,
            notification.body
        );
        Ok(())
    }
}
