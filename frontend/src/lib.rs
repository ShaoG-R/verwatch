mod api;
mod auth;
mod components {
    mod add_project_dialog;
    pub mod dashboard;
    mod icons;
    pub mod login;
}
mod serde_helper;

use crate::auth::{AuthContext, AuthState, init_auth};
use crate::components::dashboard::DashboardPage;
use crate::components::login::LoginPage;

use leptos::prelude::*;

// 原生 Web API 封装模块
// 此模块提供对浏览器原生 API 的轻量级封装，替代 gloo-* 系列 crate，
// 以减小 WASM 二进制体积。
pub(crate) mod web {
    mod http;
    pub mod router;
    mod storage;
    mod timer;

    pub use http::HttpClient;
    pub use router::use_navigate;
    pub use storage::LocalStorage;
    pub use timer::Interval;
}

use web::router::{Router, Routes};

/// 路由匹配函数
fn route_matcher(path: String) -> AnyView {
    match path.as_str() {
        "/" => view! { <LoginPage /> }.into_any(),
        "/dashboard" => view! { <DashboardPage /> }.into_any(),
        _ => view! { <div class="p-8 text-center">"页面未找到"</div> }.into_any(),
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (auth_state, set_auth) = signal(AuthState::default());
    provide_context(AuthContext(auth_state, set_auth));

    // Initialize authentication from local storage
    init_auth(set_auth);

    view! {
        <Router>
            <Routes matcher=route_matcher />
        </Router>
    }
}
