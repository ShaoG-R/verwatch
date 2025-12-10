use crate::error::{AppError, Result};
use crate::utils::github::release::GitHubRelease;
// 引入同目录下的模块
use super::adapter::{AlarmScheduler, EnvAdapter, StorageAdapter, WorkerEnv, WorkerStorage};
use super::protocol::*;
// 引入外部依赖
use crate::utils::github::gateway::GitHubGateway;
use crate::utils::request::{HttpClient, WorkerHttpClient};
use crate::utils::rpc::{ApiRequest, RpcHandler};
use std::time::Duration;
use verwatch_shared::chrono::{Duration as ChronoDuration, Utc};
use verwatch_shared::{MonitorState, ProjectConfig};
use worker::*;

// =========================================================
// 条件编译日志宏
// =========================================================
#[cfg(target_arch = "wasm32")]
macro_rules! log_info {
    ($($t:tt)*) => (worker::console_log!($($t)*))
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_info {
    ($($t:tt)*) => (println!($($t)*))
}

#[cfg(target_arch = "wasm32")]
macro_rules! log_error {
    ($($t:tt)*) => (worker::console_error!($($t)*))
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_error {
    ($($t:tt)*) => (eprintln!($($t)*))
}

#[cfg(target_arch = "wasm32")]
macro_rules! log_warn {
    ($($t:tt)*) => (worker::console_warn!($($t)*))
}

// =========================================================
// 常量配置
// =========================================================
pub(crate) const STATE_KEY_CONFIG: &str = "config";
pub(crate) const STATE_KEY_VERSION: &str = "current_version";

// =========================================================
// 业务逻辑层 (Logic) - 可测试版本
// =========================================================

/// 可测试的 ProjectMonitor 业务逻辑
/// S: StorageAdapter + AlarmScheduler
/// E: EnvAdapter
/// C: HttpClient
pub struct ProjectMonitorLogicTestable<S, E, C> {
    storage: S,
    env: E,
    client: C,
}

impl<S, E, C> ProjectMonitorLogicTestable<S, E, C>
where
    S: StorageAdapter + AlarmScheduler,
    E: EnvAdapter,
    C: HttpClient,
{
    pub fn new(storage: S, env: E, client: C) -> Self {
        Self {
            storage,
            env,
            client,
        }
    }

    // --- RPC 处理函数 (不依赖外部调用) ---

    pub async fn setup(&self, cmd: SetupMonitorCmd) -> Result<()> {
        let mut config = cmd.config;
        let delay = config.request.initial_delay;

        // 计算下一次检查时间
        let next_check_at = Utc::now() + ChronoDuration::from_std(delay).unwrap_or_default();
        config.state = MonitorState::running(next_check_at);

        self.storage.put(STATE_KEY_CONFIG, &config).await?;
        self.storage.set_alarm(delay).await?;

        Ok(())
    }

    pub async fn stop(&self, _cmd: StopMonitorCmd) -> Result<()> {
        // 清理所有数据
        self.storage.delete(STATE_KEY_CONFIG).await?;
        self.storage.delete(STATE_KEY_VERSION).await?;
        // 取消闹钟
        self.storage.delete_alarm().await?;

        Ok(())
    }

    pub async fn get_config(&self, _cmd: GetConfigCmd) -> Result<Option<ProjectConfig>> {
        self.storage.get(STATE_KEY_CONFIG).await
    }

    pub async fn switch_monitor(&self, cmd: SwitchMonitorCmd) -> Result<()> {
        let mut config: ProjectConfig = match self.storage.get(STATE_KEY_CONFIG).await? {
            Some(c) => c,
            None => return Err(AppError::not_found("No config found")),
        };

        let is_currently_paused = config.state.is_paused();
        if is_currently_paused == cmd.paused {
            return Ok(());
        }

        if cmd.paused {
            // 暂停监控
            config.state = MonitorState::Paused;
            self.storage.put(STATE_KEY_CONFIG, &config).await?;
            self.storage.delete_alarm().await?;
        } else {
            // 恢复监控：立即开始
            let next_check_at = Utc::now();
            config.state = MonitorState::running(next_check_at);
            self.storage.put(STATE_KEY_CONFIG, &config).await?;
            self.storage.set_alarm(Duration::from_millis(0)).await?;
        }

        Ok(())
    }

    /// 手动触发检查
    pub async fn trigger(&self, _cmd: TriggerCheckCmd) -> Result<()> {
        let config: Option<ProjectConfig> = self.storage.get(STATE_KEY_CONFIG).await?;
        match config {
            Some(cfg) => self.perform_check_flow(&cfg).await,
            None => Err(AppError::not_found("No config found")),
        }
    }

    // --- Alarm 回调函数 ---

    pub async fn on_alarm(&self) -> Result<()> {
        let config: Option<ProjectConfig> = self.storage.get(STATE_KEY_CONFIG).await?;

        // 1. 僵尸检查
        let mut config = match config {
            Some(c) => c,
            None => {
                self.storage.delete_alarm().await?;
                return Ok(());
            }
        };

        // 2. 暂停检查
        if config.state.is_paused() {
            self.storage.delete_alarm().await?;
            return Ok(());
        }

        // 3. 执行核心逻辑 (捕获错误以决定下一次调度时间)
        let result = self.perform_check_flow(&config).await;

        // 记录日志
        match &result {
            Ok(_) => log_info!("Monitor Success [{}]", config.unique_key),
            Err(e) => log_error!("Monitor Failed [{}]: {}", config.unique_key, e),
        }

        // 4. 计算下一次时间
        let next_interval = if result.is_ok() {
            config.request.time_config.check_interval
        } else {
            config.request.time_config.retry_interval
        };

        // 5. 更新状态中的下一次检查时间
        let next_check_at =
            Utc::now() + ChronoDuration::from_std(next_interval).unwrap_or_default();
        config.state = MonitorState::running(next_check_at);
        self.storage.put(STATE_KEY_CONFIG, &config).await?;

        // 6. 设置下一次 Alarm
        self.storage.set_alarm(next_interval).await?;

        Ok(())
    }

    async fn perform_check_flow(&self, config: &ProjectConfig) -> Result<()> {
        // 获取 Secrets
        let github_token_name = self
            .env
            .var("GITHUB_TOKEN_NAME")
            .unwrap_or_else(|| "GITHUB_TOKEN".to_string());
        let global_token = self.env.secret(&github_token_name);

        // 1. 初始化 Gateway (注入 comparison_mode)
        // 这里传入了 config 中的模式，Gateway 后续会自动只解析该模式所需的字段
        let gateway =
            GitHubGateway::new(&self.client, global_token, config.request.comparison_mode);

        // A. 获取上游 Release (强类型，必定包含有效时间戳)
        let remote_release = gateway
            .fetch_latest_release(
                &config.request.base_config.upstream_owner,
                &config.request.base_config.upstream_repo,
            )
            .await
            .map_err(|e| AppError::store(format!("GitHub API: {}", e)))?;

        // B & C. 获取本地状态并进行比较
        // 存储的是 GitHubRelease 结构体(JSON)，而不仅仅是 String
        let local_state: Option<GitHubRelease> = self.storage.get(STATE_KEY_VERSION).await?;

        if let Some(local_release) = local_state {
            match remote_release.is_newer_than(&local_release) {
                // 远程版本确实更新 -> 继续执行
                Ok(true) => {
                    log_info!(
                        "New version found: {} (Old: {})",
                        remote_release.tag_name,
                        local_release.tag_name
                    );
                }
                // 远程版本不比本地新 -> 结束流程
                Ok(false) => return Ok(()),
                // 模式不匹配 (例如本地存的是 Updated 模式，但现在配置改成了 Published)
                // 策略：视为新版本，覆盖旧数据以修正状态
                Err(_) => {}
            }
        }

        // D. 触发 Dispatch
        let default_pat_name = self
            .env
            .var("PAT_TOKEN_NAME")
            .unwrap_or_else(|| "MY_GITHUB_PAT".to_string());

        let pat_key = config
            .request
            .dispatch_token_secret
            .as_deref()
            .unwrap_or(&default_pat_name);

        let pat = self
            .env
            .secret(pat_key)
            .ok_or_else(|| AppError::store(format!("Secret '{}' missing", pat_key)))?;

        gateway
            .trigger_dispatch(config, &remote_release.tag_name, &pat)
            .await
            .map_err(|e| AppError::store(format!("Dispatch: {}", e)))?;

        // E. 更新状态
        // 存储整个 remote_release 对象，以便下次比较时保留 mode 信息
        self.storage.put(STATE_KEY_VERSION, &remote_release).await?;

        Ok(())
    }
}

// =========================================================
// Worker 专用类型别名
// =========================================================

/// Worker 环境下的 ProjectMonitorLogic
pub type ProjectMonitorLogic<'a> =
    ProjectMonitorLogicTestable<WorkerStorage, WorkerEnv<'a>, WorkerHttpClient>;

// =========================================================
// Durable Object 绑定层 (Worker)
// =========================================================

#[durable_object]
pub struct ProjectMonitor {
    state: State,
    env: Env,
}

impl DurableObject for ProjectMonitor {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> worker::Result<Response> {
        let storage = WorkerStorage(self.state.storage());
        let env = WorkerEnv(&self.env);
        let logic = ProjectMonitorLogic::new(storage, env, WorkerHttpClient);
        let path = req.path();

        match path.as_str() {
            SetupMonitorCmd::PATH => RpcHandler::handle(req, |c| logic.setup(c)).await,
            StopMonitorCmd::PATH => RpcHandler::handle(req, |c| logic.stop(c)).await,
            TriggerCheckCmd::PATH => RpcHandler::handle(req, |c| logic.trigger(c)).await,
            GetConfigCmd::PATH => RpcHandler::handle(req, |c| logic.get_config(c)).await,
            SwitchMonitorCmd::PATH => RpcHandler::handle(req, |c| logic.switch_monitor(c)).await,
            _ => Response::error("Not Found", 404),
        }
    }

    async fn alarm(&self) -> worker::Result<Response> {
        let storage = WorkerStorage(self.state.storage());
        let env = WorkerEnv(&self.env);
        let logic = ProjectMonitorLogic::new(storage, env, WorkerHttpClient);

        // Alarm 内部即使出错，也只记录日志，不抛出异常给 Worker Runtime
        // 这样可以避免 Worker 无限重试当前的 Alarm
        if let Err(e) = logic.on_alarm().await {
            log_error!("System Alarm Critical Error: {}", e);
        }

        Response::ok("Ack")
    }
}

// =========================================================
// 测试模块
// =========================================================

#[cfg(test)]
mod tests;
