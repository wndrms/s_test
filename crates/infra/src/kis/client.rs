use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

#[cfg(feature = "live-trading")]
use lumos_domain::model::broker::BrokerOrderStatus;
use lumos_domain::model::broker::{
    BrokerAccount, BrokerFill, BrokerOrderResponse, BrokerPosition, BuyingPower,
    BuyingPowerRequest, CancelOrderRequest, LimitOrderRequest, OrderFillQuery, OrderSide,
};
use lumos_domain::model::market::QuoteSnapshot;
use lumos_domain::model::symbol::Currency;
use lumos_domain::port::broker::Broker;

use super::dto::{
    DomesticBalancePosition, DomesticBalanceResponse, DomesticCancelOrderBody, DomesticOrderBody,
    DomesticQuoteResponse, InvestorFlowItem, InvestorFlowResponse, OrderFillOutput,
    OrderFillsResponse, OrderResponse, OrderResponseOutput, OverseasBalancePosition,
    OverseasBalanceResponse, OverseasOrderBody, OverseasQuoteResponse, TokenResponse,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KisEnvironment {
    Real,
    Paper,
}

impl KisEnvironment {
    pub fn base_url(&self) -> &'static str {
        match self {
            KisEnvironment::Real => "https://openapi.koreainvestment.com:9443",
            KisEnvironment::Paper => "https://openapivts.koreainvestment.com:29443",
        }
    }
}

pub struct KisClient {
    env: KisEnvironment,
    app_key: String,
    app_secret: String,
    account_no: String,
    account_product: String,
    access_token: tokio::sync::RwLock<Option<String>>,
    http: Client,
}

impl KisClient {
    pub fn new(
        env: KisEnvironment,
        app_key: String,
        app_secret: String,
        account_no: String,
        account_product: String,
    ) -> Self {
        Self {
            env,
            app_key,
            app_secret,
            account_no,
            account_product,
            access_token: tokio::sync::RwLock::new(None),
            http: Client::new(),
        }
    }

