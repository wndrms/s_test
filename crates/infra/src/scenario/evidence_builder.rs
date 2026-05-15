use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use lumos_domain::model::market::QuoteSnapshot;
use lumos_domain::model::scenario::{EvidenceCard, EvidenceSourceType, SentimentLabel};
use lumos_domain::port::disclosure::DisclosureItem;
use lumos_domain::port::news::NewsItem;

use crate::kis::dto::InvestorFlowItem;
use crate::providers::naver_finance::NaverFinanceData;

/// 소스 유형별 신뢰도 기본값 (설계 문서 기준)
pub fn default_reliability(source_type: &EvidenceSourceType) -> Decimal {
    match source_type {
        EvidenceSourceType::Price | EvidenceSourceType::Technical => dec!(0.95),
        EvidenceSourceType::Disclosure | EvidenceSourceType::Financial => dec!(0.90),
        EvidenceSourceType::News => dec!(0.65),
        EvidenceSourceType::Community => dec!(0.35),
    }
}

pub fn from_quote(symbol_id: Uuid, snapshot: &QuoteSnapshot) -> EvidenceCard {
    let reliability = default_reliability(&EvidenceSourceType::Price);
    let title = format!("현재가 {}", snapshot.last_price);
    let summary = format!(
        "현재가: {} | 거래량: {} | 출처: {}",
        snapshot.last_price,
        snapshot.volume.unwrap_or(Decimal::ZERO),
        snapshot.source,
    );

    EvidenceCard {
        id: Uuid::new_v4(),
        symbol_id,
        source_type: EvidenceSourceType::Price,
        source_name: snapshot.source.clone(),
        source_ref_table: Some("quote_snapshots".to_string()),
        source_ref_id: Some(snapshot.id),
        title,
        summary,
        url: None,
        sentiment_label: None,
        importance_score: dec!(0.80),
        reliability_score: reliability,
        as_of: snapshot.as_of,
        fetched_at: snapshot.fetched_at,
        created_at: Utc::now(),
    }
}

pub fn from_news(symbol_id: Uuid, item: &NewsItem) -> EvidenceCard {
    let reliability = default_reliability(&EvidenceSourceType::News);
    EvidenceCard {
        id: Uuid::new_v4(),
        symbol_id,
        source_type: EvidenceSourceType::News,
        source_name: item.publisher.clone(),
        source_ref_table: None,
        source_ref_id: None,
        title: item.title.clone(),
        summary: item.snippet.clone().unwrap_or_else(|| item.title.clone()),
        url: Some(item.url.clone()),
        sentiment_label: None,
        importance_score: dec!(0.50),
        reliability_score: reliability,
        as_of: item.published_at,
        fetched_at: Utc::now(),
        created_at: Utc::now(),
    }
}

pub fn from_disclosure(symbol_id: Uuid, item: &DisclosureItem) -> EvidenceCard {
    let reliability = default_reliability(&EvidenceSourceType::Disclosure);
    let importance = match item.doc_type.as_str() {
        "10-K" | "10-Q" | "사업보고서" | "분기보고서" => dec!(0.85),
        "8-K" | "주요사항보고서" => dec!(0.90),
        _ => dec!(0.60),
    };
    EvidenceCard {
        id: Uuid::new_v4(),
        symbol_id,
        source_type: EvidenceSourceType::Disclosure,
        source_name: item.corp_name.clone(),
        source_ref_table: None,
        source_ref_id: None,
        title: item.title.clone(),
        summary: format!("[{}] {} — {}", item.doc_type, item.corp_name, item.title),
        url: item.url.clone(),
        sentiment_label: None,
        importance_score: importance,
        reliability_score: reliability,
        as_of: item.filed_at,
        fetched_at: Utc::now(),
        created_at: Utc::now(),
    }
}

/// KIS 투자자 수급 데이터 → EvidenceCard (Technical 타입 재사용)
pub fn from_investor_flow(symbol_id: Uuid, items: &[InvestorFlowItem]) -> EvidenceCard {
    let reliability = default_reliability(&EvidenceSourceType::Technical);

    let (frgn_total, orgn_total, prsn_total) = items.iter().fold((0i64, 0i64, 0i64), |acc, it| {
        let f = it.frgn_ntby_qty.parse::<i64>().unwrap_or(0);
        let o = it.orgn_ntby_qty.parse::<i64>().unwrap_or(0);
        let p = it.prsn_ntby_qty.parse::<i64>().unwrap_or(0);
        (acc.0 + f, acc.1 + o, acc.2 + p)
    });

    let frgn_hold = items
        .last()
        .and_then(|it| it.frgn_hold_rate.parse::<f64>().ok())
        .map(|r| format!("{:.2}%", r))
        .unwrap_or_else(|| "N/A".to_string());

    let days = items.len();
    let summary = format!(
        "최근 {}일 수급 — 외국인 순매수: {}, 기관 순매수: {}, 개인 순매수: {} | 외국인 보유비율: {}",
        days, frgn_total, orgn_total, prsn_total, frgn_hold
    );

    let dominant = if frgn_total > 0 && frgn_total >= orgn_total {
        "외국인 매수 우위"
    } else if orgn_total > 0 && orgn_total > frgn_total {
        "기관 매수 우위"
    } else if prsn_total > 0 {
        "개인 매수 우위"
    } else {
        "매도 우위"
    };

    EvidenceCard {
        id: Uuid::new_v4(),
        symbol_id,
        source_type: EvidenceSourceType::Technical,
        source_name: "KIS-수급".to_string(),
        source_ref_table: None,
        source_ref_id: None,
        title: format!("투자자 수급 ({}) — {}", days, dominant),
        summary,
        url: None,
        sentiment_label: None,
        importance_score: dec!(0.75),
        reliability_score: reliability,
        as_of: Utc::now(),
        fetched_at: Utc::now(),
        created_at: Utc::now(),
    }
}

