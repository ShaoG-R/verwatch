use serde::{Deserialize, Serialize};
use serde_json::json;
use worker::{Env, Error, Result, ScheduleContext, ScheduledEvent, console_error, event};

pub mod api;
pub mod request;

use api::DispatchEvent;
use request::{HttpClient, HttpMethod, HttpRequest, WorkerHttpClient};

#[cfg(target_arch = "wasm32")]
macro_rules! log_info {
    ($($t:tt)*) => (worker::console_log!($($t)*))
}

#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_info {
    ($($t:tt)*) => (println!($($t)*))
}

// ----------------------------------------------------
// 数据结构更新: 解析更多时间字段
// ----------------------------------------------------
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct GitHubRelease {
    pub tag_name: String,
    // GitHub API 可能返回 null (例如 draft)，所以使用 Option
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
}

// ----------------------------------------------------
// 比较模式枚举
// ----------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComparisonMode {
    PublishedAt,
    UpdatedAt,
}

impl From<&str> for ComparisonMode {
    fn from(s: &str) -> Self {
        match s {
            "updated_at" => ComparisonMode::UpdatedAt,
            _ => ComparisonMode::PublishedAt, // 默认使用 published_at
        }
    }
}

// =========================================================
// Watchdog Host Traits & Logic
// =========================================================

#[async_trait::async_trait(?Send)]
pub trait WatchdogHost {
    fn get_secret(&self, key: &str) -> Result<String>;
    // 返回值类型改为 GitHubRelease 完整对象，不再只是 tag String
    async fn fetch_upstream_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease>;
    async fn trigger_dispatch(
        &self,
        owner: &str,
        repo: &str,
        version: &str,
        token: &str,
    ) -> Result<()>;
    async fn get_kv(&self, key: &str) -> Result<Option<String>>;
    async fn set_kv(&self, key: &str, value: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct WatchdogConfig {
    pub kv_key: String,
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,
    pub comparison_mode: ComparisonMode, // 新增：比较模式
}

pub async fn run_watchdog_logic<H: WatchdogHost>(host: &H, config: &WatchdogConfig) -> Result<()> {
    log_info!(
        "Checking upstream: {}/{}",
        config.upstream_owner,
        config.upstream_repo
    );

    // 1. 获取完整的 Release 信息
    let release = host
        .fetch_upstream_release(&config.upstream_owner, &config.upstream_repo)
        .await?;

    // 2. 根据配置决定使用哪个时间字段作为"当前版本指纹"
    let remote_time = match config.comparison_mode {
        ComparisonMode::PublishedAt => release.published_at.clone(),
        ComparisonMode::UpdatedAt => release.updated_at.clone(),
    };

    // 3. 确保时间字段存在 (GitHub 有时可能没有 published_at，如果是 draft)
    let remote_time = match remote_time {
        Some(t) => t,
        None => {
            log_info!("Skipping: Upstream release missing required timestamp field.");
            return Ok(());
        }
    };

    log_info!(
        "Upstream latest tag: {}, timestamp ({:?}): {}",
        release.tag_name,
        config.comparison_mode,
        remote_time
    );

    // 4. 从 KV 获取上次记录的时间戳
    let local_time = host.get_kv(&config.kv_key).await?;

    match local_time {
        Some(v) if v == remote_time => {
            log_info!("No change (Timestamp match): {}", v);
            Ok(())
        }
        _ => {
            log_info!(
                "Update needed! Local Time: {:?} -> Remote Time: {}",
                local_time,
                remote_time
            );

            // 获取 Token
            let token = host.get_secret("MY_GITHUB_PAT")?;

            // 5. 触发 Dispatch
            // 注意：虽然我们对比的是时间，但发送给 Action 的通常还是 tag_name
            host.trigger_dispatch(&config.my_owner, &config.my_repo, &release.tag_name, &token)
                .await?;

            // 6. 更新 KV 为最新的时间戳
            host.set_kv(&config.kv_key, &remote_time).await?;

            Ok(())
        }
    }
}

// =========================================================
// 生产环境 Host (使用 Worker API)
// =========================================================
struct CfHost<'a> {
    env: &'a Env,
    client: WorkerHttpClient,
}

impl<'a> CfHost<'a> {
    fn new(env: &'a Env) -> Self {
        Self {
            env,
            client: WorkerHttpClient,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<'a> WatchdogHost for CfHost<'a> {
    fn get_secret(&self, key: &str) -> Result<String> {
        self.env.secret(key).map(|s| s.to_string())
    }

    // 实现 fetch_upstream_release
    async fn fetch_upstream_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        );

        let mut req =
            HttpRequest::new(&url, HttpMethod::Get).with_header("User-Agent", "rust-cf-worker");

        if let Ok(token) = self.env.secret("GITHUB_TOKEN") {
            req = req.with_header("Authorization", &format!("Bearer {}", token.to_string()));
        }

        let resp = self.client.send(req).await?;

        if resp.status != 200 {
            return Err(Error::from(format!("Upstream API Error: {}", resp.status)));
        }

        let release: GitHubRelease = resp.json()?;
        Ok(release)
    }

    async fn trigger_dispatch(
        &self,
        owner: &str,
        repo: &str,
        version: &str,
        token: &str,
    ) -> Result<()> {
        let payload = json!({ "version": version });
        let event = DispatchEvent {
            owner,
            repo,
            token,
            event_type: "upstream_update",
            client_payload: payload,
        };
        event.send(&self.client).await
    }

    async fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let kv = self.env.kv("VERSION_STORE")?;
        Ok(kv.get(key).text().await?)
    }