    pub async fn issue_access_token(&self) -> Result<String> {
        let url = format!("{}/oauth2/tokenP", self.env.base_url());
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": self.app_key,
            "appsecret": self.app_secret,
        });
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("KIS token request failed")?;
        
        let text = resp
            .text()
            .await
            .context("KIS token response body read failed")?;
        
        eprintln!("DEBUG KIS Token Response: {}", text);
        
        let token_resp: TokenResponse = serde_json::from_str(&text)
            .with_context(|| format!("KIS token parse failed: {}", text))?;
        
        let token = token_resp
            .get_token()
            .ok_or_else(|| anyhow::anyhow!("KIS token response missing access_token field"))?;
        
        *self.access_token.write().await = Some(token.clone());
        Ok(token)
    }

    async fn bearer_token(&self) -> Result<String> {
        let guard = self.access_token.read().await;
        guard
            .clone()
            .ok_or_else(|| anyhow::anyhow!("KIS access token not initialized"))
    }

    // ─── Quote ───────────────────────────────────────────────────────────────

    pub async fn domestic_quote(&self, symbol_code: &str) -> Result<QuoteSnapshot> {
        #[cfg(feature = "offline-fixtures")]
        return self.domestic_quote_fixture(symbol_code);
        #[cfg(not(feature = "offline-fixtures"))]
        self.domestic_quote_online(symbol_code).await
    }

    pub async fn overseas_quote(&self, symbol_code: &str, market: &str) -> Result<QuoteSnapshot> {
        #[cfg(feature = "offline-fixtures")]
        return self.overseas_quote_fixture(symbol_code, market);
        #[cfg(not(feature = "offline-fixtures"))]
        self.overseas_quote_online(symbol_code, market).await
    }

    #[cfg(feature = "offline-fixtures")]
    fn domestic_quote_fixture(&self, symbol_code: &str) -> Result<QuoteSnapshot> {
        let fixture_path = format!(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/kis/fixtures/domestic_quote_{}.json"
            ),
            symbol_code
        );
        let fallback = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/kis/fixtures/domestic_quote_sample.json"
        );
        let data = std::fs::read_to_string(&fixture_path)
            .or_else(|_| std::fs::read_to_string(fallback))
            .context("domestic quote fixture not found")?;
        let resp: DomesticQuoteResponse = serde_json::from_str(&data)?;
        parse_domestic_quote(&resp, symbol_code)
    }

    #[cfg(feature = "offline-fixtures")]
    fn overseas_quote_fixture(&self, symbol_code: &str, _market: &str) -> Result<QuoteSnapshot> {
        let fixture_path = format!(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/kis/fixtures/overseas_quote_{}.json"
            ),
            symbol_code
        );
        let fallback = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/kis/fixtures/overseas_quote_sample.json"
        );
        let data = std::fs::read_to_string(&fixture_path)
            .or_else(|_| std::fs::read_to_string(fallback))
            .context("overseas quote fixture not found")?;
        let resp: OverseasQuoteResponse = serde_json::from_str(&data)?;
        parse_overseas_quote(&resp, symbol_code)
    }

    #[allow(dead_code)]
    async fn domestic_quote_online(&self, symbol_code: &str) -> Result<QuoteSnapshot> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-price",
            self.env.base_url()
        );
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", "FHKST01010100")
            .query(&[
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", symbol_code),
            ])
            .send()
            .await?
            .json::<DomesticQuoteResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS domestic quote error: {} {}", resp.msg_cd, resp.msg1);
        }
        parse_domestic_quote(&resp, symbol_code)
    }

    #[allow(dead_code)]
    async fn overseas_quote_online(
        &self,
        symbol_code: &str,
        market: &str,
    ) -> Result<QuoteSnapshot> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/overseas-price/v1/quotations/price",
            self.env.base_url()
        );
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", "HHDFS00000300")
            .query(&[("AUTH", ""), ("EXCD", market), ("SYMB", symbol_code)])
            .send()
            .await?
            .json::<OverseasQuoteResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS overseas quote error: {} {}", resp.msg_cd, resp.msg1);
        }
        parse_overseas_quote(&resp, symbol_code)
    }

    // ─── Balance / Positions ─────────────────────────────────────────────────

    pub async fn domestic_balance(&self) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        #[cfg(feature = "offline-fixtures")]
        {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/kis/fixtures/domestic_balance_sample.json"
            );
            if std::path::Path::new(path).exists() {
                return self.domestic_balance_fixture();
            }
            // fallback to online when fixture is missing
            return self.domestic_balance_online().await;
        }
        #[cfg(not(feature = "offline-fixtures"))]
        self.domestic_balance_online().await
    }

    #[cfg(feature = "offline-fixtures")]
    fn domestic_balance_fixture(&self) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/kis/fixtures/domestic_balance_sample.json"
        );
        let data = std::fs::read_to_string(path).context("domestic balance fixture not found")?;
        let resp: DomesticBalanceResponse = serde_json::from_str(&data)?;
        parse_domestic_balance(&resp, Uuid::nil())
    }

    #[allow(dead_code)]
    async fn domestic_balance_online(&self) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-balance",
            self.env.base_url()
        );
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header(
                "tr_id",
                if self.env == KisEnvironment::Real {
                    "TTTC8434R"
                } else {
                    "VTTC8434R"
                },
            )
            .query(&[
                ("CANO", self.account_no.as_str()),
                ("ACNT_PRDT_CD", self.account_product.as_str()),
                ("AFHR_FLPR_YN", "N"),
                ("OFL_YN", ""),
                ("INQR_DVSN", "02"),
                ("UNPR_DVSN", "01"),
                ("FUND_STTL_ICLD_YN", "N"),
                ("FNCG_AMT_AUTO_RDPT_YN", "N"),
                ("PRCS_DVSN", "01"),
                ("CTX_AREA_FK100", ""),
                ("CTX_AREA_NK100", ""),
            ])
            .send()
            .await?
            .json::<DomesticBalanceResponse>()
            .await?;
        parse_domestic_balance(&resp, Uuid::nil())
    }

    // ─── Investor Flow (수급) ─────────────────────────────────────────────────

    /// 국내 종목 투자자별 순매수 (최근 N일, FHKST01010300)
    pub async fn domestic_investor_flow(
        &self,
        symbol_code: &str,
        days: u32,
    ) -> Result<Vec<InvestorFlowItem>> {
        #[cfg(feature = "offline-fixtures")]
        return Ok(mock_investor_flow(symbol_code, days));
        #[cfg(not(feature = "offline-fixtures"))]
        self.domestic_investor_flow_online(symbol_code, days).await
    }

    #[allow(dead_code)]
    async fn domestic_investor_flow_online(
        &self,
        symbol_code: &str,
        days: u32,
    ) -> Result<Vec<InvestorFlowItem>> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/inquire-investor",
            self.env.base_url()
        );
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", "FHKST01010300")
            .query(&[
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", symbol_code),
                ("FID_PERIOD_DIV_CODE", "D"),
                ("FID_INPUT_DATE_1", &days.to_string()),
            ])
            .send()
            .await?
            .json::<InvestorFlowResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS investor flow error: {} {}", resp.msg_cd, resp.msg1);
        }
        Ok(resp.output)
    }

    // ─── Overseas Balance / Positions ────────────────────────────────────────

    pub async fn overseas_balance(
        &self,
        exchange: &str,
    ) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        #[cfg(feature = "offline-fixtures")]
        {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/kis/fixtures/overseas_balance_sample.json"
            );
            if std::path::Path::new(path).exists() {
                return self.overseas_balance_fixture(exchange);
            }
            // fallback to online when fixture is missing
            return self.overseas_balance_online(exchange).await;
        }
        #[cfg(not(feature = "offline-fixtures"))]
        self.overseas_balance_online(exchange).await
    }

    #[cfg(feature = "offline-fixtures")]
    fn overseas_balance_fixture(
        &self,
        _exchange: &str,
    ) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/kis/fixtures/overseas_balance_sample.json"
        );
        let data = std::fs::read_to_string(path).context("overseas balance fixture not found")?;
        let resp: OverseasBalanceResponse = serde_json::from_str(&data)?;
        parse_overseas_balance(&resp, Uuid::nil())
    }

    #[allow(dead_code)]
    async fn overseas_balance_online(
        &self,
        exchange: &str,
    ) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
        let token = self.bearer_token().await?;
        let tr_id = if self.env == KisEnvironment::Real {
            "TTTS3012R"
        } else {
            "VTTS3012R"
        };
        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/inquire-balance",
            self.env.base_url()
        );
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", tr_id)
            .query(&[
                ("CANO", self.account_no.as_str()),
                ("ACNT_PRDT_CD", self.account_product.as_str()),
                ("OVRS_EXCG_CD", exchange),
                ("TR_CRCY_CD", exchange_to_currency(exchange)),
                ("CTX_AREA_FK200", ""),
                ("CTX_AREA_NK200", ""),
            ])
            .send()
            .await?
            .json::<OverseasBalanceResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS overseas balance error: {} {}", resp.msg_cd, resp.msg1);
        }
        parse_overseas_balance(&resp, Uuid::nil())
    }

    // ─── Overseas Limit Order ─────────────────────────────────────────────────

    #[cfg(feature = "live-trading")]
    pub async fn overseas_buy_limit_order(
        &self,
        req: &LimitOrderRequest,
    ) -> Result<BrokerOrderResponse> {
        self.overseas_limit_order(req, OrderSide::Buy).await
    }

    #[cfg(feature = "live-trading")]
    pub async fn overseas_sell_limit_order(
        &self,
        req: &LimitOrderRequest,
    ) -> Result<BrokerOrderResponse> {
        self.overseas_limit_order(req, OrderSide::Sell).await
    }

    #[cfg(feature = "live-trading")]
    async fn overseas_limit_order(
        &self,
        req: &LimitOrderRequest,
        side: OrderSide,
    ) -> Result<BrokerOrderResponse> {
        let token = self.bearer_token().await?;
        let exchange = req.market.as_deref().unwrap_or("NAS");
        let tr_id = match (&self.env, &side) {
            (KisEnvironment::Real, OrderSide::Buy) => "TTTT1002U",
            (KisEnvironment::Real, OrderSide::Sell) => "TTTT1006U",
            (KisEnvironment::Paper, OrderSide::Buy) => "VTTT1002U",
            (KisEnvironment::Paper, OrderSide::Sell) => "VTTT1006U",
        };
        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/order",
            self.env.base_url()
        );
        let body = OverseasOrderBody {
            CANO: self.account_no.clone(),
            ACNT_PRDT_CD: self.account_product.clone(),
            OVRS_EXCG_CD: exchange.to_string(),
            PDNO: req.symbol_code.clone(),
            ORD_DVSN: "00".to_string(),
            ORD_QTY: req.quantity.to_string(),
            OVRS_ORD_UNPR: req.limit_price.to_string(),
            ORD_SVR_DVSN_CD: "0".to_string(),
        };
        let resp = self
            .http
            .post(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", tr_id)
            .json(&body)
            .send()
            .await?
            .json::<OrderResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS overseas order error: {} {}", resp.msg_cd, resp.msg1);
        }
        let out = resp.output.unwrap_or_default();
        Ok(BrokerOrderResponse {
            external_order_id: out.ODNO,
            external_org_no: out.KRX_FWDG_ORD_ORGNO,
            status: BrokerOrderStatus::Submitted,
            submitted_at: Utc::now(),
        })
    }

    // ─── Order Fills ─────────────────────────────────────────────────────────

    pub async fn domestic_order_fills(
        &self,
        trading_date: NaiveDate,
        symbol_code: Option<&str>,
    ) -> Result<Vec<BrokerFill>> {
        #[cfg(feature = "offline-fixtures")]
        return self.order_fills_fixture();
        #[cfg(not(feature = "offline-fixtures"))]
        self.order_fills_online(trading_date, symbol_code).await
    }

    #[cfg(feature = "offline-fixtures")]
    fn order_fills_fixture(&self) -> Result<Vec<BrokerFill>> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/kis/fixtures/order_fills_sample.json"
        );
        let data = std::fs::read_to_string(path).context("order fills fixture not found")?;
        let resp: OrderFillsResponse = serde_json::from_str(&data)?;
        parse_order_fills(&resp)
    }

    #[allow(dead_code)]
    async fn order_fills_online(
        &self,
        trading_date: NaiveDate,
        symbol_code: Option<&str>,
    ) -> Result<Vec<BrokerFill>> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-daily-ccld",
            self.env.base_url()
        );
        let date_str = trading_date.format("%Y%m%d").to_string();
        let mut query = vec![
            ("CANO", self.account_no.as_str()),
            ("ACNT_PRDT_CD", self.account_product.as_str()),
            ("INQR_STRT_DT", date_str.as_str()),
            ("INQR_END_DT", date_str.as_str()),
            ("SLL_BUY_DVSN_CD", "00"),
            ("INQR_DVSN", "00"),
            ("PDNO", symbol_code.unwrap_or("")),
            ("CCLD_DVSN", "01"),
            ("ORD_GNO_BRNO", ""),
            ("ODNO", ""),
            ("INQR_DVSN_3", "00"),
            ("INQR_DVSN_1", ""),
            ("CTX_AREA_FK100", ""),
            ("CTX_AREA_NK100", ""),
        ];
        let resp = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header(
                "tr_id",
                if self.env == KisEnvironment::Real {
                    "TTTC8001R"
                } else {
                    "VTTC8001R"
                },
            )
            .query(&query)
            .send()
            .await?
            .json::<OrderFillsResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS order fills error: {} {}", resp.msg_cd, resp.msg1);
        }
        parse_order_fills(&resp)
    }

    // ─── Cancel Order ─────────────────────────────────────────────────────────

    #[cfg(feature = "live-trading")]
    pub async fn domestic_cancel_order(
        &self,
        req: &CancelOrderRequest,
    ) -> Result<BrokerOrderResponse> {
        let token = self.bearer_token().await?;
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/order-rvsecncl",
            self.env.base_url()
        );
        let tr_id = if self.env == KisEnvironment::Real {
            "TTTC0803U"
        } else {
            "VTTC0803U"
        };
        let body = DomesticCancelOrderBody {
            CANO: self.account_no.clone(),
            ACNT_PRDT_CD: self.account_product.clone(),
            KRX_FWDG_ORD_ORGNO: "".to_string(),
            ORGN_ODNO: req.external_order_id.clone(),
            ORD_DVSN: "00".to_string(),
            RVSE_CNCL_DVSN_CD: "02".to_string(),
            ORD_QTY: "0".to_string(),
            ORD_UNPR: "0".to_string(),
            QTY_ALL_ORD_YN: "Y".to_string(),
        };
        let resp = self
            .http
            .post(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", tr_id)
            .json(&body)
            .send()
            .await?
            .json::<OrderResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS cancel order error: {} {}", resp.msg_cd, resp.msg1);
        }
        let out = resp.output.unwrap_or_default();
        Ok(BrokerOrderResponse {
            external_order_id: out.ODNO,
            external_org_no: out.KRX_FWDG_ORD_ORGNO,
            status: BrokerOrderStatus::Canceled,
            submitted_at: Utc::now(),
        })
    }

    // ─── Limit Order ──────────────────────────────────────────────────────────

    #[cfg(feature = "live-trading")]
    pub async fn domestic_limit_order(
        &self,
        req: &LimitOrderRequest,
    ) -> Result<BrokerOrderResponse> {
        let token = self.bearer_token().await?;
        let tr_id = match (&self.env, &req.side) {
            (KisEnvironment::Real, OrderSide::Buy) => "TTTC0802U",
            (KisEnvironment::Real, OrderSide::Sell) => "TTTC0801U",
            (KisEnvironment::Paper, OrderSide::Buy) => "VTTC0802U",
            (KisEnvironment::Paper, OrderSide::Sell) => "VTTC0801U",
        };
        let url = format!(
            "{}/uapi/domestic-stock/v1/trading/order-cash",
            self.env.base_url()
        );
        let body = DomesticOrderBody {
            CANO: self.account_no.clone(),
            ACNT_PRDT_CD: self.account_product.clone(),
            PDNO: req.symbol_code.clone(),
            ORD_DVSN: "00".to_string(),
            ORD_QTY: req.quantity.to_string(),
            ORD_UNPR: req.limit_price.to_string(),
        };
        let resp = self
            .http
            .post(&url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .header("tr_id", tr_id)
            .json(&body)
            .send()
            .await?
            .json::<OrderResponse>()
            .await?;
        if resp.rt_cd != "0" {
            bail!("KIS order error: {} {}", resp.msg_cd, resp.msg1);
        }
        let out = resp.output.unwrap_or_default();
        Ok(BrokerOrderResponse {
            external_order_id: out.ODNO,
            external_org_no: out.KRX_FWDG_ORD_ORGNO,
            status: BrokerOrderStatus::Submitted,
            submitted_at: Utc::now(),
        })
    }
}

