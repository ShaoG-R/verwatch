//! VerWatch 前端应用
//!
//! 采用 Silex 标准路由架构：
//! - 使用 `#[derive(Route)]` 宏定义路由
//! - 使用 `Router` 组件和 `route.render()` 渲染
//! - `auth`: 认证状态管理
//! - `components`: UI 组件层

mod api;
mod auth;
mod components {
    mod add_project_dialog;
    pub mod dashboard;
    mod icons;
    pub mod login;
}
mod serde_helper;

use crate::auth::{AuthContext, init_auth, use_auth};
use crate::components::dashboard::DashboardPage;
use crate::components::login::LoginPage;

use silex::prelude::*;

// 原生 Web API 封装模块
// 此模块提供对浏览器原生 API 的轻量级封装，替代 gloo-* 系列 crate，
// 以减小 WASM 二进制体积。
pub(crate) mod web {
    mod http;
    mod storage;

    pub use http::HttpClient;
    pub use storage::LocalStorage;
}

// ============================================================================
// 路由定义 - 使用 Silex 标准路由宏
// ============================================================================

/// 404 页面组件
#[component]
fn NotFoundPage() -> impl View {
    div(div![
        h1("404").class("text-6xl font-bold text-error"),
        p("页面未找到").class("text-xl mt-4"),
        Link("/", "返回首页").class("btn btn-primary mt-4")
    ]
    .class("text-center"))
    .class("flex items-center justify-center min-h-screen bg-base-200")
}

/// 认证守卫组件
///
/// 如果用户未认证，重定向到登录页。
/// 如果正在加载，显示加载状态。
#[component]
fn AuthGuard(children: Children) -> impl View {
    let auth = use_auth();
    let navigator = use_context::<RouterContext>()
        .expect("RouterContext not found")
        .navigator;

    Effect::new(move |_| {
        let state = auth.state.get();
        if !state.is_loading && !state.is_authenticated {
            navigator.push("/");
        }
    });

    Dynamic::new(move || {
        let state = auth.state.get();
        if state.is_loading {
            div(span(()).class("loading loading-spinner loading-lg text-primary"))
                .class("flex items-center justify-center min-h-screen")
                .into_any()
        } else if !state.is_authenticated {
            // 重定向中...
            div(()).into_any()
        } else {
            children.clone()
        }
    })
}

/// 访客守卫组件
///
/// 如果用户已认证，重定向到 Dashboard。
#[component]
fn GuestGuard(children: Children) -> impl View {
    let auth = use_auth();
    let navigator = use_context::<RouterContext>()
        .expect("RouterContext not found")
        .navigator;

    Effect::new(move |_| {
        let state = auth.state.get();
        if !state.is_loading && state.is_authenticated {
            navigator.push("/dashboard");
        }
    });

    Dynamic::new(move || {
        let state = auth.state.get();
        // 此处不需要 loading 状态，因为登录页本身可以处理
        if state.is_authenticated {
            // 重定向中...
            div(()).into_any()
        } else {
            children.clone()
        }
    })
}

/// 应用路由枚举
#[derive(Route, Clone, PartialEq)]
pub enum AppRoute {
    #[route("/", view = LoginPage, guard = GuestGuard)]
    Login,
    #[route("/dashboard", view = DashboardPage, guard = AuthGuard)]
    Dashboard,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

// ============================================================================
// 应用入口
// ============================================================================

#[component]
pub fn App() -> impl View {
    // 1. 创建认证上下文
    let auth_ctx = AuthContext::new();
    provide_context(auth_ctx);

    // 2. 初始化认证状态（从 LocalStorage 加载 URL）
    init_auth(&auth_ctx);

    // 3. 路由器组件
    Router::new().match_route::<AppRoute>()
}
