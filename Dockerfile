# Stage 1: Builder
FROM rust:1.96-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Layer 1: Copy manifests only + create dummy sources → cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml core/Cargo.toml
COPY api/Cargo.toml api/Cargo.toml
COPY platform/Cargo.toml platform/Cargo.toml
COPY provider/Cargo.toml provider/Cargo.toml
COPY db/Cargo.toml db/Cargo.toml
COPY config_mgr/Cargo.toml config_mgr/Cargo.toml
COPY plugin/Cargo.toml plugin/Cargo.toml
COPY utils/Cargo.toml utils/Cargo.toml
COPY cli/Cargo.toml cli/Cargo.toml
COPY tests/Cargo.toml tests/Cargo.toml

RUN mkdir -p core/src api/src platform/src provider/src \
    db/src config_mgr/src plugin/src utils/src cli/src tests/src && \
    echo > core/src/lib.rs && \
    echo > api/src/lib.rs && \
    echo > platform/src/lib.rs && \
    echo > provider/src/lib.rs && \
    echo > db/src/lib.rs && \
    echo > config_mgr/src/lib.rs && \
    echo > plugin/src/lib.rs && \
    echo > utils/src/lib.rs && \
    echo "fn main() {}" > cli/src/main.rs && \
    echo > tests/src/lib.rs && \
    cargo check --release -p astrbot-cli 2>&1 || true

# Layer 2: Copy real source → only rebuild our crates (deps from layer 1 reused)
COPY . .
RUN cargo build --release -p astrbot-cli

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/astrbot-cli /usr/local/bin/astrbot-cli

VOLUME ["/data"]

EXPOSE 6185

CMD ["sh", "-c", "ASTRBOT_CONFIG=/data/config.toml ASTRBOT_DB=/data/astrbot.db /usr/local/bin/astrbot-cli"]