/// 네이버 금융 컨센서스 + 보조지표 → EvidenceCard (Financial 타입)
pub fn from_naver_consensus(symbol_id: Uuid, data: &NaverFinanceData) -> Option<EvidenceCard> {
    if data.target_price.is_none() && data.per.is_none() && data.foreign_hold_rate.is_none() {
        return None;
    }

    let reliability = default_reliability(&EvidenceSourceType::Financial);

    let target_str = data
        .target_price
        .map(|p| format!("{:.0}원", p))
        .unwrap_or_else(|| "N/A".to_string());
    let per_str = data
        .per
        .map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "N/A".to_string());
    let pbr_str = data
        .pbr
        .map(|v| format!("{:.2}", v))
        .unwrap_or_else(|| "N/A".to_string());
    let frgn_str = data
        .foreign_hold_rate
        .map(|v| format!("{:.2}%", v))
        .unwrap_or_else(|| "N/A".to_string());

    let summary = format!(
        "목표주가 컨센서스: {} | PER: {} | PBR: {} | 외국인 보유비율: {}",
        target_str, per_str, pbr_str, frgn_str
    );

    Some(EvidenceCard {
        id: Uuid::new_v4(),
        symbol_id,
        source_type: EvidenceSourceType::Financial,
        source_name: "Naver-컨센서스".to_string(),
        source_ref_table: None,
        source_ref_id: None,
        title: format!("애널리스트 컨센서스 — 목표가 {}", target_str),
        summary,
        url: None,
        sentiment_label: None,
        importance_score: dec!(0.70),
        reliability_score: dec!(0.60),
        as_of: Utc::now(),
        fetched_at: Utc::now(),
        created_at: Utc::now(),
    })
}

pub fn with_sentiment(mut card: EvidenceCard, label: SentimentLabel) -> EvidenceCard {
    card.sentiment_label = Some(label);
    card
}

pub fn with_importance(mut card: EvidenceCard, score: Decimal) -> EvidenceCard {
    card.importance_score = score;
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dummy_quote(symbol_id: Uuid) -> QuoteSnapshot {
        QuoteSnapshot {
            id: Uuid::new_v4(),
            symbol_id,
            source: "KIS".to_string(),
            last_price: dec!(75400),
            bid: None,
            ask: None,
            volume: Some(dec!(1234567)),
            as_of: Utc::now(),
            fetched_at: Utc::now(),
        }
    }

    #[test]
    fn quote_card_has_correct_reliability() {
        let symbol_id = Uuid::new_v4();
        let q = dummy_quote(symbol_id);
        let card = from_quote(symbol_id, &q);
        assert_eq!(card.reliability_score, dec!(0.95));
        assert_eq!(card.source_type, EvidenceSourceType::Price);
        assert_eq!(card.symbol_id, symbol_id);
    }

    #[test]
    fn news_card_has_lower_reliability() {
        let symbol_id = Uuid::new_v4();
        let item = NewsItem {
            title: "삼성전자 실적 발표".to_string(),
            url: "https://example.com".to_string(),
            publisher: "연합뉴스".to_string(),
            published_at: Utc::now(),
            snippet: Some("삼성전자가 3분기 실적을 발표했다.".to_string()),
        };
        let card = from_news(symbol_id, &item);
        assert_eq!(card.reliability_score, dec!(0.65));
        assert_eq!(card.source_type, EvidenceSourceType::News);
    }

    #[test]
    fn disclosure_major_has_high_importance() {
        let symbol_id = Uuid::new_v4();
        let item = DisclosureItem {
            title: "8-K 공시".to_string(),
            corp_name: "Apple Inc.".to_string(),
            filed_at: Utc::now(),
            doc_type: "8-K".to_string(),
            url: None,
        };
        let card = from_disclosure(symbol_id, &item);
        assert_eq!(card.importance_score, dec!(0.90));
        assert_eq!(card.reliability_score, dec!(0.90));
    }
}
