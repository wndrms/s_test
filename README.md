# Lumos — AI 자동매매 시스템

한국투자증권 Open API 기반의 AI 자동매매 플랫폼입니다.  
LLM 멀티스텝 파이프라인으로 시나리오를 분석하고, Risk Gate를 통과한 주문만 자동 실행합니다.  
한국(KRX) + 미국 시장을 동시 지원하며, 모의·실전 매매 모드를 모두 제공합니다.

---

## 핵심 특징

| 항목 | 내용 |
|------|------|
| 브로커 | 한국투자증권 Open API (KIS) |
| 시장 | KRX 정규장 + 미국 정규장 |
| 주문 방식 | 지정가 전용 |
| AI 파이프라인 | Fundamental → News → Strategy → Critic (4-step LLM) |
| 신뢰도 게이팅 | Critic 검토 후 3단계 자동 하향 규칙 적용 |
| 수급 데이터 | KIS API (실시간 공식) + 네이버 금융 (컨센서스 best-effort) |
| 프론트엔드 | Leptos PWA (CSR) |
| 백엔드 | Axum REST API |
| 데이터베이스 | PostgreSQL (SQLx) |

---

## 아키텍처

```
[Leptos PWA]
     │
     └──▶ [Axum API Server] ──▶ [PostgreSQL]
                │
                ├──▶ KIS Open API
                ├──▶ LLM (OpenAI-compatible)
                ├──▶ DART / SEC Edgar
                ├──▶ Naver News / Finance
                └──▶ Telegram

[Worker Process]
 ├─ Scheduler        (5분 슬롯)
 ├─ Evidence Collector (뉴스/공시/수급)
 ├─ Scenario Engine   (LLM 4-step)
 ├─ Risk Gate
 ├─ Order Executor
 └─ Telegram Notifier
```

### Crate 의존 규칙

```
domain ◀── app ◀── infra ◀── { api, worker }
                               web (API DTO만 사용)
```

| Crate | 역할 |
|-------|------|
| `domain` | 순수 도메인 모델 및 Port 트레이트 |
| `app` | 애플리케이션 서비스, Repository 트레이트 |
| `infra` | KIS / LLM / DART / Naver 구현체, DB 레포지토리 |
| `api` | Axum REST 서버, JWT 인증 |
| `worker` | 스케줄러, 데이터 수집, 주문 실행 |
| `web` | Leptos PWA 프론트엔드 |

---

## 매니저-계좌 구조

매니저(Manager)는 계좌(BrokerConnection)와 1:1로 연결됩니다.

```
broker_connections (계좌 정보 + KIS 자격증명)
        │  1
        │
        │  N
    managers (매니저 = AI 자동매매 전략 단위)
        │
        └──▶ order_plans → broker_orders
```

- **매니저 생성 시** `broker_connection_id`의 존재 여부와 소유자(user_id) 일치를 검증합니다.
- **주문 실행 시** 매니저 → broker_connection 조회 → 계좌 환경(Real/Paper)에 맞는 KisClient를 생성하여 실행합니다.
- `mode=paper`인 매니저는 PaperBroker, `mode=live`인 매니저는 KisClient(Real)를 사용합니다.

---

## 시작하기

### 사전 요구 사항

