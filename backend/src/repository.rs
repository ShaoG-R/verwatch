pub mod adapter;
pub mod protocol;
mod registry;

use crate::error::WatchResult;
use crate::utils::rpc::{ApiRequest, RpcClient};
use protocol::*;
use verwatch_shared::ProjectConfig;
use worker::Env;

// =========================================================
// Registry Trait (面向 ProjectRegistry DO)
// =========================================================

#[async_trait::async_trait(?Send)]
pub trait Registry {
    /// 注册一个 Monitor (内部调用 ProjectMonitor.setup)
    async fn register(&self, config: &ProjectConfig) -> WatchResult<String>;
    /// 注销一个 Monitor (内部调用 ProjectMonitor.stop)
    async fn unregister(&self, unique_key: &str) -> WatchResult<bool>;
    /// 列出所有已注册的 Monitor 的 Config
    async fn list(&self) -> WatchResult<Vec<ProjectConfig>>;
    /// 检查是否已注册
    async fn is_registered(&self, unique_key: &str) -> WatchResult<bool>;
    /// 切换 Monitor 监控状态
    async fn switch_monitor(&self, unique_key: &str, paused: bool) -> WatchResult<bool>;
    /// 手动触发 Monitor 检查
    async fn trigger_check(&self, unique_key: &str) -> WatchResult<bool>;
}

// =========================================================
// Durable Object 实现
// =========================================================

pub struct DoProjectRegistry {
    client: RpcClient,
}

impl DoProjectRegistry {
    pub fn new(env: &Env, binding_name: &str) -> WatchResult<Self> {
        let namespace = env.durable_object(binding_name).map_err(|e| {
            crate::error::WatchError::from(e).in_op_with("registry.namespace", binding_name)
        })?;
        // Registry 是单例，使用固定 ID
        let id = namespace
            .id_from_name("default")
            .map_err(|e| crate::error::WatchError::from(e).in_op("registry.id"))?;
        let stub = id
            .get_stub()
            .map_err(|e| crate::error::WatchError::from(e).in_op("registry.stub"))?;
        // Registry DO base URL
        let client = RpcClient::new(stub, "http://registry");
        Ok(Self { client })
    }

    /// 核心泛型方法：执行 RPC 请求
    async fn execute<T: ApiRequest>(&self, req: T) -> WatchResult<T::Response> {
        self.client.send(&req).await
    }
}

#[async_trait::async_trait(?Send)]
impl Registry for DoProjectRegistry {
    async fn register(&self, config: &ProjectConfig) -> WatchResult<String> {
        self.execute(RegisterMonitorCmd {
            config: config.clone(),
        })
        .await
    }

    async fn unregister(&self, unique_key: &str) -> WatchResult<bool> {
        self.execute(UnregisterMonitorCmd {
            unique_key: unique_key.to_string(),
        })
        .await
    }

    async fn list(&self) -> WatchResult<Vec<ProjectConfig>> {
        self.execute(ListMonitorsCmd).await
    }

    async fn is_registered(&self, unique_key: &str) -> WatchResult<bool> {
        self.execute(IsRegisteredCmd {
            unique_key: unique_key.to_string(),
        })
        .await
    }

    async fn switch_monitor(&self, unique_key: &str, paused: bool) -> WatchResult<bool> {
        self.execute(RegistrySwitchMonitorCmd {
            unique_key: unique_key.to_string(),
            paused,
        })
        .await
    }

    async fn trigger_check(&self, unique_key: &str) -> WatchResult<bool> {
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
        async fn register(&self, config: &ProjectConfig) -> WatchResult<String> {
            let key = config.unique_key.clone();
            self.monitors
                .borrow_mut()
                .insert(key.clone(), config.clone());
            Ok(key)
        }

        async fn unregister(&self, unique_key: &str) -> WatchResult<bool> {
            Ok(self.monitors.borrow_mut().remove(unique_key).is_some())
        }

        async fn list(&self) -> WatchResult<Vec<ProjectConfig>> {
            Ok(self.monitors.borrow().values().cloned().collect())
        }

        async fn is_registered(&self, unique_key: &str) -> WatchResult<bool> {
            Ok(self.monitors.borrow().contains_key(unique_key))
        }

        async fn switch_monitor(&self, unique_key: &str, paused: bool) -> WatchResult<bool> {
            let mut monitors = self.monitors.borrow_mut();
            if let Some(config) = monitors.get_mut(unique_key) {
                if paused {
                    config.state = verwatch_shared::MonitorState::Paused;
                } else {
                    config.state = verwatch_shared::MonitorState::Running {
                        next_check_at: verwatch_shared::Date::now_timestamp(),
                    };
                }
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn trigger_check(&self, unique_key: &str) -> WatchResult<bool> {
            Ok(self.monitors.borrow().contains_key(unique_key))
        }
    }
}
