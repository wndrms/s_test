# Offline Development Setup

인터넷이 없는 환경에서 개발하려면 온라인 환경에서 의존성과 문서, 샘플 응답을 미리 고정해 가져가야 합니다.

## 1. Rust Dependencies Vendor

온라인 환경에서:

```bash
cargo new stockworldcup --bin
cd stockworldcup
# Cargo.toml 작성 후
cargo vendor vendor > .cargo/config.toml
cargo fetch --locked
```

오프라인 환경에서:

```bash
cargo check --locked
```

권장 `.cargo/config.toml` 예시:

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

## 2. SQLx Offline

SQLx compile-time query checking을 쓸 경우 온라인/DB 접속 가능 환경에서 준비합니다.

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/stockworldcup cargo sqlx prepare --workspace
```

생성된 `.sqlx/` 디렉터리를 오프라인 환경으로 복사합니다.

## 3. Docker Images

온라인 환경에서 필요한 이미지를 저장합니다.

```bash
docker pull postgres:15
docker save postgres:15 -o postgres_15.tar
```

오프라인 환경에서:

```bash
docker load -i postgres_15.tar
```

## 4. API Fixture Strategy

외부 API 호출 코드는 다음 구조를 권장합니다.

```text
infra/provider_name/client.rs
infra/provider_name/dto.rs
infra/provider_name/fixtures/*.json
```

테스트는 기본적으로 fixture를 사용합니다.
온라인 호출 테스트는 별도 feature flag 뒤에 숨깁니다.

예시:

```bash
cargo test
cargo test --features online-kis -- --ignored
```

## 5. Feature Flags

권장 feature:

```toml
[features]
default = ["offline-fixtures"]
offline-fixtures = []
online-kis = []
online-opendart = []
online-sec = []
online-naver = []
online-telegram = []
live-trading = []
```

## 6. 개발 순서

1. 외부 API 없이 도메인/DB/화면 구현
2. fixture 기반 provider 파서 구현
3. PaperBroker 자동매매 구현
4. Risk Engine 구현
5. 온라인 환경에서 KIS 조회 API 검증
6. 모의 주문 검증
7. 소액 실전 주문 검증

## 7. 절대 원칙

- 오프라인 개발 중에는 실전 주문 코드를 기본 활성화하지 않습니다.
- `live-trading` feature가 없으면 KIS live order 함수를 컴파일 또는 실행하지 못하게 막습니다.
- API 키는 테스트 fixture나 로그에 넣지 않습니다.
- 외부 응답 fixture에는 개인정보/계좌번호를 마스킹합니다.
