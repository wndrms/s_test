# ── Stage 1: builder ──────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 의존성 레이어 캐시
COPY Cargo.toml Cargo.lock ./
COPY crates/domain/Cargo.toml  crates/domain/Cargo.toml
COPY crates/app/Cargo.toml     crates/app/Cargo.toml
COPY crates/infra/Cargo.toml   crates/infra/Cargo.toml
COPY crates/api/Cargo.toml     crates/api/Cargo.toml
COPY crates/worker/Cargo.toml  crates/worker/Cargo.toml
COPY crates/web/Cargo.toml     crates/web/Cargo.toml

RUN mkdir -p \
    crates/domain/src crates/app/src crates/infra/src \
    crates/api/src crates/worker/src crates/web/src && \
    echo "pub fn main() {}" > crates/domain/src/lib.rs && \
    echo "pub fn main() {}" > crates/app/src/lib.rs && \
    echo "pub fn main() {}" > crates/infra/src/lib.rs && \
    echo "fn main() {}" > crates/api/src/main.rs && \
    echo "fn main() {}" > crates/worker/src/main.rs && \
    echo "pub fn main() {}" > crates/web/src/lib.rs && \
    echo "fn main() {}" > crates/web/src/main.rs

RUN cargo build --release -p lumos-api -p lumos-worker 2>/dev/null || true

# 실제 소스 복사 후 재빌드
COPY crates/ crates/
COPY migrations/ migrations/
COPY fixtures/ fixtures/
COPY prompts/ prompts/

RUN touch crates/domain/src/lib.rs crates/app/src/lib.rs \
         crates/infra/src/lib.rs crates/api/src/main.rs \
         crates/worker/src/main.rs && \
    cargo build --release -p lumos-api -p lumos-worker

# ── Stage 2: api runtime ──────────────────────────────────────────────────────
FROM debian:bookworm-slim AS api

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/lumos-server /app/lumos-server
COPY --from=builder /app/migrations /app/migrations/

EXPOSE 5000
CMD ["/app/lumos-server"]

# ── Stage 3: worker runtime ───────────────────────────────────────────────────
FROM debian:bookworm-slim AS worker

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/lumos-worker /app/lumos-worker
COPY --from=builder /app/migrations /app/migrations/
COPY --from=builder /app/fixtures /app/fixtures/

CMD ["/app/lumos-worker"]