// ─── Broker trait implementation ──────────────────────────────────────────────

#[async_trait]
impl Broker for KisClient {
    async fn get_account(&self) -> Result<BrokerAccount> {
        let (account, _) = self.domestic_balance().await?;
        Ok(account)
    }

    async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
        let (_, positions) = self.domestic_balance().await?;
        Ok(positions)
    }

    async fn get_buying_power(&self, req: BuyingPowerRequest) -> Result<BuyingPower> {
        let (account, _) = self.domestic_balance().await?;
        let price = req.price;
        let max_quantity = if price > Decimal::ZERO {
            (account.cash / price).floor()
        } else {
            Decimal::ZERO
        };
        Ok(BuyingPower {
            max_quantity,
            available_cash: account.cash,
            currency: Currency::Krw,
        })
    }

    async fn place_limit_order(&self, req: LimitOrderRequest) -> Result<BrokerOrderResponse> {
        #[cfg(not(feature = "live-trading"))]
        bail!("live-trading feature is not enabled — compile with --features live-trading");

        #[cfg(feature = "live-trading")]
        self.domestic_limit_order(&req).await
    }

    async fn cancel_order(&self, req: CancelOrderRequest) -> Result<BrokerOrderResponse> {
        #[cfg(not(feature = "live-trading"))]
        bail!("live-trading feature is not enabled — compile with --features live-trading");

        #[cfg(feature = "live-trading")]
        self.domestic_cancel_order(&req).await
    }

    async fn get_order_fills(&self, req: OrderFillQuery) -> Result<Vec<BrokerFill>> {
        self.domestic_order_fills(req.trading_date, req.symbol_code.as_deref())
            .await
    }
}

