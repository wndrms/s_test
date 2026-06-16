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
| `api` | Axum REST 서버 (자동 인증 미들웨어) |
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

- **매니저 생성 시** `broker_connection_id`(미지정 시 KIS 자격증명 또는 모의 connection 자동 생성)와 소유자(user_id) 일치를 검증합니다. 모의(paper) 매니저는 실제 KIS 인증 없이 생성됩니다.
- **주문 실행 시** `BrokerFactory`가 매니저 → broker_connection 조회 → 계좌 환경(Real/Paper)에 맞는 broker를 동적 생성합니다.
  - `environment=real` → `KisClient` (시크릿 복호화 후 생성)
  - `environment=paper` → `PaperBroker` (connection별 상태 캐시)
- **매니저당 LLM 연결** — 매니저 생성 시 `model_provider`/`model_name`/`llm_key_id`를 지정하며, 해당 매니저의 모든 시나리오 생성이 이 설정을 따릅니다.

### 자동매매 안전장치

실제 주문은 **다음 조건을 모두** 만족할 때만 실행됩니다.

1. 빌드에 `live-trading` feature 포함 (로컬 `Dockerfile.local`은 기본 포함)
2. 환경변수 `ENABLE_LIVE_TRADING=true` (전역 스위치, 기본 false)
3. 매니저의 `auto_trade_enabled=true`

위 조건이 하나라도 빠지면 order plan 생성·리스크 평가까지만 수행하고 실제 주문은 나가지 않습니다.

---

## 시작하기

### 사전 요구 사항

