use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use lumos_domain::port::news::{NewsItem, NewsProvider, NewsQuery};

const NAVER_SEARCH_BASE: &str = "https://openapi.naver.com/v1/search/news.json";

pub struct NaverNewsClient {
    client_id: String,
    client_secret: String,
    http: Client,
}

impl NaverNewsClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self { client_id, client_secret, http: Client::new() }
    }
}

#[derive(Debug, Deserialize)]
struct NaverNewsResponse {
    items: Vec<NaverNewsItem>,
}

#[derive(Debug, Deserialize)]
struct NaverNewsItem {
    title: String,
    link: String,
    description: String,
    #[serde(rename = "originallink")]
    original_link: String,
    #[serde(rename = "pubDate")]
    pub_date: String,
}

#[async_trait]
impl NewsProvider for NaverNewsClient {
    async fn search_news(&self, query: NewsQuery) -> Result<Vec<NewsItem>> {
        #[cfg(feature = "offline-fixtures")]
        return Ok(mock_naver_news(&query));

        #[cfg(not(feature = "offline-fixtures"))]
        {
            #[cfg(not(feature = "online-naver"))]
            bail!("online-naver feature not enabled");

            #[cfg(feature = "online-naver")]
            self.fetch_news(&query).await
        }
    }
}

impl NaverNewsClient {
    #[allow(dead_code)]
    async fn fetch_news(&self, query: &NewsQuery) -> Result<Vec<NewsItem>> {
        let resp = self
            .http
            .get(NAVER_SEARCH_BASE)
            .header("X-Naver-Client-Id", &self.client_id)
            .header("X-Naver-Client-Secret", &self.client_secret)
            .query(&[
                ("query", query.keyword.as_str()),
                ("display", &query.limit.to_string()),
                ("sort", "date"),
            ])
            .send()
            .await
            .context("Naver News API request failed")?
            .json::<NaverNewsResponse>()
            .await
            .context("Naver News API parse failed")?;

        Ok(resp.items.into_iter().map(|item| {
            let published_at = parse_rfc2822(&item.pub_date);
            let snippet = if item.description.is_empty() {
                None
            } else {
                Some(strip_html_tags(&item.description))
            };
            NewsItem {
                title: strip_html_tags(&item.title),
                url: item.original_link,
                publisher: extract_publisher(&item.link),
                published_at,
                snippet,
            }
        }).collect())
    }
}

fn parse_rfc2822(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc2822(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    // HTML 엔티티 간단 처리
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn extract_publisher(url: &str) -> String {
    url.split('/')
        .nth(2)
        .unwrap_or("unknown")
        .trim_start_matches("www.")
        .to_string()
}

#[cfg(feature = "offline-fixtures")]
fn mock_naver_news(query: &NewsQuery) -> Vec<NewsItem> {
    vec![
        NewsItem {
            title: format!("[MOCK] {} 관련 최신 뉴스", query.keyword),
            url: "https://n.news.naver.com/article/001/0001".to_string(),
            publisher: "연합뉴스".to_string(),
            published_at: Utc::now(),
            snippet: Some(format!("{} 관련 주요 이슈 요약", query.keyword)),
        },
        NewsItem {
            title: format!("[MOCK] {} 실적 발표 예정", query.keyword),
            url: "https://n.news.naver.com/article/001/0002".to_string(),
            publisher: "한국경제".to_string(),
            published_at: Utc::now() - chrono::Duration::hours(2),
            snippet: Some("다음 분기 실적 발표 예정".to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_removes_tags() {
        assert_eq!(strip_html_tags("<b>삼성전자</b> 실적"), "삼성전자 실적");
    }

    #[test]
    fn strip_html_handles_entities() {
        assert_eq!(strip_html_tags("A &amp; B"), "A & B");
    }

    #[test]
    fn extract_publisher_from_url() {
        assert_eq!(
            extract_publisher("https://n.news.naver.com/article/001/0001"),
            "n.news.naver.com"
        );
    }

    #[test]
    fn parse_rfc2822_valid() {
        let dt = parse_rfc2822("Mon, 01 Jan 2024 09:00:00 +0900");
        assert_eq!(dt.format("%Y").to_string(), "2024");
    }

    #[test]
    fn parse_rfc2822_invalid_falls_back() {
        let dt = parse_rfc2822("invalid");
        assert!(dt <= Utc::now());
    }
}