// ─── Fixtures ────────────────────────────────────────────────────────────────

#[cfg(feature = "offline-fixtures")]
fn mock_investor_flow(symbol_code: &str, days: u32) -> Vec<InvestorFlowItem> {
    use chrono::{Duration, NaiveDate};
    let base = NaiveDate::from_ymd_opt(2024, 4, 1).unwrap();
    let count = days.min(30) as i64;
    (0..count)
        .map(|i| {
            let date = base + Duration::days(i);
            InvestorFlowItem {
                stck_bsop_date: date.format("%Y%m%d").to_string(),
                stck_clpr: "70000".to_string(),
                acml_vol: "15000000".to_string(),
                prsn_ntby_qty: "-50000".to_string(),
                frgn_ntby_qty: "30000".to_string(),
                orgn_ntby_qty: "20000".to_string(),
                frgn_hold_rate: format!("{:.2}", 53.0 + i as f64 * 0.01),
            }
        })
        .collect()
}

// ─── Parse helpers ───────────────────────────────────────────────────────────

fn parse_domestic_quote(resp: &DomesticQuoteResponse, _symbol_code: &str) -> Result<QuoteSnapshot> {
    let price = Decimal::from_str(&resp.output.stck_prpr)
        .with_context(|| format!("invalid domestic price: {}", resp.output.stck_prpr))?;
    let volume = Decimal::from_str(&resp.output.acml_vol).ok();
    Ok(QuoteSnapshot {
        id: Uuid::new_v4(),
        symbol_id: Uuid::nil(),
        source: "KIS".to_string(),
        last_price: price,
        bid: None,
        ask: None,
        volume,
        as_of: Utc::now(),
        fetched_at: Utc::now(),
    })
}

