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

## 开发运行 (Running Locally)

1.  进入 `frontend` 目录：
    ```bash
    cd frontend
    ```

2.  启动开发服务器：
    ```bash
    trunk serve
    ```
    或者自动并在浏览器打开：
    ```bash
    trunk serve --open
    ```

*   默认服务地址为：`http://127.0.0.1:8080`
*   **后端连接**: 默认情况下，前端可能需要连接到后端 Worker。请在登录界面输入您的 VerWatch 后端 URL 和 Admin Secret。
*   **热重载**:Trunk 支持热重载，修改代码后浏览器会自动刷新。

## 构建发布 (Build for Production)

构建优化后的生产环境静态文件：

```bash
trunk build --release
```

*   构建完成后的文件位于 `dist/` 目录。
*   `dist` 目录包含了所有需要的 HTML、JS、WASM 和 资源文件。
*   您可以将此目录部署到任何静态托管服务（如 Cloudflare Pages, GitHub Pages, Vercel, Netlify 或 Nginx 服务器）。

### 部署到 Cloudflare Pages (推荐)

如果您使用 Cloudflare Pages 部署，可以在构建设置中配置：
*   **Build command**: `curl -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env" && rustup target add wasm32-unknown-unknown && cargo install trunk && trunk build --release` (或者使用预装 Rust 环境)
*   **Build output directory**: `frontend/dist`
*   **Root directory**: `frontend`

## 项目结构

*   `src/`: 源代码
    *   `components/`: Leptos UI 组件 (`dashboard.rs`, `login.rs` 等)
    *   `api.rs`: 与后端 Worker 通信的 API 客户端
    *   `auth.rs`: 处理登录状态和 LocalStorage
*   `index.html`: 应用入口 HTML，包含 TailwindCSS 和 DaisyUI 的 CDN 引用。
*   `Cargo.toml`: Rust 依赖定义。

## 样式说明

本项目为了简化开发流程，直接在 `index.html` 中通过 CDN 引入了 **TailwindCSS v4** (Alpha) 和 **DaisyUI v5** (Beta)。
*   无需配置 `npm install` 或 `postcss`。
*   **注意**:这也意味着开发和运行时客户端需要能够访问 `cdn.jsdelivr.net` 和 `unpkg.com`。

## 依赖关系

前端项目依赖于仓库根目录下的 `shared` crate (`../shared`)，其中定义了前后端共用的数据结构（如 `ProjectConfig`, `CreateProjectRequest`）。请确保整个仓库代码完整。
