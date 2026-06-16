use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;

use lumos_domain::model::symbol::Symbol;
use lumos_domain::port::disclosure::{DisclosureItem, DisclosureProvider};

const DART_API_BASE: &str = "https://opendart.fss.or.kr/api";

pub struct DartClient {
    api_key: String,
    http: Client,
}

impl DartClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key, http: Client::new() }
    }
}

#[derive(Debug, Deserialize)]
struct DartListResponse {
    status: String,
    message: String,
    list: Option<Vec<DartFilingItem>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // DART 응답 스키마 문서화: form_no는 현재 직접 읽지 않음
struct DartFilingItem {
    corp_name: String,
    report_nm: String,
    rcept_no: String,
    rcept_dt: String, // YYYYMMDD
    #[serde(rename = "form_no")]
    form_no: Option<String>,
}

#[async_trait]
impl DisclosureProvider for DartClient {
    async fn recent_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>> {
        #[cfg(feature = "offline-fixtures")]
        return Ok(mock_dart_filings(symbol));

        #[cfg(not(feature = "offline-fixtures"))]
        {
            #[cfg(not(feature = "online-opendart"))]
            bail!("online-opendart feature not enabled");

            #[cfg(feature = "online-opendart")]
            self.fetch_filings(symbol).await
        }
    }
}

impl DartClient {
    #[allow(dead_code)]
    async fn fetch_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>> {
        let corp_code = symbol
            .name_ko
            .as_deref()
            .unwrap_or(&symbol.code);

        let url = format!("{}/list.json", DART_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("crtfc_key", self.api_key.as_str()),
                ("corp_code", corp_code),
                ("bgn_de", &thirty_days_ago()),
                ("last_reprt_at", "N"),
                ("pblntf_ty", "A"),
                ("page_count", "20"),
            ])
            .send()
            .await
            .context("DART API request failed")?
            .json::<DartListResponse>()
            .await
            .context("DART API parse failed")?;

        if resp.status != "000" {
            bail!("DART API error: {} {}", resp.status, resp.message);
        }

        Ok(resp.list.unwrap_or_default().into_iter().map(|item| {
            let filed_at = parse_dart_date(&item.rcept_dt);
            let doc_type = classify_dart_doc(&item.report_nm);
            let url = Some(format!(
                "https://dart.fss.or.kr/dsaf001/main.do?rcpNo={}",
                item.rcept_no
            ));
            DisclosureItem {
                title: item.report_nm,
                corp_name: item.corp_name,
                filed_at,
                doc_type,
                url,
            }
        }).collect())
    }
}

fn parse_dart_date(s: &str) -> DateTime<Utc> {
    NaiveDate::parse_from_str(s, "%Y%m%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|_| Utc::now())
}

fn classify_dart_doc(report_nm: &str) -> String {
    if report_nm.contains("사업보고서") {
        "사업보고서".to_string()
    } else if report_nm.contains("분기보고서") {
        "분기보고서".to_string()
    } else if report_nm.contains("주요사항보고서") {
        "주요사항보고서".to_string()
    } else if report_nm.contains("반기보고서") {
        "반기보고서".to_string()
    } else {
        "기타".to_string()
    }
}

fn thirty_days_ago() -> String {
    (Utc::now() - chrono::Duration::days(30))
        .format("%Y%m%d")
        .to_string()
}

#[cfg(feature = "offline-fixtures")]
fn mock_dart_filings(symbol: &Symbol) -> Vec<DisclosureItem> {
    vec![
        DisclosureItem {
            title: format!("[MOCK] {} 분기보고서 (2024.3)", symbol.code),
            corp_name: symbol.name_ko.clone().unwrap_or_else(|| symbol.code.clone()),
            filed_at: Utc::now(),
            doc_type: "분기보고서".to_string(),
            url: Some("https://dart.fss.or.kr".to_string()),
        },
        DisclosureItem {
            title: format!("[MOCK] {} 주요사항보고서", symbol.code),
            corp_name: symbol.name_ko.clone().unwrap_or_else(|| symbol.code.clone()),
            filed_at: Utc::now(),
            doc_type: "주요사항보고서".to_string(),
            url: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_dart_doc_quarterly() {
        assert_eq!(classify_dart_doc("분기보고서 (2024.03)"), "분기보고서");
    }

    #[test]
    fn classify_dart_doc_material_event() {
        assert_eq!(classify_dart_doc("주요사항보고서"), "주요사항보고서");
    }

    #[test]
    fn classify_dart_doc_annual() {
        assert_eq!(classify_dart_doc("사업보고서 (2023.12)"), "사업보고서");
    }

    #[test]
    fn parse_dart_date_valid() {
        let dt = parse_dart_date("20240315");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-03-15");
    }
}