fn parse_overseas_quote(resp: &OverseasQuoteResponse, _symbol_code: &str) -> Result<QuoteSnapshot> {
    let price = Decimal::from_str(&resp.output.last)
        .with_context(|| format!("invalid overseas price: {}", resp.output.last))?;
    let volume = Decimal::from_str(&resp.output.tvol).ok();
    Ok(QuoteSnapshot {
        id: Uuid::new_v4(),
        symbol_id: Uuid::nil(),
        source: "KIS".to_string(),
        last_price: price,
        bid: None,
        ask: None,
        volume,
        as_of: Utc::now(),
        fetched_at: Utc::now(),
    })
}

fn parse_domestic_balance(
    resp: &DomesticBalanceResponse,
    broker_connection_id: Uuid,
) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
    let summary = resp
        .output2
        .first()
        .ok_or_else(|| anyhow::anyhow!("empty balance output2"))?;

    let equity = Decimal::from_str(&summary.nass_amt)
        .with_context(|| format!("invalid equity: {}", summary.nass_amt))?;
    let cash = Decimal::from_str(&summary.dnca_tot_amt)
        .with_context(|| format!("invalid cash: {}", summary.dnca_tot_amt))?;

    let account = BrokerAccount {
        broker_connection_id,
        total_equity: equity,
        cash,
        currency: Currency::Krw,
        as_of: Utc::now(),
    };

    let positions = resp
        .output1
        .iter()
        .filter_map(|p| parse_broker_position(p).ok())
        .collect();

    Ok((account, positions))
}

