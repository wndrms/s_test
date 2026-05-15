use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

/// 네이버 금융 통합 API에서 가져오는 데이터
/// m.stock.naver.com/api/stock/{code}/integration (비공식)
#[derive(Debug, Clone)]
pub struct NaverFinanceData {
    /// 외국인 보유비율 (%)
    pub foreign_hold_rate: Option<f64>,
    /// 애널리스트 목표주가 컨센서스 (원)
    pub target_price: Option<f64>,
    /// PER (Price/Earnings Ratio)
    pub per: Option<f64>,
    /// PBR (Price/Book Ratio)
    pub pbr: Option<f64>,
    /// 당일 외국인 순매수량
    pub foreign_net_buy: Option<i64>,
    /// 당일 기관 순매수량
    pub institution_net_buy: Option<i64>,
}

pub struct NaverFinanceClient {
    http: Client,
}

impl NaverFinanceClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// 종목 통합 데이터 조회 (best-effort — 실패 시 None 필드로 반환)
    pub async fn fetch_integration(&self, symbol_code: &str) -> NaverFinanceData {
        #[cfg(feature = "offline-fixtures")]
        return mock_naver_finance(symbol_code);

        #[cfg(not(feature = "offline-fixtures"))]
        match self.fetch_integration_online(symbol_code).await {
            Ok(data) => data,
            Err(_) => NaverFinanceData {
                foreign_hold_rate: None,
                target_price: None,
                per: None,
                pbr: None,
                foreign_net_buy: None,
                institution_net_buy: None,
            },
        }
    }

    #[allow(dead_code)]
    async fn fetch_integration_online(&self, symbol_code: &str) -> Result<NaverFinanceData> {
        let url = format!(
            "https://m.stock.naver.com/api/stock/{}/integration",
            symbol_code
        );
        let resp: NaverIntegrationResponse = self.http.get(&url).send().await?.json().await?;

        let foreign_hold_rate = resp
            .stockInfo
            .as_ref()
            .and_then(|s| s.foreignRatio.as_ref())
            .and_then(|v| v.parse::<f64>().ok());

        let target_price = resp
            .consensus
            .as_ref()
            .and_then(|c| c.targetPrice.as_ref())
            .and_then(|v| v.parse::<f64>().ok());

        let per = resp
            .stockInfo
            .as_ref()
            .and_then(|s| s.per.as_ref())
            .and_then(|v| v.parse::<f64>().ok());

        let pbr = resp
            .stockInfo
            .as_ref()
            .and_then(|s| s.pbr.as_ref())
            .and_then(|v| v.parse::<f64>().ok());

        let foreign_net_buy = resp
            .investorInfo
            .as_ref()
            .and_then(|i| i.foreignNetBuy.as_ref())
            .and_then(|v| v.parse::<i64>().ok());

        let institution_net_buy = resp
            .investorInfo
            .as_ref()
            .and_then(|i| i.institutionNetBuy.as_ref())
            .and_then(|v| v.parse::<i64>().ok());

        Ok(NaverFinanceData {
            foreign_hold_rate,
            target_price,
            per,
            pbr,
            foreign_net_buy,
            institution_net_buy,
        })
    }
}

impl Default for NaverFinanceClient {
    fn default() -> Self {
        Self::new()
    }
}

// ─── API 응답 DTO ─────────────────────────────────────────────────────────────

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct NaverIntegrationResponse {
    stockInfo: Option<NaverStockInfo>,
    consensus: Option<NaverConsensus>,
    investorInfo: Option<NaverInvestorInfo>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct NaverStockInfo {
    foreignRatio: Option<String>,
    per: Option<String>,
    pbr: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct NaverConsensus {
    targetPrice: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct NaverInvestorInfo {
    foreignNetBuy: Option<String>,
    institutionNetBuy: Option<String>,
}

// ─── Fixture ─────────────────────────────────────────────────────────────────

#[cfg(feature = "offline-fixtures")]
fn mock_naver_finance(symbol_code: &str) -> NaverFinanceData {
    let _ = symbol_code;
    NaverFinanceData {
        foreign_hold_rate: Some(53.21),
        target_price: Some(85_000.0),
        per: Some(14.5),
        pbr: Some(1.8),
        foreign_net_buy: Some(30_000),
        institution_net_buy: Some(20_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_client_constructs() {
        let _client = NaverFinanceClient::new();
    }

    #[cfg(feature = "offline-fixtures")]
    #[tokio::test]
    async fn fixture_returns_data() {
        let client = NaverFinanceClient::new();
        let data = client.fetch_integration("005930").await;
        assert!(data.foreign_hold_rate.is_some());
        assert!(data.target_price.is_some());
        assert_eq!(data.per, Some(14.5));
    }
}