- Rust 1.78+ (stable)
- PostgreSQL 15+
- [trunk](https://trunkrs.dev/) (Leptos 빌드)
- 한국투자증권 Open API 키 (app_key, app_secret, 계좌번호)

### 환경 변수 설정

```bash
cp .env.example .env
```

```env
DATABASE_URL=postgres://user:password@localhost/lumos
ENCRYPTION_KEY=<openssl rand -base64 32 으로 생성>
LISTEN_ADDR=0.0.0.0:3000

# 한국투자증권 (로컬 개발용 — 실제 값은 DB 암호화 저장 예정)
KIS_ENV=paper            # paper | real
KIS_APP_KEY=your_app_key
KIS_APP_SECRET=your_app_secret
KIS_ACCOUNT_NO=12345678  # 계좌번호 앞 8자리
KIS_ACCOUNT_PRODUCT=01   # 계좌상품코드 뒤 2자리

# JWT (개발용 기본값: "dev-secret-change-in-prod")
JWT_SECRET=change_me_in_production

# 프로덕션 환경 표시 (설정 시 dev token 엔드포인트 비활성화)
# APP_ENV=production

# LLM (선택)
# OPENAI_API_KEY=sk-...

# Telegram (선택)
# TELEGRAM_BOT_TOKEN=
```

### DB 마이그레이션

```bash
sqlx migrate run --database-url "$DATABASE_URL"
```

### 실행

**한 번에 모두 실행 (권장)**

```bash
./scripts/dev.sh
```

tmux 세션 `lumos-dev`를 생성합니다.

| tmux 창 | 프로세스 |
|---------|---------|
| `docker` | `docker compose up` — DB + API 서버 + Worker |
| `web` | `trunk serve` — Leptos 개발 서버 (`:8080`) |

창 전환: `Ctrl+B` → `0`/`1` 또는 창 이름  
세션 재접속: `tmux attach -t lumos-dev`  
전체 종료: `tmux kill-session -t lumos-dev`

**Docker만 실행**

```bash
docker compose up --build          # 포그라운드
docker compose up --build -d       # 백그라운드
docker compose down                # 종료 (데이터 보존)
docker compose down -v             # 종료 + DB 볼륨 삭제
```

회사 네트워크 / VPN 환경 (`Dockerfile.local` 사용):

```bash
cargo vendor --versioned-dirs
docker compose -f docker-compose.yml -f docker-compose.local.yml up --build
```

**프론트엔드만 별도 실행**

```bash
cd crates/web && trunk serve
```

---

## 인증 (JWT)

모든 `/api/*` 경로는 JWT Bearer 토큰이 필요합니다.

### 개발용 토큰 발급

```bash
# user_id 자동 생성
curl -X POST http://localhost:3000/api/auth/token \
  -H "Content-Type: application/json" \
  -d '{}'

# 특정 user_id 고정
curl -X POST http://localhost:3000/api/auth/token \
  -H "Content-Type: application/json" \
  -d '{"user_id": "00000000-0000-0000-0000-000000000001"}'
```

응답:
```json
{
  "token": "eyJ...",
  "user_id": "00000000-0000-0000-0000-000000000001",
  "expires_in_hours": 24
}
```

이후 모든 요청에 헤더 추가:
```
Authorization: Bearer eyJ...
```

> **주의:** `APP_ENV=production` 설정 시 이 엔드포인트는 403을 반환합니다.  
> 프로덕션에서는 별도 OAuth2/OIDC 인증으로 교체해야 합니다.

---

## Feature Flags

| Flag | 설명 |
|------|------|
| `offline-fixtures` (기본) | fixture JSON으로 외부 API 대체 |
| `online-kis` | KIS 실시간 API 활성화 |
| `online-naver` | 네이버 뉴스 API 활성화 |
| `online-opendart` | DART 공시 API 활성화 |
| `online-sec` | SEC Edgar API 활성화 |
| `online-telegram` | Telegram 알림 활성화 |
| `live-trading` | 실전 브로커 주문 실행 활성화 |

```bash
# 모든 외부 API + 실전 매매 활성화
cargo build -p lumos-infra --features "online-kis,online-naver,live-trading"
```

---

## AI 시나리오 파이프라인

### 4-Step LLM 파이프라인

```
Step 1: Fundamental 분석
  재무/공시 Evidence → 기업 건강도 요약, 리스크 요인

Step 2: News/Event 분석
  뉴스/커뮤니티 Evidence → 이벤트 촉매, 감성, 고위험 키워드 감지

Step 3: Strategy 초안
  (Step 1 + Step 2 컨텍스트) → bullish/sideways/bearish 시나리오 + 추천 액션

Step 4: Critic 검토
  자기비판 → 편향/논리 비약 지적, 고위험 신호 시 전략 하향
```

### 신뢰도 게이팅 (Critic 이후 자동 적용)

| 규칙 | 조건 | 결과 |
|------|------|------|
| Rule 1 | Critic 이슈 있음 + 수정 시나리오 없음 | Watch로 하향 |
| Rule 2 | 시나리오 probability 합계 < 70% | Watch로 하향 |
| Rule 3 | 고위험 키워드 감지 + Buy 액션 | Hold로 하향 |

### Evidence Card 소스

| 소스 | 신뢰도 | 용도 |
|------|--------|------|
| KIS 시세 | 0.95 | 현재가/거래량 |
| KIS 투자자 수급 | 0.95 | 외국인/기관/개인 순매수 |
| DART/SEC 공시 | 0.90 | 기업 이벤트 근거 |
| 재무제표 | 0.90 | 중장기 밸류에이션 |
| 뉴스 | 0.65 | 이벤트 촉매 |
| Naver 컨센서스 | 0.60 | 목표주가/PER/PBR (best-effort) |
| 커뮤니티 | 0.35 | 보조 심리 신호 |

---

## Risk Gate

주문 실행 전 20단계 검증을 순차 적용합니다.

| 구분 | 주요 규칙 |
|------|-----------|
| 매니저 상태 | `status == active`, `auto_trade_enabled == true` |
| 글로벌 | `global_kill_switch == false` |
| 시장 | KRX/미국 정규장 시간 내, 지정가만 허용 |
| 데이터 신선도 | 시세 60초 이내, 잔고 60초 이내 |
| 리스크 한도 | 단일 주문 ≤ 100만원, 종목 비중 ≤ 자산의 5%, 일일 손실 ≤ -2% |
| 중복 방지 | 멱등키 검증 |
| AI 품질 | Evidence card ≥ 2개, AI 스키마 검증 통과 |

---

## API 엔드포인트

### 인증 (인증 불필요)

| Method | Path | 설명 |
|--------|------|------|
| `POST` | `/api/auth/token` | 개발용 JWT 토큰 발급 |
| `GET` | `/health` | 헬스체크 |

### 매니저 (JWT 필요)

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/managers` | 매니저 목록 |
| `POST` | `/api/managers` | 매니저 생성 (broker_connection_id 필수) |
| `GET` | `/api/managers/:id` | 매니저 상세 |
| `GET` | `/api/managers/:id/risk-policy` | 리스크 정책 조회 |
| `POST` | `/api/managers/:id/auto-trade` | 자동매매 ON/OFF |

### 시나리오 / 분석

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/managers/:id/scenarios` | 시나리오 실행 목록 |
| `POST` | `/api/managers/:id/scenarios` | 시나리오 수동 실행 |
| `GET` | `/api/managers/:id/analysis-reports` | 분석 리포트 목록 |

### 주문

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/managers/:id/order-plans` | 주문 계획 목록 |
| `POST` | `/api/managers/:id/order-plans` | 시나리오 아이템 → 주문 계획 생성 |
| `POST` | `/api/managers/:id/order-plans/:plan_id/execute` | 주문 실행 (`live-trading` feature 필요) |
| `POST` | `/api/paper/orders` | 모의 주문 직접 생성 |

### 포트폴리오

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/managers/:id/holdings` | 보유 종목 |
| `GET` | `/api/managers/:id/trades` | 체결 내역 |
| `GET` | `/api/managers/:id/schedule` | 스케줄 조회 |

---

## 스케줄러

- 30초마다 tick, 5분 단위 슬롯 기준으로 실행
- 슬롯별 `run_scenario` / `run_trade` 플래그로 독립 제어
- KRX / 미국 거래 시간 자동 감지
- 멱등키로 동일 슬롯 중복 실행 방지

---

## 오프라인 개발

기본 feature는 `offline-fixtures`가 활성화되어 있어 KIS/Naver/LLM 호출 없이 fixture 데이터로 동작합니다.

```bash
cargo test --workspace
```

---

## 디렉토리 구조

```
.
├── crates/
│   ├── domain/         # 도메인 모델, Port 트레이트
│   ├── app/            # 서비스 레이어, Repository 트레이트
│   ├── infra/          # 외부 API 구현체, DB 레포지토리
│   │   ├── kis/        # KIS Open API 클라이언트 + PaperBroker
│   │   ├── providers/  # Naver, DART, SEC, Telegram
│   │   ├── scenario/   # LLM 구현체, Evidence Builder
│   │   ├── db/         # SQLx 레포지토리
│   │   └── crypto/     # AES-GCM 키 암호화
│   ├── api/            # Axum REST 서버
│   ├── worker/         # 스케줄러, 수집기
│   └── web/            # Leptos PWA
├── migrations/         # SQLx 마이그레이션
├── prompts/            # LLM 시스템 프롬프트
├── contracts/          # JSON Schema 계약
├── fixtures/           # 오프라인 테스트 데이터
├── db/                 # DB 스키마 SQL
└── docs/               # 설계 문서
    ├── architecture.md
    ├── risk_policy.md
    ├── schedule_design.md
    ├── data_ingestion_design.md
    ├── api_specs/      # 외부 API 명세 (KIS, DART, SEC, Naver, Telegram, LLM)
    ├── db/             # DB 데이터 모델 노트
    └── tasks/          # 구현 태스크 및 개발 설정
```

---

## 개발 가이드

### 테스트

```bash
# 전체 테스트 (offline-fixtures 사용)
cargo test --workspace

# infra 단위 테스트 (fixture 포함)
cargo test -p lumos-infra --features offline-fixtures

# 특정 KIS 클라이언트 테스트
cargo test -p lumos-infra --features offline-fixtures kis
```

### 린트 / 포맷

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### 마이그레이션 추가

```bash
sqlx migrate add <migration_name>
# migrations/<timestamp>_<migration_name>.sql 편집 후
sqlx migrate run
```

---

## 보안 주의사항

- KIS API 키, DB URL, JWT 시크릿은 반드시 환경 변수로 관리하세요.
- `live-trading` feature는 실제 자산에 영향을 줍니다. 반드시 `paper` 환경에서 충분히 검증 후 활성화하세요.
- API 키는 DB에 AES-GCM으로 암호화 저장됩니다. `ENCRYPTION_KEY` 분실 시 복구 불가합니다.
- `APP_ENV=production` 설정 시 개발용 토큰 발급 엔드포인트(`/api/auth/token`)가 비활성화됩니다.
- `global_kill_switch`를 활용해 비상 시 전체 자동매매를 즉시 중단할 수 있습니다.

---

## 라이선스

MIT