fn parse_broker_position(p: &DomesticBalancePosition) -> Result<BrokerPosition> {
    let quantity = Decimal::from_str(&p.hldg_qty)?;
    let avg_price = Decimal::from_str(&p.pchs_avg_pric)?;
    let current_price = Decimal::from_str(&p.prpr)?;
    let market_value = Decimal::from_str(&p.evlu_amt)?;
    let unrealized_pnl = Decimal::from_str(&p.evlu_pfls_amt)?;
    Ok(BrokerPosition {
        symbol_code: p.pdno.clone(),
        quantity,
        avg_price,
        current_price,
        market_value,
        unrealized_pnl,
    })
}

fn parse_overseas_balance(
    resp: &OverseasBalanceResponse,
    broker_connection_id: Uuid,
) -> Result<(BrokerAccount, Vec<BrokerPosition>)> {
    let summary = &resp.output2;
    let cash = Decimal::from_str(&summary.excc_amt)
        .with_context(|| format!("invalid overseas cash: {}", summary.excc_amt))?;
    let equity = Decimal::from_str(&summary.tot_asst_amt)
        .with_context(|| format!("invalid overseas equity: {}", summary.tot_asst_amt))?;

    let account = BrokerAccount {
        broker_connection_id,
        total_equity: equity,
        cash,
        currency: Currency::Usd,
        as_of: Utc::now(),
    };

    let positions = resp
        .output1
        .iter()
        .filter_map(|p| parse_overseas_position(p).ok())
        .collect();

    Ok((account, positions))
}

