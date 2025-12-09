# VerWatch: Serverless GitHub Release Monitor

**VerWatch** 是一个基于 Rust 和 Cloudflare Workers (Durable Objects) 构建的轻量级“看门狗”服务。它能够定期监控上游 GitHub 仓库的最新 Release 版本，一旦发现更新，就会自动通过 `repository_dispatch` 事件触发您自己仓库的 GitHub Actions 工作流。

它是维护 Fork 版本、Docker 镜像自动构建或同步上游更新的理想工具。

## ✨ 特性

- **轻量高效**：基于 Cloudflare Workers 运行，无服务器维护成本。
- **强一致性**：使用 **Durable Objects** 存储配置和状态，解决了最终一致性问题，并支持原子操作。
- **安全可靠**：支持 GitHub Token 和 Admin Secret 加密存储。
- **配置灵活**：支持自定义版本对比模式（发布时间 vs 更新时间）。
- **Rust 驱动**：利用 Rust 的强类型和高性能特性。
- **跨域支持**：内置 CORS 支持，允许前端应用直接调用 API。

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
cd verwatch
```

### 2. 配置 wrangler.toml

在项目 `backend` 目录修改 `wrangler.toml` 文件。我们现在使用 Durable Objects 代替 KV：

```toml
name = "verwatch"
main = "build/worker/shim.mjs"
compatibility_date = "2023-01-01"

# 显式开启 workers.dev 域名
workers_dev = true

# 替换 KV 为 Durable Object 绑定
[durable_objects]
bindings = [
    # class_name 需与 durable_object.rs 中的 impl DurableObject for ProjectStore 中的 class_name 一致 
    { name = "PROJECT_STORE", class_name = "ProjectStore" } 
]
# 环境变量配置 (Vars)
[vars]
# DO 绑定名称，需与上面的 binding 保持一致
DO_BINDING = "PROJECT_STORE"
# 以下变量定义了 Secret 的"变量名"，保持默认即可
ADMIN_SECRET_NAME = "ADMIN_SECRET"
GITHUB_TOKEN_NAME = "GITHUB_TOKEN"
PAT_TOKEN_NAME = "MY_GITHUB_PAT"

# 定时任务配置 (Cron Triggers)
# 示例：每小时运行一次
[triggers]
crons = ["0 * * * *"]

[build]
command = "cargo install -q worker-build && worker-build --release"
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
# 输入您的 GitHub PAT (Fine-grained personal access tokens 下勾选Context，设置Read and Write)
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
    "upstream_owner": "fail2ban",
    "upstream_repo": "fail2ban",
    "my_owner": "my-github-user",
    "my_repo": "my-forked-repo",
    "comparison_mode": "published_at",
    "dispatch_token_secret": "MY_CUSTOM_TOKEN_VAR"
  }'
```

**字段说明**:
- `upstream_owner/repo`: 您想要监控的上游仓库。
- `my_owner/repo`: 您想要触发更新的下游仓库（您自己的仓库）。
- `comparison_mode`: (必填) `published_at` (推荐) 或 `updated_at`。
- `dispatch_token_secret`: (可选) **重要更新**：此处需填写在 `wrangler` Secrets 或 Vars 中配置的变量名称（例如 `MY_CUSTOM_TOKEN_VAR`），而不是 Token 明文。如果不填，默认使用全局配置的 `MY_GITHUB_PAT`。

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
- **Response**: `204 No Content`

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

### 4. 暂停/恢复监控 (POST)

切换项目的暂停状态。暂停后，定时任务将跳过对该项目的检查。

- **Endpoint**: `POST /api/projects/toggle_pause`
- **Response**: `200 OK` (Body: `true` 表示已暂停, `false` 表示运行中)

```bash
curl -X POST https://verwatch.your-subdomain.workers.dev/api/projects/toggle_pause \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "fail2ban/fail2ban->my-github-user/my-forked-repo"
  }'
```

### 5. 手动触发检查 (调试用)

由于 Cloudflare Worker 的 Cron 触发器在开发环境较难测试，您可以等待定时任务执行，或者在本地使用 `wrangler dev --test-scheduled` 进行模拟。

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

## 📄 License

[MIT License](LICENSE)
