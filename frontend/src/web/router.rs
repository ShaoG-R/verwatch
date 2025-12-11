//! 轻量级路由模块
//!
//! 使用 `web_sys` 的 History API 替代 `leptos_router`，
//! 提供基本的 URL 路径映射功能，大幅减小 WASM 体积。

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

/// 获取当前路径
fn current_path() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

/// 路由上下文
#[derive(Clone, Copy)]
pub struct RouterContext {
    /// 当前路径信号
    path: ReadSignal<String>,
    set_path: WriteSignal<String>,
}

impl RouterContext {
    /// 导航到指定路径
    pub fn navigate(&self, to: &str) {
        if let Some(window) = web_sys::window() {
            if let Ok(history) = window.history() {
                // 使用 pushState 更新 URL 但不刷新页面
                let _ = history.push_state_with_url(&JsValue::NULL, "", Some(to));
                self.set_path.set(to.to_string());
            }
        }
    }

    /// 获取当前路径
    #[allow(dead_code)]
    pub fn path(&self) -> String {
        self.path.get()
    }
}

/// 获取路由上下文
pub fn use_router() -> RouterContext {
    use_context::<RouterContext>().expect("RouterContext should be provided by Router component")
}

/// 导航函数（返回一个可调用的闭包）
pub fn use_navigate() -> impl Fn(&str) + Clone {
    let router = use_router();
    move |to: &str| {
        router.navigate(to);
    }
}

/// 路由器组件
///
/// 提供路由上下文并监听浏览器的 popstate 事件（后退/前进按钮）
#[component]
pub fn Router(children: Children) -> impl IntoView {
    let (path, set_path) = signal(current_path());

    // 监听 popstate 事件（浏览器后退/前进）
    Effect::new(move |_| {
        let closure = Closure::<dyn Fn()>::new(move || {
            set_path.set(current_path());
        });

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
        }

        // 保持 closure 存活
        closure.forget();
    });

    provide_context(RouterContext { path, set_path });

    children()
}

/// 路由匹配组件
///
/// 根据当前路径渲染对应的视图
#[component]
pub fn Routes(
    /// 路由匹配函数：接收当前路径，返回对应视图
    matcher: fn(String) -> AnyView,
) -> impl IntoView {
    let router = use_router();

    move || {
        let current = router.path.get();
        matcher(current)
    }
}
