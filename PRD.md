# AstrBot EF — Rust 重构 PRD

## 一、概述

- **项目名称**：astrbot_ef（Efficient）
- **定位**：用 Rust 完整移植 AstrBot v4.25.5，功能不变，性能更优
- **原项目**：Python 15.6 万行 → 目标 Rust 约 3-5 万行
- **Why Rust**：零成本抽象、无 GIL 并发、低内存占用、单二进制部署

## 二、痛点

| # | 痛点 | 解决 |
|---|---|---|
| 1 | Python GIL 限制并发消息处理 | tokio 异步原生并发 |
| 2 | 依赖管理复杂（pip 数十个包）| 单二进制，cargo 管理 |
| 3 | 内存占用高（Python 运行时 ~200MB+）| Rust 原生 ~10-30MB |
| 4 | Docker 部署依赖 Python 镜像（~1GB）| 最小镜像 ~10MB |
| 5 | 跨平台分发需打包麻烦 | 单文件编译，开箱即用 |

## 三、技术栈

| 层 | 选型 | 说明 |
|---|---|---|
| 语言 | Rust (edition 2024 / 1.96.1) | |
| Web 框架 | Axum | 生态最活跃，tokio 原生 |
| 异步运行时 | tokio | |
| 数据库 | SQLite via sqlx | 编译期 SQL 校验 |
| 序列化 | serde + serde_json | |
| HTTP 客户端 | reqwest | LLM API 调用 |
| 日志 | tracing | 结构化日志 |
| 配置 | serde + toml | |
| 插件系统 | dlopen / WASM（P2）| |

## 四、架构映射

```
Python AstrBot               Rust astrbot_ef
─────────────────────         ─────────────────────
core/event_bus.py      →      core::event_bus (tokio broadcast)
core/astr_main_agent   →      core::agent
core/pipeline/*        →      core::pipeline
platform/* (24K行)     →      platform::adapters (trait + impl)
provider/* (9K行)      →      provider::llm (trait + impl)
db/* (5K行)            →      db:: (sqlx models)
config/* (4K行)        →      config:: (serde deser)
star/* (5K行)          →      plugin:: (trait + registry)
api/ (FastAPI)         →      api:: (Axum routes)
dashboard/ (Vue前端)    →      [复用现有前端，对接新 API]
utils/* (8K行)         →      utils::
```

## 五、MVP 功能（P0）

| 优先级 | 功能 | 验收标准 |
|---|---|---|
| P0 | 配置管理 | 读取 TOML 配置文件，支持热重载 |
| P0 | LLM 对话 | 接入 DeepSeek V4 Flash/Pro，支持流式输出 |
| P0 | 微信消息收发 | 通过 wechatcom_api 协议收发文本消息 |
| P0 | 消息路由闭环 | 微信消息 → LLM 处理 → 回复微信 |
| P1 | 数据库持久化 | 会话历史、用户数据存入 SQLite |
| P1 | 管理 API | 对接现有 Vue 前端的状态查询/控制 |
| P1 | 多消息源 | QQ / Telegram / Discord 适配器 |
| P1 | 多 LLM Provider | OpenAI / Claude / 自定义 endpoint |
| P2 | 插件系统 | WASM / dlopen 动态加载插件 |
| P2 | 知识库 | 向量搜索集成（Qdrant / tantivy）|
| P2 | 完整 Dashboard | 复用或重写 Web 管理界面 |

## 六、里程碑

| 阶段 | 内容 | 预估 |
|---|---|---|
| Week 1-2 | 项目骨架 + config + core::event_bus + LLM provider(DeepSeek) | P0 闭环 |
| Week 3-4 | platform::wechat + 消息路由闭环（微信→LLM→回复） | P0 完成 |
| Week 5-8 | db + api + 其他 platform/provider + dashboard 对接 | P1 完成 |
| Week 9-12 | 插件系统 + 知识库 + 性能优化 + 完整测试 | P2 完成 |

## 七、目录结构

```
astrbot_ef/
├── Cargo.toml              # workspace 根
├── PRD.md
├── README.md
├── config/
│   └── default.toml
├── core/                   # 核心逻辑
│   ├── Cargo.toml
│   └── src/
│       ├── event_bus.rs
│       ├── agent.rs
│       ├── pipeline.rs
│       └── lib.rs
├── platform/               # 消息源适配器
│   ├── Cargo.toml
│   └── src/
│       ├── adapters/
│       │   ├── wechat.rs
│       │   ├── qq.rs
│       │   ├── telegram.rs
│       │   └── discord.rs
│       ├── traits.rs
│       └── lib.rs
├── provider/               # LLM 提供商
│   ├── Cargo.toml
│   └── src/
│       ├── llm/
│       │   ├── deepseek.rs
│       │   ├── openai.rs
│       │   └── claude.rs
│       ├── traits.rs
│       └── lib.rs
├── db/                     # 数据库
│   ├── Cargo.toml
│   └── src/
│       ├── models.rs
│       ├── migrations/
│       └── lib.rs
├── config_mgr/             # 配置管理
│   ├── Cargo.toml
│   └── src/lib.rs
├── plugin/                 # 插件系统
│   ├── Cargo.toml
│   └── src/lib.rs
├── api/                    # HTTP API（Dashboard 后端）
│   ├── Cargo.toml
│   └── src/
│       ├── routes.rs
│       └── lib.rs
├── utils/                  # 工具函数
│   ├── Cargo.toml
│   └── src/lib.rs
├── cli/                    # 入口二进制
│   ├── Cargo.toml
│   └── src/main.rs
└── tests/                  # 集成测试
    ├── Cargo.toml
    └── src/lib.rs
```
