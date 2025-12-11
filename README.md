# VerWatch

<div align="center">

**🔭 Serverless GitHub Release Monitor**

*一个基于 Cloudflare Workers 和 Rust + Leptos 构建的轻量级上游版本监控系统*

[![Backend](https://img.shields.io/badge/Backend-Cloudflare_Workers-f38020?style=flat-square&logo=cloudflare)](./backend)
[![Frontend](https://img.shields.io/badge/Frontend-Leptos_WASM-orange?style=flat-square&logo=rust)](./frontend)
[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](./LICENSE)

</div>

---

## 📖 简介

**VerWatch** 是一个 "看门狗" 服务，能够定期监控上游 GitHub 仓库的最新 Release 版本。一旦发现更新，就会自动通过 `repository_dispatch` 事件触发您自己仓库的 GitHub Actions 工作流。

它是以下场景的理想工具：
- 维护 Fork 版本，自动同步上游更新
- Docker 镜像自动构建流水线
- 监控第三方依赖的版本更新

## ✨ 核心特性

| 特性 | 描述 |
|------|------|
| **☁️ 无服务器** | 基于 Cloudflare Workers 运行，零服务器维护成本 |
| **🔀 分布式架构** | 每个项目使用独立的 Durable Object (ProjectMonitor) 处理，天然水平扩展 |
| **⏰ 自主调度** | 每个 Monitor 通过 Alarm 机制独立调度检查任务，无需中心化 Cron |
| **🔐 安全可靠** | 支持 GitHub Token 和 Admin Secret 加密存储 |
| **🎛️ 配置灵活** | 支持自定义版本对比模式（发布时间 vs 更新时间） |
| **🦀 Rust 驱动** | 前后端均使用 Rust，利用强类型和高性能特性 |
| **🌐 跨域支持** | 内置 CORS 支持，允许前端应用直接调用 API |

## 🏗️ 系统架构

``` mermaid
graph TD
    %% 样式定义
    classDef frontend fill:#fce4ec,stroke:#c2185b,stroke-width:2px,color:#000;
    classDef api fill:#e1f5fe,stroke:#01579b,stroke-width:2px,color:#000;
    classDef registry fill:#fff9c4,stroke:#fbc02d,stroke-width:2px,color:#000;
    classDef monitor fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px,color:#000;

    %% 前端
    Frontend["<b>Frontend (Leptos SPA)</b><br/>Rust + WASM + DaisyUI"]:::frontend

    %% Admin API 层
    Admin["<b>Admin API (lib.rs)</b><br/>/api/projects (CRUD 操作)"]:::api

    %% 注册表层
    Registry["<b>ProjectRegistry DO</b><br/>(单例，注册表)<br/>──────────────<br/>register(config) → 调用 Monitor.setup()<br/>unregister(key) → 调用 Monitor.stop()<br/>list() → 遍历查询所有 Monitor.config"]:::registry

    %% 监控实例层
    subgraph Monitors [Durable Objects 实例群]
        direction LR
        MonA["<b>ProjectMonitor</b><br/>(项目 A)<br/>───<br/>config<br/>version<br/>alarm ⏰"]:::monitor
        MonB["<b>ProjectMonitor</b><br/>(项目 B)<br/>───<br/>config<br/>version<br/>alarm ⏰"]:::monitor
        MonC["<b>ProjectMonitor</b><br/>(项目 C)<br/>───<br/>config<br/>version<br/>alarm ⏰"]:::monitor
    end

    %% 连接关系
    Frontend --> Admin
    Admin --> Registry
    Registry --> MonA
    Registry --> MonB
    Registry --> MonC

    %% 调整连接线样式
    linkStyle 0 stroke:#c2185b,stroke-width:2px;
    linkStyle 1 stroke:#01579b,stroke-width:2px;
    linkStyle 2,3,4 stroke:#fbc02d,stroke-width:2px;
```

## 📁 项目结构

```
verwatch/
├── backend/          # 后端 Cloudflare Worker (Rust)
│   ├── src/          # 源代码
│   │   ├── lib.rs              # 入口，Admin API 路由
│   │   ├── project/            # 项目相关模块
│   │   │   ├── registry.rs     # ProjectRegistry Durable Object
│   │   │   └── monitor.rs      # ProjectMonitor Durable Object
│   │   └── ...
│   ├── wrangler.toml # Cloudflare 配置
│   └── README.md
│
├── frontend/         # 前端 SPA (Rust + Leptos + WASM)
│   ├── src/          # 源代码
│   │   ├── components/         # UI 组件
│   │   │   ├── dashboard.rs    # 控制面板
│   │   │   └── login.rs        # 登录页面
│   │   ├── api.rs              # API 客户端
│   │   └── auth.rs             # 认证状态管理
│   ├── index.html    # 应用入口
│   └── README.md
│
├── shared/           # 共享库 (前后端公用)
│   └── src/
│       ├── lib.rs              # 数据结构定义
│       └── protocol.rs         # RPC 协议定义
│
└── .github/
    └── workflows/
        ├── deploy_backend.yml   # 后端自动部署
        └── deploy_frontend.yml  # 前端自动部署
```

## 🚀 快速开始

### 环境准备

确保已安装以下工具：

1. **Rust & Cargo**: [安装指南](https://www.rust-lang.org/tools/install)
2. **Node.js & npm**: 用于安装 Wrangler
3. **Wrangler CLI**: Cloudflare Workers 命令行工具
   ```bash
   npm install -g wrangler
   ```
4. **Trunk** (仅前端开发需要): 
   ```bash
   cargo install trunk
   rustup target add wasm32-unknown-unknown
   ```

### 部署后端

```bash
# 1. 克隆项目
git clone https://github.com/ShaoG-R/verwatch.git
cd verwatch/backend

# 2. 配置密钥
wrangler secret put ADMIN_SECRET      # 管理 API 认证密钥
wrangler secret put GITHUB_TOKEN      # GitHub API Token (可选，用于读取上游 Release)
wrangler secret put MY_GITHUB_PAT     # GitHub PAT (必需，用于触发 Dispatch 事件)

# 3. 部署
wrangler deploy
```

详细部署说明请参考 [后端文档](./backend/README.md)。

### 部署前端

**方式 A: 本地开发**
```bash
cd frontend
trunk serve --open
```

**方式 B: 部署到 Cloudflare Pages**

推荐使用 GitHub Actions 自动部署，详细配置请参考 [前端文档](./frontend/README.md)。

## 🎮 使用方法

### 通过前端控制面板

1. 访问您部署的前端 URL
2. 输入后端 Worker URL 和 Admin Secret 登录
3. 在控制面板中添加、管理和监控您的项目

### 通过 API

**添加监控项目:**
```bash
curl -X POST https://your-worker.workers.dev/api/projects \
  -H "X-Auth-Key: your_admin_secret" \
  -H "Content-Type: application/json" \
  -d '{
    "base_config": {
      "upstream_owner": "tokio-rs",
      "upstream_repo": "tokio",
      "my_owner": "your-username",
      "my_repo": "your-fork"
    },
    "time_config": {
      "check_interval": { "secs": 3600, "nanos": 0 },
      "retry_interval": { "secs": 60, "nanos": 0 }
    },
    "comparison_mode": "published_at",
    "initial_delay": { "secs": 60, "nanos": 0 }
  }'
```

**查看监控列表:**
```bash
curl https://your-worker.workers.dev/api/projects \
  -H "X-Auth-Key: your_admin_secret"
```

更多 API 详情请参考 [后端文档](./backend/README.md#-使用指南)。

## 🤖 下游仓库配置

在您的目标仓库中创建以下 Workflow 文件以接收更新通知：

**`.github/workflows/sync-upstream.yml`**

```yaml
name: Sync Upstream Update

on:
  repository_dispatch:
    types: [upstream_update]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Receive Version Info
        run: |
          echo "Upstream released new version: ${{ github.event.client_payload.version }}"
          
      # 在这里添加您的构建、合并或发布逻辑
```

## 📚 详细文档

| 文档 | 描述 |
|------|------|
| [后端文档](./backend/README.md) | 后端架构、部署配置、API 详解 |
| [前端文档](./frontend/README.md) | 前端开发、构建、部署说明 |

## 🔄 版本历史

### v2 架构重构

| 变更项 | v1 (旧) | v2 (新) |
|--------|---------|---------|
| **核心设计** | 单一 ProjectStore DO 存储所有配置 | 分布式 ProjectMonitor DO，每个项目独立 |
| **调度方式** | 中心化 Cron Job | 每个 Monitor 独立 Alarm 调度 |
| **扩展性** | 受单 DO 性能限制 | 天然水平扩展 |
| **配置存储** | ProjectStore 存储 Config | ProjectMonitor 自己存储 Config |
| **注册表** | N/A | ProjectRegistry 管理注册关系 |

## 🛠️ 技术栈

### 后端
- **Runtime**: Cloudflare Workers
- **Language**: Rust
- **State Management**: Durable Objects
- **Framework**: worker-rs

### 前端
- **Language**: Rust (WebAssembly)
- **Framework**: Leptos
- **Styling**: TailwindCSS + DaisyUI
- **Build Tool**: Trunk
- **Hosting**: Cloudflare Pages

### 共享
- **Crate**: `shared` - 前后端共用的数据结构和协议定义

## 📄 License

[MIT License](./LICENSE)
