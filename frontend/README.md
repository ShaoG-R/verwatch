# VerWatch Frontend

VerWatch 的前端控制面板，基于 Rust + Leptos + TailwindCSS (DaisyUI) 构建的单页应用 (SPA)。

## 环境准备 (Prerequisites)

在运行此项目之前，请确保您的开发环境已安装以下工具：

1.  **Rust**: 如果尚未安装，请参考 [Rust 官网](https://www.rust-lang.org/tools/install) 进行安装。
    ```bash
    # Linux/macOS
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```

2.  **WebAssembly Target**: 为了编译成 Wasm，需要添加 `wasm32-unknown-unknown` 构建目标。
    ```bash
    rustup target add wasm32-unknown-unknown
    ```

3.  **Trunk**: 我们使用 [Trunk](https://trunkrs.dev/) 作为构建和打包工具。
    ```bash
    cargo install trunk
    ```

4.  **Node.js**: 用于构建优化的 CSS 文件。
    ```bash
    # 安装 npm 依赖
    cd frontend
    npm install
    ```

## 开发运行 (Running Locally)

1.  进入 `frontend` 目录：
    ```bash
    cd frontend
    ```

2.  首次运行或修改了 Tailwind/DaisyUI 类后，需要构建 CSS：
    ```bash
    npm run build:css
    ```
    或者开启 CSS 监听模式（自动重新构建）：
    ```bash
    npm run watch:css
    ```

3.  启动开发服务器：
    ```bash
    trunk serve
    ```
    或者自动在浏览器打开：
    ```bash
    trunk serve --open
    ```

*   默认服务地址为：`http://127.0.0.1:8080`
*   **后端连接**: 默认情况下，前端可能需要连接到后端 Worker。请在登录界面输入您的 VerWatch 后端 URL 和 Admin Secret。
*   **热重载**: Trunk 支持热重载，修改代码后浏览器会自动刷新。

## 构建发布 (Build for Production)

构建优化后的生产环境静态文件：

```bash
npm run build:css  # 构建精简的 CSS
trunk build --release
```

*   构建完成后的文件位于 `dist/` 目录。
*   `dist` 目录包含了所有需要的 HTML、JS、WASM 和 资源文件。
*   您可以将此目录部署到任何静态托管服务（如 Cloudflare Pages, GitHub Pages, Vercel, Netlify 或 Nginx 服务器）。

### 部署到 Cloudflare Pages (推荐)

#### 方式 A: 手动配置 Cloudflare Pages

如果您使用 Cloudflare Pages 部署，可以在构建设置中配置：
*   **Build command**: `curl -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env" && rustup target add wasm32-unknown-unknown && cargo install trunk && trunk build --release` (或者使用预装 Rust 环境)
*   **Build output directory**: `frontend/dist`
*   **Root directory**: `frontend`

#### 方式 B: 使用 GitHub Actions 自动部署 (推荐)

如果您希望通过 GitHub Actions 实现自动化部署（CI/CD），请在 GitHub 仓库的 **Settings → Secrets and variables → Actions** 中配置以下 Repository Secrets：

* `CLOUDFLARE_API_TOKEN`: 您的 Cloudflare API Token 

**获取方式:**
1.  **CLOUDFLARE_API_TOKEN**: 
    - 创建地址：Cloudflare Dashboard → My Profile → API Tokens
    - 权限模板：选择 "Edit Cloudflare Pages"
2.  **CLOUDFLARE_ACCOUNT_ID**: 
    - 获取地址：Cloudflare Dashboard → Workers & Pages → 右侧边栏

**工作流配置** (`.github/workflows/deploy_frontend.yml`):

```yaml
name: Deploy Frontend

on:
  push:
    branches:
      - main
    paths:
      - frontend/**
      - shared/**
      - '!frontend/**/*.md'
      - .github/workflows/deploy_frontend.yml

jobs:
  deploy:
    runs-on: ubuntu-latest
    name: Build and Deploy
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install Trunk
        run: cargo install trunk --locked

      - name: Build CSS
        working-directory: frontend
        run: npm install && npm run build:css

      - name: Build WASM
        working-directory: frontend
        run: trunk build --release

      - name: Deploy to Cloudflare Pages
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          command: pages deploy frontend/dist --project-name=verwatch
```

**触发条件:**
-   推送到 `main` 分支
-   更改 `frontend/` 或 `shared/` 目录下的文件（不包括 `.md` 文件）

## 项目结构

*   `src/`: 源代码
    *   `components/`: Leptos UI 组件 (`dashboard.rs`, `login.rs` 等)
    *   `api.rs`: 与后端 Worker 通信的 API 客户端
    *   `auth.rs`: 处理登录状态和 LocalStorage
*   `index.html`: 应用入口 HTML。
*   `Cargo.toml`: Rust 依赖定义。
*   `package.json`: Node.js 依赖和 CSS 构建脚本。
*   `input.css`: Tailwind CSS 入口文件。

## 样式说明

本项目使用 **Tailwind CSS v4** + **DaisyUI v5** 作为样式框架，采用构建时优化：

*   **Tree Shaking**: Tailwind 会扫描 `./src/**/*.rs` 和 `./index.html` 中使用的类名，只生成实际使用的 CSS。
*   **精简输出**: 原始 DaisyUI CSS 约 **968KB**，优化后仅约 **126KB**（减少 ~87%）。
*   **无运行时依赖**: 不需要加载 CDN 资源，所有样式都在构建时打包。

构建命令：
```bash
npm run build:css   # 构建压缩的 CSS
npm run watch:css   # 开发时监听文件变化自动重建
```

## 依赖关系

前端项目依赖于仓库根目录下的 `shared` crate (`../shared`)，其中定义了前后端共用的数据结构（如 `ProjectConfig`, `CreateProjectRequest`）。请确保整个仓库代码完整。
