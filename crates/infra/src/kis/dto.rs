use serde::{Deserialize, Serialize};

// ── Token ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub access_token_token_expired: Option<String>,
    // Error fields
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

impl TokenResponse {
    pub fn get_token(&self) -> Option<String> {
        self.access_token.clone()
    }
    
    pub fn is_error(&self) -> bool {
        self.error_code.is_some()
    }
    
    pub fn error_message(&self) -> Option<String> {
        if let (Some(code), Some(desc)) = (&self.error_code, &self.error_description) {
            Some(format!("{}: {}", code, desc))
        } else {
            self.error_description.clone()
        }
    }
}

// ── Domestic Quote (FHKST01010100) ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DomesticQuoteResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output: DomesticQuoteOutput,
}

#[derive(Debug, Deserialize)]
pub struct DomesticQuoteOutput {
    pub stck_prpr: String, // 주식 현재가
    pub stck_oprc: String, // 시가
    pub stck_hgpr: String, // 고가
    pub stck_lwpr: String, // 저가
    pub acml_vol: String,  // 누적 거래량
    pub prdy_vrss: String, // 전일 대비
    pub prdy_ctrt: String, // 전일 대비율
    pub hts_avls: String,  // HTS 시가총액
}

// ── Overseas Quote (HHDFS00000300) ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OverseasQuoteResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output: OverseasQuoteOutput,
}

#[derive(Debug, Deserialize)]
pub struct OverseasQuoteOutput {
    pub last: String, // 현재가
    pub open: String, // 시가
    pub high: String, // 고가
    pub low: String,  // 저가
    pub tvol: String, // 거래량
    pub diff: String, // 전일 대비
    pub rate: String, // 등락률
}

// ── Domestic Balance (TTTC8434R) ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DomesticBalanceResponse {
    pub rt_cd: String,
    pub output1: Vec<DomesticBalancePosition>,
    pub output2: Vec<DomesticBalanceSummary>,
}

#[derive(Debug, Deserialize)]
pub struct DomesticBalancePosition {
    pub pdno: String,          // 종목코드
    pub prdt_name: String,     // 종목명
    pub hldg_qty: String,      // 보유수량
    pub pchs_avg_pric: String, // 매입평균가
    pub prpr: String,          // 현재가
    pub evlu_amt: String,      // 평가금액
    pub evlu_pfls_amt: String, // 평가손익
}

#[derive(Debug, Deserialize)]
pub struct DomesticBalanceSummary {
    pub dnca_tot_amt: String,       // 예수금 총금액
    pub tot_evlu_amt: String,       // 총평가금액
    pub nass_amt: String,           // 순자산금액
    pub pchs_amt_smtl_amt: String,  // 매입금액합계
    pub evlu_pfls_smtl_amt: String, // 평가손익합계
}

// ── Investor Flow / 체결 수급 (FHKST01010300) ────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InvestorFlowResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output: Vec<InvestorFlowItem>,
}

#[derive(Debug, Deserialize)]
pub struct InvestorFlowItem {
    pub stck_bsop_date: String, // 영업일 YYYYMMDD
    pub stck_clpr: String,      // 종가
    pub acml_vol: String,       // 누적 거래량
    pub prsn_ntby_qty: String,  // 개인 순매수량
    pub frgn_ntby_qty: String,  // 외국인 순매수량
    pub orgn_ntby_qty: String,  // 기관 순매수량
    pub frgn_hold_rate: String, // 외국인 보유비율
}

// ── Cancel Order (TTTC0803U / VTTC0803U) ─────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DomesticCancelOrderBody {
    pub CANO: String,
    pub ACNT_PRDT_CD: String,
    pub KRX_FWDG_ORD_ORGNO: String,
    pub ORGN_ODNO: String,         // 원주문번호
    pub ORD_DVSN: String,          // 주문구분 (00: 지정가)
    pub RVSE_CNCL_DVSN_CD: String, // 02: 취소
    pub ORD_QTY: String,           // 취소 수량
    pub ORD_UNPR: String,          // 주문단가 (취소는 0)
    pub QTY_ALL_ORD_YN: String,    // Y: 전량취소
}

