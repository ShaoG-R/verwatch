# VerWatch: Serverless GitHub Release Monitor

**VerWatch** 是一个基于 Rust 和 Cloudflare Workers (Durable Objects) 构建的轻量级"看门狗"服务。它能够定期监控上游 GitHub 仓库的最新 Release 版本，一旦发现更新，就会自动通过 `repository_dispatch` 事件触发您自己仓库的 GitHub Actions 工作流。

它是维护 Fork 版本、Docker 镜像自动构建或同步上游更新的理想工具。

## ✨ 特性

- **轻量高效**：基于 Cloudflare Workers 运行，无服务器维护成本。
- **分布式架构**：每个项目使用独立的 Durable Object (ProjectMonitor) 处理，天然水平扩展。
- **自主调度**：每个 Monitor 通过 Alarm 机制独立调度检查任务，无需中心化 Cron。
- **安全可靠**：支持 GitHub Token 和 Admin Secret 加密存储。
- **配置灵活**：支持自定义版本对比模式（发布时间 vs 更新时间）。
- **Rust 驱动**：利用 Rust 的强类型和高性能特性。
- **跨域支持**：内置 CORS 支持，允许前端应用直接调用 API。

## 🏗️ 架构

``` mermaid
graph TD
    %% 样式定义
    classDef api fill:#e1f5fe,stroke:#01579b,stroke-width:2px,color:#000;
    classDef registry fill:#fff9c4,stroke:#fbc02d,stroke-width:2px,color:#000;
    classDef monitor fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px,color:#000;

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
    Admin --> Registry
    Registry --> MonA
    Registry --> MonB
    Registry --> MonC

    %% 调整连接线样式
    linkStyle 0 stroke:#01579b,stroke-width:2px;
    linkStyle 1,2,3 stroke:#fbc02d,stroke-width:2px;
```

## 🛠️ 环境准备

在开始之前，请确保您已经安装了以下工具：