    async fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        let kv = self.env.kv("VERSION_STORE")?;
        kv.put(key, value)?.execute().await?;
        Ok(())
    }
}

// =========================================================
// Worker 入口
// =========================================================
#[event(scheduled)]
pub async fn main(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    if let Err(e) = run_worker_logic(&env).await {
        console_error!("Execution failed: {}", e);
    }
}

async fn run_worker_logic(env: &Env) -> Result<()> {
    // 解析 comparison_mode，默认为 published_at
    let mode_str = env
        .var("COMPARISON_MODE")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "published_at".to_string());

    let config = WatchdogConfig {
        kv_key: env.var("KV_KEY")?.to_string(),
        upstream_owner: env.var("UPSTREAM_OWNER")?.to_string(),
        upstream_repo: env.var("UPSTREAM_REPO")?.to_string(),
        my_owner: env.var("MY_OWNER")?.to_string(),
        my_repo: env.var("MY_REPO")?.to_string(),
        comparison_mode: ComparisonMode::from(mode_str.as_str()),
    };
    let host = CfHost::new(env);
    run_watchdog_logic(&host, &config).await
}

// =========================================================
// 测试模块
// =========================================================
#[cfg(test)]
mod tests {
    use crate::request::ReqwestHttpClient;

    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::env;

    struct RealNetworkHost {
        client: ReqwestHttpClient,
        kv_store: RefCell<HashMap<String, String>>,
    }

    impl RealNetworkHost {
        fn new() -> Self {
            Self {
                client: ReqwestHttpClient::new(),
                kv_store: RefCell::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl WatchdogHost for RealNetworkHost {
        fn get_secret(&self, key: &str) -> Result<String> {
            env::var(key)
                .map_err(|_| Error::from(format!("Environment variable {} not found", key)))
        }

        async fn fetch_upstream_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease> {
            let url = format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                owner, repo
            );
            println!("Local Test: Fetching URL: {}", url);

            let mut req = HttpRequest::new(&url, HttpMethod::Get)
                .with_header("User-Agent", "rust-local-test");

            if let Ok(token) = env::var("GITHUB_TOKEN") {
                req = req.with_header("Authorization", &format!("Bearer {}", token));
            }

            let resp = self.client.send(req).await?;

            if resp.status != 200 {
                return Err(Error::from(format!("Upstream Error: {}", resp.status)));
            }

            let release: GitHubRelease = resp.json()?;
            Ok(release)
        }

        async fn trigger_dispatch(
            &self,
            owner: &str,
            repo: &str,
            version: &str,
            token: &str,
        ) -> Result<()> {
            println!(
                "Local Test: Triggering Dispatch to {}/{} with version {}",
                owner, repo, version
            );

            let payload = json!({ "version": version });
            let event = DispatchEvent {
                owner,
                repo,
                token,
                event_type: "upstream_update",
                client_payload: payload,
            };

            event.send(&self.client).await
        }

        async fn get_kv(&self, key: &str) -> Result<Option<String>> {
            Ok(self.kv_store.borrow().get(key).cloned())
        }

        async fn set_kv(&self, key: &str, value: &str) -> Result<()> {
            self.kv_store
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn integration_test_real_network() {
        if env::var("GITHUB_TOKEN").is_err() {
            println!("Skipping real network test because GITHUB_TOKEN is not set.");
            return;
        }

        let host = RealNetworkHost::new();

        let config = WatchdogConfig {
            kv_key: "test_timestamp".to_string(), // KV Key 建议改名以反映存储的是时间
            upstream_owner: "tokio-rs".to_string(),
            upstream_repo: "tokio".to_string(),
            my_owner: "your_user".to_string(),
            my_repo: "your_repo".to_string(),
            comparison_mode: ComparisonMode::PublishedAt, // 测试使用 published_at
        };

        println!("--- Starting Real Network Integration Test ---");

        let release = host
            .fetch_upstream_release(&config.upstream_owner, &config.upstream_repo)
            .await;

        match release {
            Ok(r) => {
                println!("Successfully fetched release.");
                println!("Tag: {}", r.tag_name);
                println!("Published at: {:?}", r.published_at);
                assert!(r.published_at.is_some());
            }
            Err(e) => panic!("Network request failed: {}", e),
        }

        println!("--- Integration Test Finished ---");
    }
}