// ── Order Fills (TTTC8001R / VTTC8001R) ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OrderFillsResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output1: Vec<OrderFillOutput>,
}

#[derive(Debug, Deserialize)]
pub struct OrderFillOutput {
    pub odno: String,            // 주문번호
    pub sll_buy_dvsn_cd: String, // 01: 매도, 02: 매수
    pub pdno: String,            // 종목코드
    pub ord_qty: String,         // 주문수량
    pub avg_prvs: String,        // 체결평균가
    pub cncl_yn: String,         // 취소여부
    pub tot_ccld_qty: String,    // 총체결수량
    pub tot_ccld_amt: String,    // 총체결금액
    pub ccld_dtm: String,        // 체결일시 (YYYYMMDDHHmmss)
    pub brkr_bsns_asgn_no: String,
    pub ord_tmd: String,      // 주문시각
    pub cmsn_amt: String,     // 수수료
    pub slng_tax_amt: String, // 매도세금
}

// ── Overseas Balance (TTTS3012R / VTTS3012R) ─────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OverseasBalanceResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output1: Vec<OverseasBalancePosition>,
    pub output2: OverseasBalanceSummary,
}

#[derive(Debug, Deserialize)]
pub struct OverseasBalancePosition {
    pub ovrs_pdno: String,          // 해외 종목코드
    pub ovrs_item_name: String,     // 해외 종목명
    pub ovrs_cblc_qty: String,      // 해외 잔고 수량
    pub pchs_avg_pric: String,      // 매입평균가
    pub now_pric2: String,          // 현재가
    pub ovrs_stck_evlu_amt: String, // 해외주식평가금액
    pub frcr_evlu_pfls_amt: String, // 외화평가손익금액
    pub tr_crcy_cd: String,         // 거래통화코드
}

#[derive(Debug, Deserialize)]
pub struct OverseasBalanceSummary {
    pub frcr_pchs_amt1: String,    // 외화매입금액1
    pub ovrs_tot_pfls: String,     // 해외총손익
    pub tot_evlu_pfls_amt: String, // 총평가손익금액
    pub tot_asst_amt: String,      // 총자산금액
    pub excc_amt: String,          // 정산금액 (예수금)
}

// ── Overseas Order (TTTT1002U / VTTT1002U buy, TTTT1006U / VTTT1006U sell) ───

#[derive(Debug, Serialize)]
pub struct OverseasOrderBody {
    pub CANO: String,
    pub ACNT_PRDT_CD: String,
    pub OVRS_EXCG_CD: String, // 해외거래소코드 (NAS, NYS, AMS, HKS, TSE, SHS, SZS 등)
    pub PDNO: String,
    pub ORD_DVSN: String, // 00: 지정가, 32: LOC
    pub ORD_QTY: String,
    pub OVRS_ORD_UNPR: String,   // 주문단가
    pub ORD_SVR_DVSN_CD: String, // "0"
}

// ── Limit Order Request/Response ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DomesticOrderBody {
    pub CANO: String,         // 계좌번호 앞 8자리
    pub ACNT_PRDT_CD: String, // 계좌상품코드 뒤 2자리
    pub PDNO: String,         // 종목코드
    pub ORD_DVSN: String,     // 주문구분 (00: 지정가)
    pub ORD_QTY: String,      // 주문수량
    pub ORD_UNPR: String,     // 주문단가
}

#[derive(Debug, Deserialize)]
pub struct OrderResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output: Option<OrderResponseOutput>,
}

#[derive(Debug, Deserialize)]
pub struct OrderResponseOutput {
    pub KRX_FWDG_ORD_ORGNO: Option<String>, // 한국거래소전송주문조직번호
    pub ODNO: Option<String>,               // 주문번호
    pub ORD_TMD: Option<String>,            // 주문시각
}
