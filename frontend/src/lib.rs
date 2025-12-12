//! VerWatch 前端应用
//!
//! 采用 Context-Driven 的高内聚低耦合架构：
//! - `web::route`: 路由定义（领域模型）
//! - `web::router`: 路由服务（核心引擎）
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

use crate::auth::{AuthContext, init_auth};
use crate::components::dashboard::DashboardPage;
use crate::components::login::LoginPage;

use leptos::prelude::*;

// 原生 Web API 封装模块
// 此模块提供对浏览器原生 API 的轻量级封装，替代 gloo-* 系列 crate，
// 以减小 WASM 二进制体积。
pub(crate) mod web {
    mod http;
    pub mod route;
    pub mod router;
    mod storage;
    mod timer;

    pub use http::HttpClient;
    pub use storage::LocalStorage;
    pub use timer::Interval;
}

use web::route::AppRoute;
use web::router::{Router, RouterOutlet};

/// 路由匹配函数
///
/// 根据 AppRoute 枚举返回对应的视图组件。
fn route_matcher(route: AppRoute) -> AnyView {
    match route {
        AppRoute::Login => view! { <LoginPage /> }.into_any(),
        AppRoute::Dashboard => view! { <DashboardPage /> }.into_any(),
        AppRoute::NotFound => view! {
            <div class="flex items-center justify-center min-h-screen bg-base-200">
                <div class="text-center">
                    <h1 class="text-6xl font-bold text-error">"404"</h1>
                    <p class="text-xl mt-4">"页面未找到"</p>
                </div>
            </div>
        }
        .into_any(),
    }
}

#[component]
pub fn App() -> impl IntoView {
    // 1. 创建认证上下文
    let auth_ctx = AuthContext::new();
    provide_context(auth_ctx);

    // 2. 初始化认证状态（从 LocalStorage 加载 URL）
    init_auth(&auth_ctx);

    // 3. 获取认证状态信号，用于注入路由服务（解耦！）
    let is_authenticated = auth_ctx.is_authenticated_signal();

    view! {
        // 4. 路由器组件：注入认证信号实现守卫
        <Router is_authenticated=is_authenticated>
            <RouterOutlet matcher=route_matcher />
        </Router>
    }
}
