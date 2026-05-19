use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;

use lumos_domain::model::symbol::{Region, Symbol};
use lumos_domain::port::disclosure::{DisclosureItem, DisclosureProvider};

const SEC_SUBMISSIONS_BASE: &str = "https://data.sec.gov/submissions";
const SEC_EDGAR_BASE: &str = "https://www.sec.gov/cgi-bin/browse-edgar";

pub struct SecEdgarClient {
    http: Client,
    /// 요청 간 간격 (SEC는 초당 10회 제한)
    user_agent: String,
}

impl SecEdgarClient {
    pub fn new(user_agent: String) -> Self {
        Self {
            http: Client::new(),
            user_agent,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SecSubmissions {
    cik: String,
    #[serde(rename = "entityType")]
    entity_type: String,
    name: String,
    filings: SecFilings,
}

#[derive(Debug, Deserialize)]
struct SecFilings {
    recent: SecRecentFilings,
}

#[derive(Debug, Deserialize)]
struct SecRecentFilings {
    #[serde(rename = "accessionNumber")]
    accession_number: Vec<String>,
    #[serde(rename = "filingDate")]
    filing_date: Vec<String>,
    form: Vec<String>,
    #[serde(rename = "primaryDocument")]
    primary_document: Vec<String>,
    #[serde(rename = "primaryDocDescription")]
    primary_doc_description: Vec<String>,
}

#[async_trait]
impl DisclosureProvider for SecEdgarClient {
    async fn recent_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>> {
        if symbol.region != Region::Us {
            return Ok(vec![]);
        }

        #[cfg(feature = "offline-fixtures")]
        return Ok(mock_sec_filings(symbol));

        #[cfg(not(feature = "offline-fixtures"))]
        {
            #[cfg(not(feature = "online-sec"))]
            bail!("online-sec feature not enabled");

            #[cfg(feature = "online-sec")]
            self.fetch_filings(symbol).await
        }
    }
}

impl SecEdgarClient {
    #[allow(dead_code)]
    async fn fetch_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>> {
        let cik = cik_for_symbol(symbol)
            .ok_or_else(|| anyhow::anyhow!("CIK not found for {}", symbol.code))?;

        let url = format!("{}/CIK{}.json", SEC_SUBMISSIONS_BASE, cik_padded(&cik));
        let submissions: SecSubmissions = self
            .http
            .get(&url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await
            .context("SEC EDGAR request failed")?
            .json()
            .await
            .context("SEC EDGAR parse failed")?;

        let filings = &submissions.filings.recent;
        let corp_name = submissions.name.clone();

        let items: Vec<DisclosureItem> = filings
            .form
            .iter()
            .enumerate()
            .filter(|(_, form)| matches!(form.as_str(), "10-K" | "10-Q" | "8-K" | "6-K"))
            .take(10)
            .map(|(i, form)| {
                let filed_at = filings.filing_date.get(i)
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                    .unwrap_or_else(Utc::now);

                let accession = filings.accession_number.get(i).cloned().unwrap_or_default();
                let accession_clean = accession.replace('-', "");
                let doc = filings.primary_document.get(i).cloned().unwrap_or_default();
                let title = filings.primary_doc_description.get(i)
                    .cloned()
                    .unwrap_or_else(|| form.clone());

                let url = Some(format!(
                    "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                    cik, accession_clean, doc
                ));

                DisclosureItem {
                    title,
                    corp_name: corp_name.clone(),
                    filed_at,
                    doc_type: form.clone(),
                    url,
                }
            })
            .collect();

        Ok(items)
    }
}

fn cik_padded(cik: &str) -> String {
    format!("{:0>10}", cik)
}

fn cik_for_symbol(symbol: &Symbol) -> Option<String> {
    // 실제 구현에서는 DB의 symbol_identifiers 테이블(id_type='cik')에서 조회
    // 여기서는 잘 알려진 CIK 매핑만 하드코딩
    match symbol.code.as_str() {
        "AAPL" => Some("320193".to_string()),
        "MSFT" => Some("789019".to_string()),
        "GOOGL" | "GOOG" => Some("1652044".to_string()),
        "AMZN" => Some("1018724".to_string()),
        "NVDA" => Some("1045810".to_string()),
        "META" => Some("1326801".to_string()),
        "TSLA" => Some("1318605".to_string()),
        _ => None,
    }
}

#[cfg(feature = "offline-fixtures")]
fn mock_sec_filings(symbol: &Symbol) -> Vec<DisclosureItem> {
    vec![
        DisclosureItem {
            title: format!("[MOCK] {} Annual Report on Form 10-K", symbol.code),
            corp_name: symbol.name_en.clone().unwrap_or_else(|| symbol.code.clone()),
            filed_at: Utc::now(),
            doc_type: "10-K".to_string(),
            url: Some("https://www.sec.gov/".to_string()),
        },
        DisclosureItem {
            title: format!("[MOCK] {} Current Report on Form 8-K", symbol.code),
            corp_name: symbol.name_en.clone().unwrap_or_else(|| symbol.code.clone()),
            filed_at: Utc::now(),
            doc_type: "8-K".to_string(),
            url: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cik_padded_left_pads() {
        assert_eq!(cik_padded("320193"), "0000320193");
    }

    #[test]
    fn known_cik_lookup() {
        use uuid::Uuid;
        use chrono::Utc;
        use lumos_domain::model::symbol::{Currency, Region, Symbol};

        let sym = Symbol {
            id: Uuid::nil(),
            region: Region::Us,
            market: "NAS".to_string(),
            code: "AAPL".to_string(),
            display_code: "AAPL".to_string(),
            name_ko: None,
            name_en: Some("Apple Inc.".to_string()),
            currency: Currency::Usd,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(cik_for_symbol(&sym), Some("320193".to_string()));
    }
}
