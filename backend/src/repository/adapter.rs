use crate::error::WatchResult;
use crate::project::protocol::{
    GetConfigCmd, SetupMonitorCmd, StopMonitorCmd, SwitchMonitorCmd, TriggerCheckCmd,
};
use crate::utils::rpc::{ApiRequest, RpcClient};
use async_trait::async_trait;
use verwatch_shared::ProjectConfig;
use worker::Env;

// =========================================================
// 抽象存储接口
// =========================================================

/// Registry 存储适配器：负责 Set<String> 的持久化
#[async_trait(?Send)]
pub trait RegistryStorageAdapter {
    /// 添加一个 key 到集合
    async fn add(&self, key: &str) -> WatchResult<()>;
    /// 从集合中移除一个 key
    async fn remove(&self, key: &str) -> WatchResult<bool>;
    /// 获取所有 key
    async fn list(&self) -> WatchResult<Vec<String>>;
    /// 检查 key 是否存在
    async fn contains(&self, key: &str) -> WatchResult<bool>;
}

// =========================================================
// 抽象环境变量接口
// =========================================================

pub trait EnvAdapter {
    fn var(&self, name: &str) -> Option<String>;
}

// =========================================================
// Monitor Client 接口
// =========================================================

#[async_trait(?Send)]
pub trait MonitorClient {
    async fn setup(&self, unique_key: &str, config: &ProjectConfig) -> WatchResult<()>;
    async fn stop(&self, unique_key: &str) -> WatchResult<()>;
    async fn get_config(&self, unique_key: &str) -> WatchResult<Option<ProjectConfig>>;
    async fn switch(&self, unique_key: &str, paused: bool) -> WatchResult<()>;
    async fn trigger_check(&self, unique_key: &str) -> WatchResult<()>;
}

// =========================================================
// 生产环境实现 (Worker)
// =========================================================

pub struct WorkerRegistryStorage(pub worker::Storage);

const REGISTRY_PREFIX: &str = "reg:";

#[async_trait(?Send)]
impl RegistryStorageAdapter for WorkerRegistryStorage {
    async fn add(&self, key: &str) -> WatchResult<()> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        self.0
            .put(&storage_key, "")
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("registry.add", key))
    }

    async fn remove(&self, key: &str) -> WatchResult<bool> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        self.0
            .delete(&storage_key)
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("registry.remove", key))
    }

    async fn list(&self) -> WatchResult<Vec<String>> {
        let opts = worker::ListOptions::new().prefix(REGISTRY_PREFIX);
        let map = self
            .0
            .list_with_options(opts)
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op("registry.list"))?;

        let mut keys = Vec::new();
        let iter = map.keys();

        loop {
            let next = iter
                .next()
                .map_err(|e| crate::error::WatchError::from(e).in_op("registry.list.iter"))?;
            if next.done() {
                break;
            }
            if let Some(key_str) = next.value().as_string() {
                // 移除前缀
                if let Some(stripped) = key_str.strip_prefix(REGISTRY_PREFIX) {
                    keys.push(stripped.to_string());
                }
            }
        }

        Ok(keys)
    }

    async fn contains(&self, key: &str) -> WatchResult<bool> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        let result: Option<String> = self.0.get(&storage_key).await.or_else(|e| {
            let msg = e.to_string();
            if msg.contains("No such value") {
                Ok(None)
            } else {
                Err(crate::error::WatchError::from(e).in_op_with("registry.contains", key))
            }
        })?;
        Ok(result.is_some())
    }
}

pub struct WorkerEnv<'a>(pub &'a Env);

impl<'a> EnvAdapter for WorkerEnv<'a> {
    fn var(&self, name: &str) -> Option<String> {
        self.0.var(name).ok().map(|v| v.to_string())
    }
}

pub struct WorkerMonitorClient<'a> {
    env: &'a Env,
    binding_name: String,
}

impl<'a> WorkerMonitorClient<'a> {
    pub fn new(env: &'a Env, binding_name: &str) -> Self {
        Self {
            env,
            binding_name: binding_name.to_string(),
        }
    }

    fn get_stub(&self, unique_key: &str) -> WatchResult<worker::Stub> {
        let namespace = self.env.durable_object(&self.binding_name).map_err(|e| {
            crate::error::WatchError::from(e).in_op_with("monitor.namespace", unique_key)
        })?;
        let id = namespace
            .id_from_name(unique_key)
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("monitor.id", unique_key))?;
        id.get_stub()
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("monitor.stub", unique_key))
    }

    async fn send<T: ApiRequest>(&self, unique_key: &str, cmd: &T) -> WatchResult<T::Response> {
        let stub = self.get_stub(unique_key)?;
        let client = RpcClient::new(stub, "http://monitor");
        client
            .send(cmd)
            .await
            .map_err(|e| e.in_op_with("monitor.send", unique_key))
    }
}

#[async_trait(?Send)]
impl<'a> MonitorClient for WorkerMonitorClient<'a> {
    async fn setup(&self, unique_key: &str, config: &ProjectConfig) -> WatchResult<()> {
        self.send(
            unique_key,
            &SetupMonitorCmd {
                config: config.clone(),
            },
        )
        .await
    }

    async fn stop(&self, unique_key: &str) -> WatchResult<()> {
        self.send(unique_key, &StopMonitorCmd).await
    }

    async fn get_config(&self, unique_key: &str) -> WatchResult<Option<ProjectConfig>> {
        self.send(unique_key, &GetConfigCmd).await
    }

    async fn switch(&self, unique_key: &str, paused: bool) -> WatchResult<()> {
        self.send(unique_key, &SwitchMonitorCmd { paused }).await
    }

    async fn trigger_check(&self, unique_key: &str) -> WatchResult<()> {
        self.send(unique_key, &TriggerCheckCmd).await
    }
}

// =========================================================
// 测试环境实现 (Mock)
// =========================================================

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;

    pub struct MockEnv {
        vars: HashMap<String, String>,
    }

    impl MockEnv {
        pub fn new() -> Self {
            Self {
                vars: HashMap::new(),
            }
        }
    }

    impl EnvAdapter for MockEnv {
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).cloned()
        }
    }
}
