# Stage 1: Builder
FROM rust:1.96-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
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
