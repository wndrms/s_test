use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use lumos_domain::port::notification::{Notification, NotificationLevel, NotificationProvider};

const TELEGRAM_API_BASE: &str = "https://api.telegram.org/bot";

pub struct TelegramClient {
    bot_token: String,
    chat_id: String,
    http: Client,
}

impl TelegramClient {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            http: Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    description: Option<String>,
}

#[async_trait]
impl NotificationProvider for TelegramClient {
    async fn send(&self, notification: Notification) -> Result<()> {
        #[cfg(feature = "offline-fixtures")]
        {
            tracing::info!(
                "[MOCK Telegram] {} | {} — {}",
                level_emoji(&notification.level),
                notification.title,
                notification.body
            );
            return Ok(());
        }

        #[cfg(not(feature = "offline-fixtures"))]
        {
            #[cfg(not(feature = "online-telegram"))]
            bail!("online-telegram feature not enabled");

            #[cfg(feature = "online-telegram")]
            self.send_message(&notification).await
        }
    }
}

impl TelegramClient {
    #[allow(dead_code)]
    async fn send_message(&self, notification: &Notification) -> Result<()> {
        let text = format_message(notification);
        let url = format!("{}{}/sendMessage", TELEGRAM_API_BASE, self.bot_token);

        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "HTML"
            }))
            .send()
            .await
            .context("Telegram API request failed")?
            .json::<TelegramResponse>()
            .await
            .context("Telegram API parse failed")?;

        if !resp.ok {
            bail!(
                "Telegram API error: {}",
                resp.description.unwrap_or_else(|| "unknown".to_string())
            );
        }
        Ok(())
    }
}

fn format_message(n: &Notification) -> String {
    let emoji = level_emoji(&n.level);
    format!(
        "{} <b>{}</b>\n\n{}",
        emoji,
        html_escape(&n.title),
        html_escape(&n.body)
    )
}

fn level_emoji(level: &NotificationLevel) -> &'static str {
    match level {
        NotificationLevel::Info => "ℹ️",
        NotificationLevel::Warning => "⚠️",
        NotificationLevel::Alert => "🚨",
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_message_info() {
        let n = Notification::info("시나리오 생성", "삼성전자 시나리오가 생성되었습니다.");
        let msg = format_message(&n);
        assert!(msg.contains("ℹ️"));
        assert!(msg.contains("시나리오 생성"));
    }

    #[test]
    fn format_message_alert() {
        let n = Notification::alert("일 손실 한도 초과", "자동매매 일시정지됩니다.");
        let msg = format_message(&n);
        assert!(msg.contains("🚨"));
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("A & B < C > D"), "A &amp; B &lt; C &gt; D");
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn mock_send_does_not_error() {
        let client = TelegramClient::new("fake_token".to_string(), "123".to_string());
        let result = client.send(Notification::info("test", "body")).await;
        assert!(result.is_ok());
    }
}