- Rust 1.78+ (stable)
- PostgreSQL 16+ (Docker 사용 시 자동 구성)
- [trunk](https://trunkrs.dev/) (Leptos 빌드)
- 한국투자증권 Open API 키 (app_key, app_secret, 계좌번호) — 실거래/실시간 시세 사용 시

### 환경 변수 설정

`.env` 파일을 프로젝트 루트에 생성합니다. (`.env.example` 참고)

#### 필수

```env
# DB. docker-compose 사용 시 아래 값 그대로 사용
DATABASE_URL=postgres://postgres:postgres@db:5432/lumos

# API 키 암호화용 (AES-256-GCM). 분실 시 저장된 KIS/LLM 시크릿 복호화 불가.
#   openssl rand -base64 32  로 생성
ENCRYPTION_KEY=<base64 32바이트>
```

> ⚠️ `ENCRYPTION_KEY`를 설정하지 않으면 기동마다 임의 키가 생성되어 **재시작 후 저장된 시크릿을 복호화할 수 없고, api와 worker가 서로 다른 키를 갖게 됩니다.** 반드시 고정 값을 지정하세요.

#### LLM (시나리오 생성)

```env
OPENAI_API_KEY=sk-...           # 미설정 시 MockLlmProvider(더미 응답)로 동작
OPENAI_MODEL=gpt-4o-mini        # 기본값: gpt-4o-mini
```

#### 한국투자증권 KIS (실거래/실시간 시세)

```env
KIS_ENV=paper                   # paper | real (worker 시세 조회 환경)
KIS_APP_KEY=your_app_key
KIS_APP_SECRET=your_app_secret
KIS_ACCOUNT_NO=12345678         # 계좌번호 앞 8자리
KIS_ACCOUNT_PRODUCT=01          # 계좌상품코드 뒤 2자리 (기본 01)
```

> KIS 키가 없으면 worker의 투자자 수급·실시간 시세·시나리오 결과 평가가 비활성화되고, 매니저별 자격증명은 DB에 AES-GCM으로 암호화 저장되어 사용됩니다.

#### 네이버 뉴스 (선택 — Evidence 수집)

```env
NAVER_CLIENT_ID=your_client_id
NAVER_CLIENT_SECRET=your_client_secret
```

`NAVER_CLIENT_ID`와 `NAVER_CLIENT_SECRET`은 **[네이버 개발자센터](https://developers.naver.com/apps)** 에서 발급합니다.

1. 네이버 개발자센터 로그인 → **Application → 애플리케이션 등록**
2. 사용 API에서 **검색**(뉴스 검색) 선택
3. 환경 추가에서 **WEB 설정** (서비스 URL은 임의값 가능, 예 `http://localhost`)
4. 등록 후 발급된 **Client ID / Client Secret** 을 위 환경변수에 입력

> 둘 다 설정되어야 뉴스 수집이 활성화됩니다. 하나라도 없으면 뉴스 Evidence 없이 동작합니다.

#### 자동매매 / 운영 (선택)

```env
# 실거래 주문 실행 안전 스위치. 기본 false.
# true/1/yes 일 때만, 그리고 매니저의 auto_trade_enabled=true 일 때만 실제 주문이 나갑니다.
ENABLE_LIVE_TRADING=false

# 모의(paper) broker 초기 자본 (KRW). 기본 100,000,000
PAPER_INITIAL_CASH=100000000

# 로그 레벨 (tracing EnvFilter). 기본 info
RUST_LOG=info

# 인증 미들웨어가 주입하는 기본 사용자 ID (개발용)
# 기본값: 00000000-0000-0000-0000-000000000001
DEFAULT_USER_ID=00000000-0000-0000-0000-000000000001
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

API 서버는 `http://localhost:5000`, DB는 `localhost:5432`로 노출됩니다.

회사 네트워크 / VPN 환경 (`Dockerfile.local` — vendor 오프라인 빌드):

```bash
# 최초 1회 또는 Cargo.lock 변경 시 의존성 벤더링
cargo vendor --versioned-dirs

# 반드시 local override를 함께 지정 (기본 Dockerfile은 crates.io 접근 실패로 깨짐)
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d --build
```

> 로컬 빌드는 `Dockerfile.local`이 `--features live-trading`으로 빌드합니다.
> 실제 주문 실행은 `ENABLE_LIVE_TRADING=true` 일 때만 활성화됩니다(아래 자동매매 참고).

**DB 스키마 변경 시 (로컬 개발)**

마이그레이션 파일을 수정했고 로컬 DB를 버려도 되면, 볼륨을 재생성합니다.

```bash
docker compose -f docker-compose.yml -f docker-compose.local.yml down -v
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d --build
```

> 이미 적용된 마이그레이션 파일을 수정하면 sqlx 체크섬 검증에 걸려 기동이 실패합니다.
> 운영/공유 DB에 올라간 마이그레이션은 절대 수정하지 말고 새 파일로 분리하세요.

**프론트엔드만 별도 실행**

```bash
cd crates/web && trunk serve
```

---

## 인증

현재 `/api/*` 경로는 **자동 인증 미들웨어**(`auto_auth_middleware`)를 사용합니다.
모든 요청에 고정 사용자 ID를 자동으로 주입하므로 별도 토큰이 필요 없습니다.

- 기본 사용자 ID: `00000000-0000-0000-0000-000000000001`
- `DEFAULT_USER_ID` 환경변수로 변경 가능

```bash
# 토큰 없이 바로 호출
curl http://localhost:5000/api/managers
```

> **주의:** 이는 개발/단일 사용자 환경 전용입니다. 프로덕션 배포 전 OAuth2/OIDC 등
> 실제 인증으로 교체해야 합니다.

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
| 시장 | KRX/미국 정규장 시간 내, 지정가만 허용 |
| 데이터 신선도 | 시세 60초 이내, 잔고 60초 이내 |
| 리스크 한도 | 단일 주문 ≤ 100만원, 종목 비중 ≤ 자산의 5%, 일일 손실 ≤ -2% |
| 중복 방지 | 멱등키 검증 |
| AI 품질 | Evidence card ≥ 2개, AI 스키마 검증 통과 |

---

## API 엔드포인트

### 시스템

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/health` | 헬스체크 |

### 매니저

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
| `GET` | `/api/managers/:id/scenarios/runs` | 시나리오 실행 목록 |
| `GET` | `/api/managers/:id/scenarios/runs/:run_id/items` | 시나리오 항목 조회 |
| `GET` | `/api/managers/:id/analysis-reports` | 분석 리포트 목록 |

> 시나리오는 **worker 스케줄러가 자동 생성**합니다. 수동 실행 엔드포인트는 없으며 API는 조회 전용입니다. 사용할 LLM 모델은 매니저 설정(`model_provider`/`model_name`)을 따릅니다.

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
- **활성화된 슬롯은 시나리오 생성 → 매매를 하나의 사이클로 순차 실행** (시나리오/매매 구분 없음)
- KRX / 미국 거래 시간 자동 감지
- 멱등키로 동일 슬롯 중복 실행 방지
- 하루 1회 시나리오 결과 평가 (자기진화, 아래 참고)

---

## 자기진화 (시나리오 결과 피드백)

생성된 시나리오의 정확도를 추적해 다음 시나리오 생성에 반영합니다.

1. **결과 추적** — 시나리오 생성 후 `EVAL_DELAY_DAYS`(기본 3일) 경과 시, target/stop이 설정된
   Buy/Sell 시나리오의 적중 여부를 평가합니다.
2. **평가** — worker가 하루 1회 만료 시나리오를 스캔하여 KIS 현재가와 비교,
   `target_hit` / `stop_hit` / `expired` 결과를 `scenario_outcomes`에 기록합니다.
3. **피드백** — 다음 시나리오 생성 시 해당 종목의 최근 적중률을 Evidence Card로 요약해
   LLM 프롬프트에 주입합니다. ("과거 적중률 N% — 신뢰도 보정 참고")

> 한계: KIS historical 시세가 없어 "평가 시점 현재가" 기준으로 보수적 판정합니다.
> 기간 중 일시 도달 후 되돌린 경우는 포착하지 못합니다.

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

- KIS API 키, DB URL 등 시크릿은 반드시 환경 변수로 관리하세요.
- `live-trading` feature + `ENABLE_LIVE_TRADING=true` 는 실제 자산에 영향을 줍니다. 반드시 `paper` 환경에서 충분히 검증 후 활성화하세요.
- API 키는 DB에 AES-GCM으로 암호화 저장됩니다. `ENCRYPTION_KEY` 분실 시 복구 불가하며, 미설정 시 기동마다 키가 바뀌어 복호화가 깨집니다.
- 현재 인증은 개발용 자동 인증(고정 user_id)입니다. 프로덕션 배포 전 실제 인증(OAuth2/OIDC)으로 교체하세요.

---

## 라이선스

MIT