1. **Rust & Cargo**: [安装指南](https://www.rust-lang.org/tools/install)
2. **Node.js & npm**: 用于安装 Wrangler。
3. **Wrangler CLI**: Cloudflare Workers 的命令行工具。
   ```bash
   npm install -g wrangler
   ```

## 🚀 部署指南

### 1. 克隆项目

```bash
git clone https://github.com/ShaoG-R/verwatch.git
cd verwatch/backend
```

### 2. 配置 wrangler.toml

在项目 `backend` 目录的 `wrangler.toml` 文件已预配置好。关键配置说明：

```toml
[durable_objects]
bindings = [
    # ProjectRegistry: 管理所有 Monitor 的注册表 (单例)
    { name = "PROJECT_REGISTRY", class_name = "ProjectRegistry" },
    # ProjectMonitor: 每个项目的监控实例 (按 unique_key 分片)
    { name = "PROJECT_MONITOR", class_name = "ProjectMonitor" }
]

[vars]
REGISTRY_BINDING = "PROJECT_REGISTRY"
ADMIN_SECRET_NAME = "ADMIN_SECRET"
```

### 3. 设置敏感密钥 (Secrets)

为了安全起见，Token 不应明文写在配置文件中，请使用 `wrangler secret` 命令上传。

**ADMIN_SECRET**: 用于保护您的管理 API（添加/删除监控项目）。
```bash
wrangler secret put ADMIN_SECRET
# 输入一个复杂的密码，例如: my_super_secure_password
```

**GITHUB_TOKEN** (可选但推荐): 用于读取上游仓库 Release 信息（避免 API 速率限制）。
```bash
wrangler secret put GITHUB_TOKEN
# 输入您的 GitHub Personal Access Token (Fine-grained personal access tokens 下无需勾选)
```

**MY_GITHUB_PAT**: 用于触发下游仓库的 Dispatch 事件（必须有写权限）。
```bash
wrangler secret put MY_GITHUB_PAT
# 输入您的 GitHub PAT (Fine-grained personal access tokens 下勾选 Context，设置 Read and Write)
```

### 4. 部署到 Cloudflare

```bash
wrangler deploy
```

部署成功后，您将获得一个 Worker URL，例如 `https://verwatch.your-subdomain.workers.dev`。

### 5. 使用 GitHub Actions 自动部署 (可选)

如果您希望通过 GitHub Actions 实现自动化部署（CI/CD），请在 GitHub 仓库的 **Settings -> Secrets and variables -> Actions** 中配置以下 Repository Secret：

- **CLOUDFLARE_API_TOKEN** (必需): 您的 Cloudflare API Token。
  - 创建地址：Cloudflare Profile > API Tokens
  - 权限模板：选择 "Edit Cloudflare Workers"。

推荐的 Workflow 配置 (`.github/workflows/deploy.yml`)：

```yaml
name: Deploy Worker

on:
  push:
    branches:
      - main

jobs:
  deploy:
    runs-on: ubuntu-latest
    name: Deploy
    steps:
      - uses: actions/checkout@v4
      - name: Deploy
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          wranglerVersion: "4.53.0"
          workingDirectory: "backend"
```

## 🎮 使用指南

### 1. 添加监控项目 (POST)

使用 curl 向 Worker 发送请求以添加监控规则。

- **API 端点**: `POST /api/projects`
- **Header**: `X-Auth-Key: <您设置的 ADMIN_SECRET>`

```bash
curl -X POST https://verwatch.your-subdomain.workers.dev/api/projects \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "base_config": {
      "upstream_owner": "fail2ban",
      "upstream_repo": "fail2ban",
      "my_owner": "my-github-user",
      "my_repo": "my-forked-repo"
    },
    "time_config": {
      "check_interval": { "secs": 3600, "nanos": 0 },
      "retry_interval": { "secs": 60, "nanos": 0 }
    },
    "comparison_mode": "published_at",
    "dispatch_token_secret": "MY_CUSTOM_TOKEN_VAR",
    "initial_delay": { "secs": 60, "nanos": 0 }
  }'
```

**字段说明**:
- `base_config`: 基础配置
  - `upstream_owner/repo`: 您想要监控的上游仓库。
  - `my_owner/repo`: 您想要触发更新的下游仓库（您自己的仓库）。
- `time_config`: 时间配置
  - `check_interval`: 检查间隔（默认 1 小时）
  - `retry_interval`: 失败重试间隔（默认 60 秒）
- `comparison_mode`: (必填) `published_at` (推荐) 或 `updated_at`。
- `dispatch_token_secret`: (可选) 在 Secrets 中配置的 Token 变量名。默认使用 `MY_GITHUB_PAT`。
- `initial_delay`: 首次检查的延迟时间。

### 2. 查看监控列表 (GET)

```bash
curl https://verwatch.your-subdomain.workers.dev/api/projects \
  -H "X-Auth-Key: my_super_secure_password"
```

### 3. 删除监控项目 (DELETE)

我们提供两种删除模式，请根据需求选择。

**方式 A: 标准删除 (Standard Delete)**
仅执行删除操作，不返回旧数据。响应快，语义标准。

- **Endpoint**: `DELETE /api/projects`
- **Response**: 
  - `204 No Content` (成功删除)
  - `404 Not Found` (资源不存在)

```bash
curl -X DELETE https://verwatch.your-subdomain.workers.dev/api/projects \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "fail2ban/fail2ban->my-github-user/my-forked-repo"
  }'
```

**方式 B: 移除并获取 (Pop & Delete)**
删除配置，并在响应中返回被删除的配置详情。

- **Endpoint**: `DELETE /api/projects/pop`
- **Response**: `200 OK` (Body: 被删除的 Config JSON)

```bash
curl -X DELETE https://verwatch.your-subdomain.workers.dev/api/projects/pop \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "fail2ban/fail2ban->my-github-user/my-forked-repo"
  }'
```

### 4. 切换监控状态 (POST)

暂停或恢复指定项目的监控任务。

- **Endpoint**: `POST /api/projects/switch`
- **Header**: `X-Auth-Key: <您设置的 ADMIN_SECRET>`

```bash
curl -X POST https://verwatch.your-subdomain.workers.dev/api/projects/switch \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "unique_key": "fail2ban/fail2ban->my-github-user/my-forked-repo",
    "paused": true
  }'
```

- `paused`: `true` 表示暂停监控，`false` 表示恢复运行。

### 5. 手动触发检查 (POST)

立即对指定项目执行一次版本检查，不影响原有的定时计划。

- **Endpoint**: `POST /api/projects/trigger`
- **Header**: `X-Auth-Key: <您设置的 ADMIN_SECRET>`

```bash
curl -X POST https://verwatch.your-subdomain.workers.dev/api/projects/trigger \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "unique_key": "fail2ban/fail2ban->my-github-user/my-forked-repo"
  }'
```

## 🤖 下游仓库配置 (GitHub Actions)

为了让您的仓库在接收到 `repository_dispatch` 事件后自动行动，请在您的仓库（即 `my_repo`）中创建如下 Workflow 文件。

**文件**: `.github/workflows/sync-upstream.yml`

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
        uses: actions/checkout@v3

      - name: Receive Version Info
        run: |
          echo "Upstream released new version: ${{ github.event.client_payload.version }}"
          
      # 在这里添加您的构建、合并或发布逻辑
      # 例如：
      # - 拉取上游代码
      # - 构建 Docker 镜像
      # - 推送新 Tag
```

## 📝 开发与测试

在本地运行开发服务器：

```bash
wrangler dev
```

运行单元测试：

```bash
cargo test
```

## 🔄 架构变更说明 (v2)

v2 版本进行了重大架构重构：

| 变更项 | v1 (旧) | v2 (新) |
|--------|---------|---------|
| **核心设计** | 单一 ProjectStore DO 存储所有配置 | 分布式 ProjectMonitor DO，每个项目独立 |
| **调度方式** | 中心化 Cron Job | 每个 Monitor 独立 Alarm 调度 |
| **扩展性** | 受单 DO 性能限制 | 天然水平扩展 |
| **配置存储** | ProjectStore 存储 Config | ProjectMonitor 自己存储 Config |
| **注册表** | N/A | ProjectRegistry 管理注册关系 |

## 📄 License

[MIT License](LICENSE)
