# AstrBotRS

Rust rewrite of AstrBot -- LLM message relay + WeChat personal account adapter + Web dashboard + Plugin system.

## Architecture

The project is a Cargo workspace with **10 crates**:

```
┌──────────────────────────────────────────────────────────┐
│                       cli (entry point)                   │
├──────────────────────────────────────────────────────────┤
│  core (lifecycle, event_bus, pipeline, agent)             │
├─────────┬─────────┬──────────┬──────────┬───────────────┤
│ platform│ provider│   db     │config_mgr│   plugin       │
│ (weixin │ (OpenAI │ (sqlite  │ (TOML    │ (WASM/dlopen  │
│  oc,    │  compat)│  via     │  serde)  │  - v2)         │
│  ...)   │         │  sqlx)   │          │               │
├─────────┴─────────┴──────────┴──────────┴───────────────┤
│              api (Axum REST + static frontend)            │
├──────────────────────────────────────────────────────────┤
│                      utils                                │
└──────────────────────────────────────────────────────────┘
```

**Data flow**: WeChat/other platform -> `Platform` trait -> `EventBus` -> `Pipeline` -> `Provider` (LLM) -> response -> `Platform::send_message`. The API crate serves both the REST endpoints and the Vue 3 dashboard frontend.

## Quick Start

**Prerequisites**: Rust 1.85+, build-essential, pkg-config, libsqlite3-dev

```bash
git clone <repo-url> astrbot_rs
cd astrbot_rs
cargo build --release -p astrbot-cli

# Run (config auto-generated on first launch)
ASTRBOT_CONFIG=data/config.toml cargo run --release -p astrbot-cli
```

## Configuration

TOML config file at the path specified by `ASTRBOT_CONFIG` (default `data/astrbot_config.toml`). Auto-generated with defaults on first run.

```toml
[dashboard]
host = "0.0.0.0"
port = 6185
username = "astrbot"
password = "astrbot"
# jwt_secret = "<random-32-char>"   # auto-generated if absent

[[provider]]
id = "deepseek"
type = "openai_chat_completion"
enable = true
api_key = "sk-..."                  # your API key
base_url = "https://api.deepseek.com"
model = "deepseek-chat"

# Multiple providers can be defined:
# [[provider]]
# id = "openai"
# type = "openai_chat_completion"
# enable = true
# api_key = "sk-..."
# base_url = "https://api.openai.com/v1"
# model = "gpt-4o"

[[platform]]
id = "my_wechat"
type = "weixin_oc"
enable = true
# weixin_oc_base_url = "https://ilinkai.weixin.qq.com"   # default
# weixin_oc_cdn_base_url = "https://novac2c.cdn.weixin.qq.com/c2c"
# weixin_oc_token = "..."                                 # optional, pre-auth token
```

### Sections

| Section | Fields |
|---|---|
| `dashboard` | `host` (default `0.0.0.0`), `port` (default `6185`), `username`, `password`, `jwt_secret` |
| `provider[]` | `id`, `type` (`openai_chat_completion`), `enable`, `api_key`, `base_url`, `model` |
| `platform[]` | `id`, `type` (`weixin_oc`), `enable`, plus extra fields per platform type |

## API Endpoints

All routes are prefixed under `/api/v1`. Authentication uses JWT Bearer tokens (24h expiry).

| Method | Path | Description |
|---|---|---|
| POST | `/api/v1/auth/login` | Login with username/password, returns JWT |
| GET | `/api/v1/auth/verify` | Verify JWT token validity |
| GET | `/api/v1/config` | Get full config (requires auth) |
| PUT | `/api/v1/config` | Update config sections and persist to disk |
| GET | `/api/v1/bots` | List configured platforms |
| GET | `/api/v1/providers` | List configured providers |
| GET | `/api/v1/providers/instances` | List running provider instances |
| GET | `/api/v1/logs/stream` | SSE stream of live logs |
| GET | `/api/v1/logs/recent` | Recent 200 log entries |
| GET | `/api/v1/stats` | Server status, uptime, provider count |

## WeChat Setup (Personal Account)

The WeChat adapter uses the **Weixin OC (WeChat Official Client)** protocol -- an HTTP bridge to the desktop WeChat process.

1. Install [WeChat](https://pc.weixin.qq.com/) on a Windows machine
2. Start the ilink bridge service on that machine (third-party tool)
3. Set `[[platform]]` in config as shown above with `type = "weixin_oc"`
4. On startup, the adapter polls a QR code URL and prints it to the log
5. Scan the QR code with your phone's WeChat app
6. Once confirmed, messages sync in real-time over HTTP long-poll

The adapter handles text, image, voice, file, and video messages both sending and receiving.

## Dashboard Frontend

The API crate serves the Vue 3 dashboard from `data/dist/`. Build and deploy:

```bash
# Build the original AstrBot Vue dashboard
cd astrbot_refactor/dashboard   # or your Vue dashboard source
npm install -g pnpm
pnpm install && pnpm build

# Copy to where the server expects it
cp -r dist /path/to/astrbot_rs/data/dist
```

Restart AstrBotRS -- the dashboard is served at `http://localhost:6185`.

## Docker

```dockerfile
# Build stage
FROM rust:1.85-bookworm AS builder
RUN apt-get update && apt-get install -y build-essential pkg-config libsqlite3-dev
WORKDIR /app
COPY . .
RUN cargo build --release -p astrbot-cli

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libsqlite3-dev ca-certificates
COPY --from=builder /app/target/release/astrbot-cli /usr/local/bin/astrbot
EXPOSE 6185
ENTRYPOINT ["astrbot"]
```

```bash
docker build -t astrbot-rs .
docker run -d -p 6185:6185 -v /path/to/config:/app/data astrbot-rs
```

## Development

This project uses a custom GCC toolchain for the build environment. Source `setup_env.sh` before building:

```bash
source setup_env.sh
cargo build
```

This sets `CC`, `CFLAGS` (with `--sysroot=/tmp/toolchain`), and `LD_LIBRARY_PATH` for the `x86_64-linux-gnu-gcc-14` cross-compiler. The `.cargo/config.toml` documents these requirements.

### Project structure

```
cli/            -- binary entry point
core/           -- lifecycle, event bus, pipeline, agent
platform/       -- platform adapters (weixin_oc, ...)
provider/       -- LLM providers (OpenAI-compatible)
db/             -- SQLite database layer (sqlx)
config_mgr/     -- TOML config loading/saving
api/            -- Axum REST API + static frontend serving
plugin/         -- plugin system (v2)
utils/          -- shared utilities (logging, errors)
tests/          -- integration tests
```

## Project Status

**v0.1.0** -- MVP complete.

- [x] TOML configuration with hot-reload
- [x] OpenAI-compatible LLM provider (DeepSeek, OpenAI, etc.)
- [x] WeChat personal account adapter (Weixin OC protocol)
- [x] End-to-end message relay (WeChat -> LLM -> reply)
- [x] SQLite database (sqlx)
- [x] JWT-authenticated REST API
- [x] SSE log streaming
- [x] Vue 3 dashboard static serving
- [x] Config management via API
