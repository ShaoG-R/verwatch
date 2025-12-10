use crate::error::{AppError, Result};
use crate::project::protocol::{
    ApiRequest, GetConfigCmd, SetupMonitorCmd, StopMonitorCmd, SwitchMonitorCmd, TriggerCheckCmd,
};
use async_trait::async_trait;
use verwatch_shared::ProjectConfig;
use worker::wasm_bindgen::JsValue;
use worker::{Env, Headers, Request, RequestInit, Response};

// =========================================================
// 抽象存储接口
// =========================================================

/// Registry 存储适配器：负责 Set<String> 的持久化
#[async_trait(?Send)]
pub trait RegistryStorageAdapter {
    /// 添加一个 key 到集合
    async fn add(&self, key: &str) -> Result<()>;
    /// 从集合中移除一个 key
    async fn remove(&self, key: &str) -> Result<bool>;
    /// 获取所有 key
    async fn list(&self) -> Result<Vec<String>>;
    /// 检查 key 是否存在
    async fn contains(&self, key: &str) -> Result<bool>;
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
    async fn setup(&self, unique_key: &str, config: &ProjectConfig) -> Result<()>;
    async fn stop(&self, unique_key: &str) -> Result<()>;
    async fn get_config(&self, unique_key: &str) -> Result<Option<ProjectConfig>>;
    async fn switch(&self, unique_key: &str, paused: bool) -> Result<()>;
    async fn trigger_check(&self, unique_key: &str) -> Result<()>;
}

// =========================================================
// 生产环境实现 (Worker)
// =========================================================

pub struct WorkerRegistryStorage(pub worker::Storage);

const REGISTRY_PREFIX: &str = "reg:";

#[async_trait(?Send)]
impl RegistryStorageAdapter for WorkerRegistryStorage {
    async fn add(&self, key: &str) -> Result<()> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        self.0.put(&storage_key, "").await.map_err(|e| e.into())
    }

    async fn remove(&self, key: &str) -> Result<bool> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        self.0.delete(&storage_key).await.map_err(|e| e.into())
    }

    async fn list(&self) -> Result<Vec<String>> {
        let opts = worker::ListOptions::new().prefix(REGISTRY_PREFIX);
        let map = self.0.list_with_options(opts).await?;

        let mut keys = Vec::new();
        let iter = map.keys();

        loop {
            let next = iter.next()?;
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

    async fn contains(&self, key: &str) -> Result<bool> {
        let storage_key = format!("{}{}", REGISTRY_PREFIX, key);
        let result: Option<String> = self.0.get(&storage_key).await.or_else(|e| {
            let msg = e.to_string();
            if msg.contains("No such value") {
                Ok(None)
            } else {
                Err(e)
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

    fn get_stub(&self, unique_key: &str) -> Result<worker::Stub> {
        let namespace = self.env.durable_object(&self.binding_name)?;
        let id = namespace.id_from_name(unique_key)?;
        Ok(id.get_stub()?)
    }

    async fn send_internal<T: ApiRequest>(&self, unique_key: &str, cmd: &T) -> Result<Response> {
        let stub = self.get_stub(unique_key)?;

        let body = serde_json::to_string(cmd)?;
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(T::METHOD).with_headers(headers);

        if T::METHOD != worker::Method::Get && T::METHOD != worker::Method::Head {
            init.with_body(Some(JsValue::from_str(&body)));
        }

        let url = format!("http://monitor{}", T::PATH);
        let request = Request::new_with_init(&url, &init)?;

        let response = stub.fetch_with_request(request).await?;

        if response.status_code() != 200 {
            return Err(AppError::Store(format!(
                "Monitor Error: {}",
                response.status_code()
            )));
        }

        Ok(response)
    }

    async fn send_request_no_response<T: ApiRequest>(
        &self,
        unique_key: &str,
        cmd: &T,
    ) -> Result<()> {
        self.send_internal(unique_key, cmd).await?;
        Ok(())
    }

    async fn send_request_with_response<T: ApiRequest>(
        &self,
        unique_key: &str,
        cmd: &T,
    ) -> Result<T::Response> {
        let mut response = self.send_internal(unique_key, cmd).await?;
        let data = response.json::<T::Response>().await?;
        Ok(data)
    }
}

#[async_trait(?Send)]
impl<'a> MonitorClient for WorkerMonitorClient<'a> {
    async fn setup(&self, unique_key: &str, config: &ProjectConfig) -> Result<()> {
        self.send_request_no_response(
            unique_key,
            &SetupMonitorCmd {
                config: config.clone(),
            },
        )
        .await
    }

    async fn stop(&self, unique_key: &str) -> Result<()> {
        self.send_request_no_response(unique_key, &StopMonitorCmd)
            .await
    }

    async fn get_config(&self, unique_key: &str) -> Result<Option<ProjectConfig>> {
        self.send_request_with_response(unique_key, &GetConfigCmd)
            .await
    }

    async fn switch(&self, unique_key: &str, paused: bool) -> Result<()> {
        self.send_request_no_response(unique_key, &SwitchMonitorCmd { paused })
            .await
    }

    async fn trigger_check(&self, unique_key: &str) -> Result<()> {
        self.send_request_no_response(unique_key, &TriggerCheckCmd)
            .await
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
