pub mod client;
pub mod dto;
pub mod paper_broker;

pub use client::{KisClient, KisEnvironment};
pub use paper_broker::PaperBroker;

use std::sync::Arc;
use lumos_domain::port::broker::Broker;

/// `.env` / 환경변수에서 KisClient 또는 PaperBroker를 생성합니다.
///
/// KIS_APP_KEY, KIS_APP_SECRET, KIS_ACCOUNT_NO 가 모두 설정된 경우 KisClient를 반환합니다.
/// KIS_ENV=real 이면 실거래, 그 외(또는 미설정)는 모의투자(Paper) 서버를 사용합니다.
/// 환경변수가 하나라도 없으면 PaperBroker(초기 자금 50,000,000 KRW)로 폴백합니다.
pub fn broker_from_env() -> Arc<dyn Broker> {
    let app_key = std::env::var("KIS_APP_KEY").unwrap_or_default();
    let app_secret = std::env::var("KIS_APP_SECRET").unwrap_or_default();
    let account_no = std::env::var("KIS_ACCOUNT_NO").unwrap_or_default();
    let account_product = std::env::var("KIS_ACCOUNT_PRODUCT").unwrap_or_else(|_| "01".to_string());

    if app_key.is_empty() || app_secret.is_empty() || account_no.is_empty() {
        tracing::warn!(
            "KIS_APP_KEY / KIS_APP_SECRET / KIS_ACCOUNT_NO not set — using PaperBroker"
        );
        let quotes = std::collections::HashMap::new();
        return Arc::new(PaperBroker::with_static_quotes(
            uuid::Uuid::nil(),
            rust_decimal_macros::dec!(50_000_000),
            lumos_domain::model::symbol::Currency::Krw,
            quotes,
        ));
    }

    let env = match std::env::var("KIS_ENV").as_deref() {
        Ok("real") => {
            tracing::info!("KIS broker mode: REAL");
            KisEnvironment::Real
        }
        _ => {
            tracing::info!("KIS broker mode: PAPER (모의투자)");
            KisEnvironment::Paper
        }
    };

    Arc::new(KisClient::new(
        env,
        app_key,
        app_secret,
        account_no,
        account_product,
    ))
}
