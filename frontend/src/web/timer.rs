//! 定时器封装模块
//!
//! 使用 `web_sys` 的原生定时器 API 替代 `gloo-timers`。

use wasm_bindgen::prelude::*;

/// 周期性定时器
///
/// 封装 `setInterval` API。当 `Interval` 被 drop 时，自动清除定时器。
pub struct Interval {
    handle: i32,
    #[allow(dead_code)]
    closure: Closure<dyn Fn()>,
}

impl Interval {
    /// 创建新的周期性定时器
    ///
    /// # 参数
    /// - `millis`: 间隔时间（毫秒）
    /// - `callback`: 每次间隔触发的回调函数
    ///
    /// # Panics
    /// 如果无法获取 window 对象或设置定时器失败
    pub fn new<F>(millis: u32, callback: F) -> Self
    where
        F: Fn() + 'static,
    {
        let closure = Closure::new(callback);
        let window = web_sys::window().expect("无法获取 window 对象");

        let handle = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                millis as i32,
            )
            .expect("设置定时器失败");

        Self { handle, closure }
    }

    /// 取消定时器
    ///
    /// 通常不需要手动调用，因为 drop 时会自动清除。
    pub fn cancel(&self) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(self.handle);
        }
    }
}

impl Drop for Interval {
    fn drop(&mut self) {
        self.cancel();
    }
}
