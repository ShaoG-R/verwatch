pub mod adapter;
pub mod protocol;
mod registry;

use crate::error::{AppError, Result};
use protocol::*;
use verwatch_shared::ProjectConfig;
use worker::{wasm_bindgen::JsValue, *};

// =========================================================
// Registry Trait (面向 ProjectRegistry DO)
// =========================================================

#[async_trait::async_trait(?Send)]
pub trait Registry {
    /// 注册一个 Monitor (内部调用 ProjectMonitor.setup)
    async fn register(&self, config: &ProjectConfig) -> Result<String>;
    /// 注销一个 Monitor (内部调用 ProjectMonitor.stop)
    async fn unregister(&self, unique_key: &str) -> Result<bool>;
    /// 列出所有已注册的 Monitor 的 Config
    async fn list(&self) -> Result<Vec<ProjectConfig>>;
    /// 检查是否已注册
    async fn is_registered(&self, unique_key: &str) -> Result<bool>;
    /// 切换 Monitor 监控状态
    async fn switch_monitor(&self, unique_key: &str, paused: bool) -> Result<bool>;
    /// 手动触发 Monitor 检查
    async fn trigger_check(&self, unique_key: &str) -> Result<bool>;
}

// =========================================================
// Durable Object 实现
// =========================================================

pub struct DoProjectRegistry {
    stub: Stub,
}

impl DoProjectRegistry {
    pub fn new(env: &Env, binding_name: &str) -> Result<Self> {
        let namespace = env.durable_object(binding_name)?;
        // Registry 是单例，使用固定 ID
        let id = namespace.id_from_name("default")?;
        let stub = id.get_stub()?;
        Ok(Self { stub })
    }

    /// 核心泛型方法：执行 RPC 请求
    async fn execute<T: ApiRequest>(&self, req: T) -> Result<T::Response> {
        // 1. 序列化请求
        let body = serde_json::to_string(&req)?;

        // 2. 构造 Request
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(T::METHOD).with_headers(headers);

        if T::METHOD != Method::Get && T::METHOD != Method::Head {
            init.with_body(Some(JsValue::from_str(&body)));
        }

        let url = format!("http://registry{}", T::PATH);
        let request = Request::new_with_init(&url, &init)?;

        // 3. 发送请求
        let mut response = self.stub.fetch_with_request(request).await?;

        // 4. 处理 DO 内部逻辑错误
        if response.status_code() != 200 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Store(format!(
                "Registry Error [{}]: {}",
                response.status_code(),
                error_text
            )));
        }

        // 5. 反序列化响应
        let data = response.json::<T::Response>().await?;
        Ok(data)
    }
}

#[async_trait::async_trait(?Send)]
impl Registry for DoProjectRegistry {
    async fn register(&self, config: &ProjectConfig) -> Result<String> {
        self.execute(RegisterMonitorCmd {
            config: config.clone(),
        })
        .await
    }

    async fn unregister(&self, unique_key: &str) -> Result<bool> {
        self.execute(UnregisterMonitorCmd {
            unique_key: unique_key.to_string(),
        })
        .await
    }

    async fn list(&self) -> Result<Vec<ProjectConfig>> {
        self.execute(ListMonitorsCmd).await
    }

    async fn is_registered(&self, unique_key: &str) -> Result<bool> {
        self.execute(IsRegisteredCmd {
            unique_key: unique_key.to_string(),
        })
        .await
    }

    async fn switch_monitor(&self, unique_key: &str, paused: bool) -> Result<bool> {
        self.execute(RegistrySwitchMonitorCmd {
            unique_key: unique_key.to_string(),
            paused,
        })
        .await
    }

    async fn trigger_check(&self, unique_key: &str) -> Result<bool> {
        self.execute(RegistryTriggerCheckCmd {
            unique_key: unique_key.to_string(),
        })
        .await
    }
}

// =========================================================
// 内存 Mock 实现 (用于测试)
// =========================================================
#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct MockRegistry {
        pub monitors: RefCell<HashMap<String, ProjectConfig>>,
    }

    impl MockRegistry {
        pub fn new() -> Self {
            Self {
                monitors: RefCell::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Registry for MockRegistry {
        async fn register(&self, config: &ProjectConfig) -> Result<String> {
            let key = config.unique_key.clone();
            self.monitors
                .borrow_mut()
                .insert(key.clone(), config.clone());
            Ok(key)
        }

        async fn unregister(&self, unique_key: &str) -> Result<bool> {
            Ok(self.monitors.borrow_mut().remove(unique_key).is_some())
        }

        async fn list(&self) -> Result<Vec<ProjectConfig>> {
            Ok(self.monitors.borrow().values().cloned().collect())
        }

        async fn is_registered(&self, unique_key: &str) -> Result<bool> {
            Ok(self.monitors.borrow().contains_key(unique_key))
        }

        async fn switch_monitor(&self, unique_key: &str, paused: bool) -> Result<bool> {
            let mut monitors = self.monitors.borrow_mut();
            if let Some(config) = monitors.get_mut(unique_key) {
                if paused {
                    config.state = verwatch_shared::MonitorState::Paused;
                } else {
                    config.state = verwatch_shared::MonitorState::Running {
                        next_check_at: verwatch_shared::chrono::Utc::now(),
                    };
                }
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn trigger_check(&self, unique_key: &str) -> Result<bool> {
            Ok(self.monitors.borrow().contains_key(unique_key))
        }
    }
}
