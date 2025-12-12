//! 路由服务模块 - 核心引擎
//!
//! 封装了 web_sys 的 History API，实现高内聚：
//! 所有对 window.history 的操作都集中在此模块。
//! 实现了"监听 -> 验证 -> 处理 -> 加载"的导航流程。

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::route::AppRoute;

/// 获取当前浏览器路径
fn current_path() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

/// 推送 History 状态（内部工具函数）
fn push_history_state(path: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            let _ = history.push_state_with_url(&JsValue::NULL, "", Some(path));
        }
    }
}

/// 替换 History 状态（内部工具函数，用于重定向）
fn replace_history_state(path: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(path));
        }
    }
}

/// 路由器服务
///
/// 封装所有路由操作，通过 Signal 驱动界面更新。
/// 通过注入认证检查信号实现与认证系统的解耦。
#[derive(Clone, Copy)]
pub struct RouterService {
    /// 当前路由（只读信号）
    current_route: ReadSignal<AppRoute>,
    /// 设置当前路由（写入信号）
    set_route: WriteSignal<AppRoute>,
    /// 认证状态检查（注入的信号，实现解耦）
    is_authenticated: Signal<bool>,
}

impl RouterService {
    /// 创建新的路由服务
    ///
    /// # Arguments
    /// * `is_authenticated` - 认证状态信号，由外部注入实现解耦
    fn new(is_authenticated: Signal<bool>) -> Self {
        // 1. 初始化当前路由（从 URL 解析）
        let path = current_path();
        let initial_route = AppRoute::from_path(&path);
        let (current_route, set_route) = signal(initial_route);

        Self {
            current_route,
            set_route,
            is_authenticated,
        }
    }

    /// 获取当前路由信号
    pub fn current_route(&self) -> ReadSignal<AppRoute> {
        self.current_route
    }

    /// **核心方法：导航与守卫**
    ///
    /// 流程：请求 -> 验证(Guard) -> 处理 -> 加载
    pub fn navigate(&self, path: &str) {
        let target_route = AppRoute::from_path(path);
        self.navigate_to_route(target_route, true);
    }

    /// 导航到指定路由
    ///
    /// # Arguments
    /// * `target_route` - 目标路由
    /// * `use_push` - true 使用 pushState, false 使用 replaceState
    fn navigate_to_route(&self, target_route: AppRoute, use_push: bool) {
        let is_auth = self.is_authenticated.get_untracked();

        // --- Step 1: 验证目标路由 ---
        // 如果目标需要认证但用户未认证
        if target_route.requires_auth() && !is_auth {
            web_sys::console::log_1(&"[Router] Access Denied. Redirecting to Login.".into());
            let redirect = AppRoute::auth_failure_redirect();
            if use_push {
                push_history_state(redirect.to_path());
            } else {
                replace_history_state(redirect.to_path());
            }
            self.set_route.set(redirect);
            return;
        }

        // 如果用户已认证但访问登录页，重定向到面板
        if target_route.should_redirect_when_authenticated() && is_auth {
            web_sys::console::log_1(
                &"[Router] Already authenticated. Redirecting to Dashboard.".into(),
            );
            let redirect = AppRoute::auth_success_redirect();
            if use_push {
                push_history_state(redirect.to_path());
            } else {
                replace_history_state(redirect.to_path());
            }
            self.set_route.set(redirect);
            return;
        }

        // --- Step 2: 加载页面 (更新状态) ---
        // 验证通过，推入 History 并更新 UI
        if use_push {
            push_history_state(target_route.to_path());
        } else {
            replace_history_state(target_route.to_path());
        }
        self.set_route.set(target_route);
    }

    /// 初始化浏览器后退/前进按钮监听
    fn init_popstate_listener(&self) {
        let set_route = self.set_route;
        let is_authenticated = self.is_authenticated;

        let closure = Closure::<dyn Fn()>::new(move || {
            let path = current_path();
            let target_route = AppRoute::from_path(&path);
            let is_auth = is_authenticated.get_untracked();

            // popstate 时也执行守卫逻辑
            if target_route.requires_auth() && !is_auth {
                // 阻止访问受保护页面
                let redirect = AppRoute::auth_failure_redirect();
                replace_history_state(redirect.to_path());
                set_route.set(redirect);
            } else {
                set_route.set(target_route);
            }
        });

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
        }

        // 泄漏闭包以保持监听器存活
        closure.forget();
    }

    /// 设置认证状态变化时的自动重定向
    fn setup_auth_redirect(&self) {
        let current_route = self.current_route;
        let set_route = self.set_route;
        let is_authenticated = self.is_authenticated;

        // 使用 Effect 监听认证状态变化
        Effect::new(move |_| {
            let is_auth = is_authenticated.get();
            let route = current_route.get_untracked();

            if is_auth {
                // 用户刚登录，如果在登录页则重定向到面板
                if route.should_redirect_when_authenticated() {
                    let redirect = AppRoute::auth_success_redirect();
                    push_history_state(redirect.to_path());
                    set_route.set(redirect);
                    web_sys::console::log_1(
                        &"[Router] Auth state changed: logged in, redirecting to dashboard.".into(),
                    );
                }
            } else {
                // 用户登出，如果在受保护页面则重定向到登录
                if route.requires_auth() {
                    let redirect = AppRoute::auth_failure_redirect();
                    push_history_state(redirect.to_path());
                    set_route.set(redirect);
                    web_sys::console::log_1(
                        &"[Router] Auth state changed: logged out, redirecting to login.".into(),
                    );
                }
            }
        });
    }
}

/// 提供路由服务到 Context 并初始化
fn provide_router(is_authenticated: Signal<bool>) -> RouterService {
    let router = RouterService::new(is_authenticated);

    // 初始化监听器
    router.init_popstate_listener();
    router.setup_auth_redirect();

    provide_context(router);
    router
}

/// 从 Context 获取路由服务
pub fn use_router() -> RouterService {
    use_context::<RouterService>()
        .expect("RouterService not found in context. Ensure Router is provided.")
}

/// 导航函数（返回一个可调用的闭包）
#[allow(dead_code)]
pub fn use_navigate() -> impl Fn(&str) + Clone {
    let router = use_router();
    move |to: &str| {
        router.navigate(to);
    }
}

// ============================================================================
// UI 组件
// ============================================================================

/// 路由器根组件
///
/// 提供路由上下文，应在 App 根部使用。
#[component]
pub fn Router(
    /// 认证状态信号
    is_authenticated: Signal<bool>,
    /// 子组件
    children: Children,
) -> impl IntoView {
    // 提供路由服务到 Context
    provide_router(is_authenticated);

    children()
}

/// 路由出口组件
///
/// 根据当前路由状态渲染对应的组件。
#[component]
pub fn RouterOutlet(
    /// 路由匹配函数：接收当前路由，返回对应视图
    matcher: fn(AppRoute) -> AnyView,
) -> impl IntoView {
    let router = use_router();

    move || {
        let current = router.current_route().get();
        matcher(current)
    }
}

// #[allow(dead_code)]
// #[component]
// pub fn Link(
//     /// 目标路径
//     #[prop(into)]
//     to: String,
//     /// 子内容
//     children: Children,
// ) -> impl IntoView {
//     let router = use_router();

//     let to_clone = to.clone();
//     let on_click = move |ev: web_sys::MouseEvent| {
//         ev.prevent_default();
//         router.navigate(&to_clone);
//     };

//     view! {
//         <a href=to on:click=on_click>
//             {children()}
//         </a>
//     }
// }
