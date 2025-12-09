use crate::api::VerWatchApi;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;

const STORAGE_URL_KEY: &str = "verwatch_url";

#[derive(Clone, Debug, Default)]
pub struct AuthState {
    pub api: Option<VerWatchApi>,
    pub is_authenticated: bool,
    pub is_loading: bool,
    pub backend_url: String,
}

#[derive(Clone, Copy)]
pub struct AuthContext(pub ReadSignal<AuthState>, pub WriteSignal<AuthState>);

pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>().expect("AuthContext should be provided")
}

pub fn init_auth(set_auth: WriteSignal<AuthState>) {
    // 安全性修改：不再自动从 Storage 加载 Secret
    // 仅为了方便用户体验，我们可以选择记住 URL（非敏感信息），但绝对不加载 Secret。
    // 这意味着用户每次刷新都需要重新输入密码。

    set_auth.update(|state| {
        state.is_loading = false;
        // 尝试加载上次的 URL 方便输入，但状态仍未认证
        if let Ok(url) = LocalStorage::get::<String>(STORAGE_URL_KEY) {
            state.backend_url = url;
        }
    });
}

/// 登录并保存状态 (仅内存)
pub async fn login(set_auth: WriteSignal<AuthState>, url: String, secret: String) -> bool {
    let api = VerWatchApi::new(url.clone(), secret.clone());

    // 验证凭据是否有效
    if api.get_projects().await.is_ok() {
        // 成功：只保存 URL 到 LocalStorage 以便下次自动填充，但不保存 Secret
        let _ = LocalStorage::set(STORAGE_URL_KEY, &url);

        // 确保清除旧的 Secret (如果存在)
        let _ = LocalStorage::delete("verwatch_secret");

        // 更新内存状态
        set_auth.update(|state| {
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
pub fn logout(set_auth: WriteSignal<AuthState>) {
    // 清除内存状态
    set_auth.update(|state| {
        state.api = None;
        state.is_authenticated = false;
        state.backend_url = String::new(); // 这里可以选择是否保留 URL
    });
    // 导航应该由监听 Auth 状态的组件处理
}
