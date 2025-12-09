mod github_gateway;

use futures::{StreamExt, stream};
use verwatch_shared::ProjectConfig;
use worker::*;

use crate::utils::{repository::Repository, request::HttpClient};
use github_gateway::GitHubGateway;

// =========================================================
// 跨平台日志宏 (Copied for local usage)
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

// =========================================================
// 抽象接口：SecretResolver
// =========================================================

pub trait SecretResolver {
    fn get_secret(&self, name: &str) -> Option<String>;
}

// 实现：生产环境使用 Env 获取 Secret
pub struct EnvSecretResolver<'a>(pub &'a Env);

impl<'a> SecretResolver for EnvSecretResolver<'a> {
    fn get_secret(&self, name: &str) -> Option<String> {
        self.0.secret(name).ok().map(|s| s.to_string())
    }
}

// =========================================================
// 业务服务层
// =========================================================

pub struct WatchdogService<'a, C: HttpClient, R: Repository, S: SecretResolver> {
    repo: R,
    gateway: GitHubGateway<'a, C>,
    secret_resolver: &'a S,
    global_dispatch_token: String,
}

impl<'a, C: HttpClient, R: Repository, S: SecretResolver> WatchdogService<'a, C, R, S> {
    pub fn new(
        repo: R,
        client: &'a C,
        secret_resolver: &'a S,
        global_read_token: Option<String>,
        global_dispatch_token: String,
    ) -> Self {
        Self {
            repo,
            gateway: GitHubGateway::new(client, global_read_token),
            secret_resolver,
            global_dispatch_token,
        }
    }

    // 重构：check_project 现在接收预先获取的 state
    async fn check_project(
        &self,
        config: &ProjectConfig,
        current_state: Option<String>,
    ) -> Result<String> {
        let release = self
            .gateway
            .fetch_latest_release(&config.base.upstream_owner, &config.base.upstream_repo)
            .await?;

        let remote_time = match release.get_comparison_timestamp(config.base.comparison_mode) {
            Some(t) => t,
            None => {
                return Ok(format!(
                    "Skipped {}/{} (No timestamp)",
                    config.base.upstream_owner, config.base.upstream_repo
                ));
            }
        };

        // Optimization: 使用传入的 state，避免了 N+1 的 Fetch
        if let Some(local) = current_state {
            if local.as_str() == remote_time {
                return Ok(format!(
                    "No change for {}/{}",
                    config.base.upstream_owner, config.base.upstream_repo
                ));
            }
        }

        let token = if let Some(secret_name) = &config.base.dispatch_token_secret {
            match self.secret_resolver.get_secret(secret_name) {
                Some(t) => t,
                None => {
                    log_error!(
                        "Secret '{}' not found in Env/Vars, falling back to global token.",
                        secret_name
                    );
                    self.global_dispatch_token.clone()
                }
            }
        } else {
            self.global_dispatch_token.clone()
        };

        self.gateway
            .trigger_dispatch(config, &release.tag_name, &token)
            .await?;

        // 只有在更新发生时才调用 repo 写入，写入依然是单个请求，但这是低频操作
        self.repo
            .set_version_state(&config.version_store_key(), remote_time)
            .await?;

        Ok(format!(
            "Updated {}/{} to {}",
            config.base.upstream_owner, config.base.upstream_repo, release.tag_name
        ))
    }

    pub async fn run_all(&self) -> Result<String> {
        // Optimization: 使用 Bulk Get 获取配置+状态
        let items = self.repo.list_projects_with_states().await?;
        if items.is_empty() {
            return Ok("No projects configured.".to_string());
        }

        const CONCURRENCY_LIMIT: usize = 5;

        // items 是 (ProjectConfig, Option<String>) 的元组列表
        let results = stream::iter(items)
            .map(|(config, state)| async move {
                if config.paused {
                    return format!(
                        "Skipped {}/{} (Paused)",
                        config.base.upstream_owner, config.base.upstream_repo
                    );
                }
                match self.check_project(&config, state).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Error checking {}: {}", config.base.upstream_repo, e),
                }
            })
            .buffer_unordered(CONCURRENCY_LIMIT)
            .collect::<Vec<String>>()
            .await;

        let final_log = results.join("; ");
        log_info!("Batch run finished: {}", final_log);
        Ok(final_log)
    }
}

// =========================================================
// 单元测试
// =========================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::repository::tests::MockRepository;
    use crate::utils::request::MockHttpClient;
    use serde_json::json;
    use std::collections::HashMap;
    use verwatch_shared::{ComparisonMode, CreateProjectRequest};

    pub struct MockSecretResolver {
        pub secrets: HashMap<String, String>,
    }

    impl MockSecretResolver {
        pub fn new() -> Self {
            Self {
                secrets: HashMap::new(),
            }
        }
        pub fn with_secret(mut self, k: &str, v: &str) -> Self {
            self.secrets.insert(k.to_string(), v.to_string());
            self
        }
    }

    impl SecretResolver for MockSecretResolver {
        fn get_secret(&self, name: &str) -> Option<String> {
            self.secrets.get(name).cloned()
        }
    }

    #[tokio::test]
    async fn test_update_flow_with_custom_secret() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
        let resolver = MockSecretResolver::new().with_secret("MY_CUSTOM_TOKEN", "secret_value_123");

        let base_config = CreateProjectRequest {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token_secret: Some("MY_CUSTOM_TOKEN".into()),
            comparison_mode: ComparisonMode::PublishedAt,
        };
        // Use manual construction + save_project
        let config = ProjectConfig::new(base_config);
        repo.save_project(&config).await.unwrap();

        // Use set_version_state
        repo.set_version_state(&config.version_store_key(), "2023-01-01T00:00:00Z")
            .await
            .unwrap();

        client.mock_response(
            "https://api.github.com/repos/u/r/releases/latest",
            200,
            json!({ "tag_name": "v2", "published_at": "2023-02-01T00:00:00Z" }),
        );
        client.mock_response(
            "https://api.github.com/repos/m/mr/dispatches",
            204,
            json!({}),
        );

        let service = WatchdogService::new(repo, &client, &resolver, None, "global_token".into());

        let res = service.run_all().await.unwrap();
        assert!(res.contains("Updated u/r to v2"));

        // 验证请求数：只应该有两次 GitHub API 请求，不应有 N+1 的 DO 请求（MockRepo 无法验证 DO 请求数，但验证了逻辑通畅）
        // 实际上在真实环境中，run_all 只调用了一次 list_projects_with_states
    }
}
