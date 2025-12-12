//! 认证模块
//!
//! 管理用户认证状态，与路由系统解耦。
//! 路由服务通过注入的认证信号来检查认证状态。

use crate::api::VerWatchApi;
use crate::web::LocalStorage;
use leptos::prelude::*;

const STORAGE_URL_KEY: &str = "verwatch_url";

/// 认证状态
#[derive(Clone, Default)]
pub struct AuthState {
    /// API 客户端实例（仅在认证成功后存在）
    pub api: Option<VerWatchApi>,
    /// 是否已认证
    pub is_authenticated: bool,
    /// 是否正在加载
    pub is_loading: bool,
    /// 后端 URL（用于 UI 显示和自动填充）
    pub backend_url: String,
}

/// 认证上下文
///
/// 包含读写信号，通过 Context 在组件间共享。
#[derive(Clone, Copy)]
pub struct AuthContext {
    /// 认证状态（只读）
    pub state: ReadSignal<AuthState>,
    /// 设置认证状态（写入）
    pub set_state: WriteSignal<AuthState>,
}

impl AuthContext {
    /// 创建新的认证上下文
    pub fn new() -> Self {
        let (state, set_state) = signal(AuthState::default());
        Self { state, set_state }
    }

    /// 获取认证状态信号（用于路由服务注入）
    pub fn is_authenticated_signal(&self) -> Signal<bool> {
        let state = self.state;
        Signal::derive(move || state.get().is_authenticated)
    }
}

/// 从 Context 获取认证上下文
pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>().expect("AuthContext should be provided")
}

/// 初始化认证状态
///
/// 从 LocalStorage 加载上次的 URL（方便用户），但不加载密钥（安全性）。
pub fn init_auth(ctx: &AuthContext) {
    ctx.set_state.update(|state| {
        state.is_loading = false;
        // 尝试加载上次的 URL 方便输入，但状态仍未认证
        if let Some(url) = LocalStorage::get(STORAGE_URL_KEY) {
            state.backend_url = url;
        }
    });
}

/// 登录并保存状态 (仅内存)
///
/// # Arguments
/// * `ctx` - 认证上下文
/// * `url` - 后端 URL
/// * `secret` - 管理密钥
///
/// # Returns
/// 登录是否成功
pub async fn login(ctx: &AuthContext, url: String, secret: String) -> bool {
    let api = VerWatchApi::new(url.clone(), secret.clone());

    // 验证凭据是否有效
    if api.get_projects().await.is_ok() {
        // 成功：只保存 URL 到 LocalStorage 以便下次自动填充，但不保存 Secret
        LocalStorage::set(STORAGE_URL_KEY, &url);

        // 确保清除旧的 Secret (如果存在)
        LocalStorage::delete("verwatch_secret");

        // 更新内存状态
        ctx.set_state.update(|state| {
            state.api = Some(api);
            state.backend_url = url;
            state.is_authenticated = true;
        });
        true
    } else {
        false
    }
}

/// 注销并清除状态
///
/// 导航将由路由服务的认证状态监听自动处理。
pub fn logout(ctx: &AuthContext) {
    ctx.set_state.update(|state| {
        state.api = None;
        state.is_authenticated = false;
        // 保留 URL 方便下次登录
    });
    // 注意：不需要手动导航，路由服务会监听认证状态变化并自动重定向
}
