# VerWatch: Serverless GitHub Release Monitor

**VerWatch** 是一个基于 Rust 和 Cloudflare Workers 构建的轻量级“看门狗”服务。它能够定期监控上游 GitHub 仓库的最新 Release 版本，一旦发现更新，就会自动通过 `repository_dispatch` 事件触发您自己仓库的 GitHub Actions 工作流。

它是维护 Fork 版本、Docker 镜像自动构建或同步上游更新的理想工具。

## ✨ 特性

- **轻量高效**：基于 Cloudflare Workers 运行，无服务器维护成本。
- **安全可靠**：支持 GitHub Token 和 Admin Secret 加密存储。
- **配置灵活**：支持自定义版本对比模式（发布时间 vs 更新时间）。
- **Rust 驱动**：利用 Rust 的强类型和高性能特性。
- **KV 存储**：使用 Cloudflare KV 存储配置和状态，持久化且低延迟。

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
git clone [https://github.com/your-username/verwatch.git](https://github.com/your-username/verwatch.git)
cd verwatch
```

### 2. 创建 KV Namespace

我们需要一个 KV 存储空间来保存监控列表和版本历史。

```bash
wrangler kv namespace create VERSION_STORE
```

执行后，终端会输出类似以下内容，请记录下 `id`：

```toml
[kv_namespaces]
binding = "VERSION_STORE"
id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
```

### 3. 配置 `wrangler.toml`

在项目根目录修改 `wrangler.toml` 文件。请将上一步获得的 KV ID 填入：

```toml
name = "verwatch"
main = "build/worker/shim.mjs"
compatibility_date = "2023-01-01"

# 绑定 KV 存储
[[kv_namespaces]]
binding = "VERSION_STORE"
id = "<替换为你的_KV_ID>"

# 环境变量配置 (Vars)
[vars]
# KV 绑定名称，需与上面的 binding 保持一致
KV_BINDING = "VERSION_STORE"
# 存储监控列表的 Key
CONFIG_KEY = "WATCH_LIST_CONFIG"
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

### 4. 设置敏感密钥 (Secrets)

为了安全起见，Token 不应明文写在配置文件中，请使用 `wrangler secret` 命令上传。

1. **ADMIN_SECRET**: 用于保护您的管理 API（添加/删除监控项目）。
   ```bash
   wrangler secret put ADMIN_SECRET
   # 输入一个复杂的密码，例如: my_super_secure_password
   ```

2. **GITHUB_TOKEN** (可选但推荐): 用于读取上游仓库 Release 信息（避免 API 速率限制）。
   ```bash
   wrangler secret put GITHUB_TOKEN
   # 输入您的 GitHub Personal Access Token (Fine-grained personal access tokens 下无需勾选)
   ```

3. **MY_GITHUB_PAT**: 用于触发下游仓库的 Dispatch 事件（必须有写权限）。
   ```bash
   wrangler secret put MY_GITHUB_PAT
   # 输入您的 GitHub PAT (Fine-grained personal access tokens 下勾选Context，设置Read and Write)
   ```

### 5. 部署到 Cloudflare

```bash
wrangler deploy
```

部署成功后，您将获得一个 Worker URL，例如 `https://verwatch.your-subdomain.workers.dev`。

---

## 🎮 使用指南

### 1. 添加监控项目 (POST)

使用 `curl` 向 Worker 发送请求以添加监控规则。

**API 端点**: `POST /api/projects`
**Header**: `X-Auth-Key: <您设置的 ADMIN_SECRET>`

```bash
curl -X POST [https://verwatch.your-subdomain.workers.dev/api/projects](https://verwatch.your-subdomain.workers.dev/api/projects) \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "upstream_owner": "fail2ban",
    "upstream_repo": "fail2ban",
    "my_owner": "my-github-user",
    "my_repo": "my-forked-repo",
    "comparison_mode": "published_at"
  }'
```

**字段说明**:
- `upstream_owner/repo`: 您想要监控的上游仓库。
- `my_owner/repo`: 您想要触发更新的下游仓库（您自己的仓库）。
- `comparison_mode`: `published_at` (推荐) 或 `updated_at`。
- `dispatch_token`: (可选) 如果该仓库需要特定的 Token，可以在此覆盖全局 Token。

### 2. 查看监控列表 (GET)

```bash
curl [https://verwatch.your-subdomain.workers.dev/api/projects](https://verwatch.your-subdomain.workers.dev/api/projects)
```

### 3. 删除监控项目 (DELETE)

```bash
curl -X DELETE [https://verwatch.your-subdomain.workers.dev/api/projects](https://verwatch.your-subdomain.workers.dev/api/projects) \
  -H "X-Auth-Key: my_super_secure_password" \
  -H "Content-Type: application/json" \
  -d '{
    "upstream_owner": "fail2ban",
    "upstream_repo": "fail2ban"
  }'
```

### 4. 手动触发检查 (调试用)

由于 Cloudflare Worker 的 Cron 触发器在开发环境较难测试，您可以暂时在 `lib.rs` 中添加一个临时的 HTTP 路由来手动调用 `WatchdogService` 的 `run_all` 方法，或者直接等待定时任务执行。

---

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