fn parse_overseas_position(p: &OverseasBalancePosition) -> Result<BrokerPosition> {
    let quantity = Decimal::from_str(&p.ovrs_cblc_qty)?;
    let avg_price = Decimal::from_str(&p.pchs_avg_pric)?;
    let current_price = Decimal::from_str(&p.now_pric2)?;
    let market_value = Decimal::from_str(&p.ovrs_stck_evlu_amt)?;
    let unrealized_pnl = Decimal::from_str(&p.frcr_evlu_pfls_amt)?;
    Ok(BrokerPosition {
        symbol_code: p.ovrs_pdno.clone(),
        quantity,
        avg_price,
        current_price,
        market_value,
        unrealized_pnl,
    })
}

fn exchange_to_currency(exchange: &str) -> &'static str {
    match exchange {
        "HKS" => "HKD",
        "TSE" => "JPY",
        "SHS" | "SZS" | "SHI" | "SZI" => "CNY",
        _ => "USD", // NAS, NYS, AMS, BAY, BAQ 등 미국 거래소
    }
}

fn parse_order_fills(resp: &OrderFillsResponse) -> Result<Vec<BrokerFill>> {
    resp.output1
        .iter()
        .filter(|f| f.cncl_yn == "N" && f.tot_ccld_qty != "0")
        .map(|f| {
            let quantity = Decimal::from_str(&f.tot_ccld_qty)?;
            let price = Decimal::from_str(&f.avg_prvs)?;
            let fee = Decimal::from_str(&f.cmsn_amt).unwrap_or(Decimal::ZERO);
            let tax = Decimal::from_str(&f.slng_tax_amt).unwrap_or(Decimal::ZERO);
            let side = if f.sll_buy_dvsn_cd == "01" {
                OrderSide::Sell
            } else {
                OrderSide::Buy
            };
            let filled_at = chrono::NaiveDateTime::parse_from_str(&f.ccld_dtm, "%Y%m%d%H%M%S")
                .map(|dt| dt.and_utc())
                .unwrap_or_else(|_| Utc::now());
            Ok(BrokerFill {
                external_order_id: f.odno.clone(),
                symbol_code: f.pdno.clone(),
                side,
                quantity,
                price,
                fee,
                tax,
                filled_at,
            })
        })
        .collect()
}

// Required for OrderResponseOutput default
impl Default for OrderResponseOutput {
    fn default() -> Self {
        Self {
            KRX_FWDG_ORD_ORGNO: None,
            ODNO: None,
            ORD_TMD: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> KisClient {
        KisClient::new(
            KisEnvironment::Paper,
            "test_app_key".to_string(),
            "test_app_secret".to_string(),
            "12345678".to_string(),
            "01".to_string(),
        )
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn domestic_quote_fixture_parses() {
        let client = make_client();
        let snapshot = client.domestic_quote("005930").await.unwrap();
        assert!(snapshot.last_price > Decimal::ZERO);
        assert_eq!(snapshot.source, "KIS");
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn domestic_balance_fixture_parses() {
        let client = make_client();
        let (account, positions) = client.domestic_balance().await.unwrap();
        assert!(account.cash > Decimal::ZERO);
        assert!(!positions.is_empty());
        assert_eq!(positions[0].symbol_code, "005930");
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn order_fills_fixture_parses() {
        let client = make_client();
        let fills = client
            .domestic_order_fills(chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), None)
            .await
            .unwrap();
        assert!(!fills.is_empty());
        assert_eq!(fills[0].side, OrderSide::Buy);
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn overseas_quote_fixture_parses() {
        let client = make_client();
        let snapshot = client.overseas_quote("AAPL", "NAS").await.unwrap();
        assert!(snapshot.last_price > Decimal::ZERO);
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn get_positions_returns_positions() {
        let client = make_client();
        let positions = client.get_positions().await.unwrap();
        assert!(!positions.is_empty());
        assert!(positions[0].quantity > Decimal::ZERO);
    }

    #[tokio::test]
    #[cfg(feature = "offline-fixtures")]
    async fn get_buying_power_calculates_max_qty() {
        let client = make_client();
        let bp = client
            .get_buying_power(BuyingPowerRequest {
                symbol_code: "005930".to_string(),
                price: Decimal::from(50000u32),
            })
            .await
            .unwrap();
        // cash = 5_000_000 / 50_000 = 100
        assert_eq!(bp.max_quantity, Decimal::from(100u32));
    }
}
