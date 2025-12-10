use super::adapter::{
    EnvAdapter, MonitorClient, RegistryStorageAdapter, WorkerEnv, WorkerMonitorClient,
    WorkerRegistryStorage,
};
use super::protocol::*;
use crate::error::Result;
use crate::utils::rpc::{ApiRequest, RpcHandler};
use verwatch_shared::ProjectConfig;
use worker::*;

// =========================================================
// 业务逻辑层 (Logic)
// =========================================================

pub struct ProjectRegistryLogic<S, E, M> {
    storage: S,
    _env: E,
    monitor_client: M,
}

impl<S, E, M> ProjectRegistryLogic<S, E, M>
where
    S: RegistryStorageAdapter,
    E: EnvAdapter,
    M: MonitorClient,
{
    pub fn new(storage: S, env: E, monitor_client: M) -> Self {
        Self {
            storage,
            _env: env,
            monitor_client,
        }
    }

    /// 注册一个 Monitor
    /// 1. 计算 unique_key
    /// 2. 调用 Monitor setup
    /// 3. 记录到 Registry
    pub async fn register(&self, cmd: RegisterMonitorCmd) -> Result<String> {
        let config = cmd.config;
        let unique_key = config.unique_key.clone();

        // 调用 ProjectMonitor 的 setup
        self.monitor_client.setup(&unique_key, &config).await?;

        // 记录到 Registry
        self.storage.add(&unique_key).await?;

        Ok(unique_key)
    }

    /// 注销一个 Monitor
    /// 1. 调用 Monitor stop
    /// 2. 从 Registry 移除
    pub async fn unregister(&self, cmd: UnregisterMonitorCmd) -> Result<bool> {
        let unique_key = &cmd.unique_key;

        // 先检查是否存在
        if !self.storage.contains(unique_key).await? {
            return Ok(false);
        }

        // 调用 ProjectMonitor 的 stop
        self.monitor_client.stop(unique_key).await?;

        // 从 Registry 移除
        self.storage.remove(unique_key).await
    }

    /// 列出所有已注册的 Monitor 的 ProjectConfig
    /// 遍历查询每个 Monitor
    pub async fn list(&self, _cmd: ListMonitorsCmd) -> Result<Vec<ProjectConfig>> {
        let keys = self.storage.list().await?;

        // 并发获取所有 Config
        let tasks = keys
            .iter()
            .map(|key| async { self.monitor_client.get_config(key).await });

        let results = futures::future::join_all(tasks).await;

        // 收集成功的 Config，忽略失败的（可能是脏数据）
        let configs: Vec<ProjectConfig> = results
            .into_iter()
            .filter_map(|r| r.ok().flatten())
            .collect();

        Ok(configs)
    }

    pub async fn is_registered(&self, cmd: IsRegisteredCmd) -> Result<bool> {
        self.storage.contains(&cmd.unique_key).await
    }

    /// 切换监控状态
    pub async fn switch_monitor(&self, cmd: RegistrySwitchMonitorCmd) -> Result<bool> {
        if !self.storage.contains(&cmd.unique_key).await? {
            return Ok(false);
        }
        self.monitor_client
            .switch(&cmd.unique_key, cmd.paused)
            .await?;
        Ok(true)
    }

    /// 手动触发检查
    pub async fn trigger_check(&self, cmd: RegistryTriggerCheckCmd) -> Result<bool> {
        if !self.storage.contains(&cmd.unique_key).await? {
            return Ok(false);
        }
        self.monitor_client.trigger_check(&cmd.unique_key).await?;
        Ok(true)
    }
}

// =========================================================
// Durable Object 绑定层 (Worker)
// =========================================================

#[durable_object]
pub struct ProjectRegistry {
    state: State,
    env: Env,
}

impl DurableObject for ProjectRegistry {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> worker::Result<Response> {
        let storage = WorkerRegistryStorage(self.state.storage());
        let env_adapter = WorkerEnv(&self.env);

        // 从环境变量获取 MONITOR_BINDING，默认为 "PROJECT_MONITOR"
        let binding_name = env_adapter
            .var("MONITOR_BINDING")
            .unwrap_or_else(|| "PROJECT_MONITOR".to_string());

        let monitor_client = WorkerMonitorClient::new(&self.env, &binding_name);
        let logic = ProjectRegistryLogic::new(storage, env_adapter, monitor_client);
        let path = req.path();

        match path.as_str() {
            RegisterMonitorCmd::PATH => RpcHandler::handle(req, |c| logic.register(c)).await,
            UnregisterMonitorCmd::PATH => RpcHandler::handle(req, |c| logic.unregister(c)).await,
            ListMonitorsCmd::PATH => RpcHandler::handle(req, |c| logic.list(c)).await,
            IsRegisteredCmd::PATH => RpcHandler::handle(req, |c| logic.is_registered(c)).await,
            RegistrySwitchMonitorCmd::PATH => {
                RpcHandler::handle(req, |c| logic.switch_monitor(c)).await
            }
            RegistryTriggerCheckCmd::PATH => {
                RpcHandler::handle(req, |c| logic.trigger_check(c)).await
            }
            _ => Response::error("Not Found", 404),
        }
    }
}

#[cfg(test)]
mod tests;
