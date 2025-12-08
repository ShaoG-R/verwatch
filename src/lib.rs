use serde::{Deserialize, Serialize};
use uuid::Uuid;
use worker::*;

pub mod api;
pub mod request;

use api::GitHubGateway;
use request::{HttpClient, WorkerHttpClient};

// =========================================================
// 跨平台日志宏
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
// 动态运行时配置 (Runtime Configuration)
// =========================================================

/// 这些是默认值，如果 wranger.toml 的 [vars] 中没有定义，则使用这些值
const DEFAULT_KV_BINDING: &str = "VERSION_STORE";
const DEFAULT_SECRET_VAR_NAME: &str = "ADMIN_SECRET";
const DEFAULT_GITHUB_TOKEN_VAR_NAME: &str = "GITHUB_TOKEN";
const DEFAULT_PAT_VAR_NAME: &str = "MY_GITHUB_PAT";

/// 运行时配置结构体
/// 负责从 Env 中读取 [vars]，实现配置解耦
struct RuntimeConfig {
    kv_binding: String,
    admin_secret_name: String,
    github_token_name: String,
    pat_token_name: String,
}

impl RuntimeConfig {
    fn new(env: &Env) -> Self {
        Self {
            // 尝试读取 [vars] KV_BINDING，读不到就用默认值 "VERSION_STORE"
            kv_binding: env
                .var("KV_BINDING")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_KV_BINDING.to_string()),

            // 尝试读取 [vars] ADMIN_SECRET_NAME (比如你想改成 "MY_APP_PASSWORD")
            admin_secret_name: env
                .var("ADMIN_SECRET_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_SECRET_VAR_NAME.to_string()),

            github_token_name: env
                .var("GITHUB_TOKEN_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_GITHUB_TOKEN_VAR_NAME.to_string()),

            pat_token_name: env
                .var("PAT_TOKEN_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_PAT_VAR_NAME.to_string()),
        }
    }
}

// =========================================================
// 领域模型 (Domain Models)
// =========================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    PublishedAt,
    UpdatedAt,
}

impl Default for ComparisonMode {
    fn default() -> Self {
        ComparisonMode::PublishedAt
    }
}

fn default_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    #[serde(default = "default_id")]
    pub id: String,
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,
    pub dispatch_token: Option<String>,
    #[serde(default)]
    pub comparison_mode: ComparisonMode,
}

impl ProjectConfig {
    pub fn version_store_key(&self) -> String {
        format!("v:{}/{}", self.upstream_owner, self.upstream_repo)
    }

    pub fn unique_key(&self) -> String {
        format!(
            "{}/{}->{}/{}",
            self.upstream_owner, self.upstream_repo, self.my_owner, self.my_repo
        )
    }

    pub fn get_dispatch_token<'a>(&'a self, global_token: &'a str) -> &'a str {
        self.dispatch_token.as_deref().unwrap_or(global_token)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
}

impl GitHubRelease {
    pub fn get_comparison_timestamp(&self, mode: ComparisonMode) -> Option<&String> {
        match mode {
            ComparisonMode::PublishedAt => self.published_at.as_ref(),
            ComparisonMode::UpdatedAt => self.updated_at.as_ref(),
        }
    }
}

// =========================================================
// 基础设施层 (Infrastructure)
// =========================================================

#[async_trait::async_trait(?Send)]
pub trait Repository {
    async fn get_all_configs(&self) -> Result<Vec<ProjectConfig>>;
    async fn add_config(&self, config: ProjectConfig) -> Result<ProjectConfig>;
    async fn delete_config(&self, id: &str) -> Result<bool>;
    async fn get_last_version_time(&self, config: &ProjectConfig) -> Result<Option<String>>;
    async fn update_last_version_time(&self, config: &ProjectConfig, time: &str) -> Result<()>;
}

struct KvProjectRepository {
    kv: KvStore,
}

impl KvProjectRepository {
    // 初始化时传入 Config
    fn new(env: &Env, config: &RuntimeConfig) -> Result<Self> {
        Ok(Self {
            kv: env.kv(&config.kv_binding)?, // 使用动态的 Binding Name
        })
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for KvProjectRepository {
    async fn get_all_configs(&self) -> Result<Vec<ProjectConfig>> {
        let list = self.kv.list().prefix("p:".to_string()).execute().await?;
        let keys: Vec<String> = list.keys.into_iter().map(|k| k.name).collect();

        let mut futures = Vec::new();
        for key in &keys {
            futures.push(self.kv.get(key).json::<ProjectConfig>());
        }

        let results = futures::future::join_all(futures).await;
        let mut configs = Vec::new();
        for res in results {
            if let Ok(Some(cfg)) = res {
                configs.push(cfg);
            }
        }
        Ok(configs)
    }

    async fn add_config(&self, mut config: ProjectConfig) -> Result<ProjectConfig> {
        // 1. Check Index for existing ID by Unique Key
        let unique_key = config.unique_key();
        let index_key = format!("idx:{}", unique_key);

        let existing_id = self.kv.get(&index_key).text().await?;

        // Use existing ID if available, otherwise use config's ID or generate new
        let final_id = if let Some(id) = existing_id {
            id
        } else {
            // Ensure ID is set
            config.id.clone()
        };

        // Update config with final ID (in case it changed or was generated separately in a way we want to enforce)
        config.id = final_id.clone();

        // 2. Save Config
        self.kv
            .put(&format!("p:{}", final_id), &config)?
            .execute()
            .await?;

        // 3. Save Index
        self.kv.put(&index_key, &final_id)?.execute().await?;

        Ok(config)
    }

    async fn delete_config(&self, id: &str) -> Result<bool> {
        let project_key = format!("p:{}", id);

        // Get config to find unique_key for index cleanup
        let config = self.kv.get(&project_key).json::<ProjectConfig>().await?;

        if let Some(c) = config {
            let index_key = format!("idx:{}", c.unique_key());
            self.kv.delete(&index_key).await?;
            self.kv.delete(&project_key).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_last_version_time(&self, config: &ProjectConfig) -> Result<Option<String>> {
        Ok(self.kv.get(&config.version_store_key()).text().await?)
    }

    async fn update_last_version_time(&self, config: &ProjectConfig, time: &str) -> Result<()> {
        self.kv
            .put(&config.version_store_key(), time)?
            .execute()
            .await?;
        Ok(())
    }
}

// =========================================================
// 业务服务层 (Service Layer)
// =========================================================

struct WatchdogService<'a, C: HttpClient, R: Repository> {
    repo: R,
    gateway: GitHubGateway<'a, C>,
    global_dispatch_token: String,
}

impl<'a, C: HttpClient, R: Repository> WatchdogService<'a, C, R> {
    fn new(
        repo: R,
        client: &'a C,
        global_read_token: Option<String>,
        global_dispatch_token: String,
    ) -> Self {
        Self {
            repo,
            gateway: GitHubGateway::new(client, global_read_token),
            global_dispatch_token,
        }
    }

    async fn check_project(&self, config: &ProjectConfig) -> Result<String> {
        let release = self
            .gateway
            .fetch_latest_release(&config.upstream_owner, &config.upstream_repo)
            .await?;

        let remote_time = match release.get_comparison_timestamp(config.comparison_mode) {
            Some(t) => t,
            None => {
                return Ok(format!(
                    "Skipped {}/{} (No timestamp)",
                    config.upstream_owner, config.upstream_repo
                ));
            }
        };

        let local_time = self.repo.get_last_version_time(config).await?;
        if let Some(local) = local_time {
            if &local == remote_time {
                return Ok(format!(
                    "No change for {}/{}",
                    config.upstream_owner, config.upstream_repo
                ));
            }
        }

        let token = config.get_dispatch_token(&self.global_dispatch_token);
        self.gateway
            .trigger_dispatch(config, &release.tag_name, token)
            .await?;
        self.repo
            .update_last_version_time(config, remote_time)
            .await?;

        Ok(format!(
            "Updated {}/{} to {}",
            config.upstream_owner, config.upstream_repo, release.tag_name
        ))
    }

    async fn run_all(&self) -> Result<String> {
        let configs = self.repo.get_all_configs().await?;
        if configs.is_empty() {
            return Ok("No projects configured.".to_string());
        }

        let mut results = Vec::new();
        for config in configs {
            let res = match self.check_project(&config).await {
                Ok(msg) => msg,
                Err(e) => format!("Error checking {}: {}", config.upstream_repo, e),
            };
            log_info!("{}", res);
            results.push(res);
        }
        Ok(results.join("; "))
    }
}

// =========================================================
// 控制器层 (Controllers & Entry Points)
// =========================================================

fn ensure_admin_auth(req: &Request, env: &Env, config: &RuntimeConfig) -> Result<()> {
    let auth_header = req.headers().get("X-Auth-Key")?.unwrap_or_default();

    // 动态读取 Secret：先获取 Secret 的变量名，再取值
    let secret = env
        .secret(&config.admin_secret_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if secret.is_empty() || auth_header != secret {
        return Err(Error::from("Unauthorized"));
    }
    Ok(())
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();

    router
        .get_async("/api/projects", |_, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;
            let configs = repo.get_all_configs().await?;
            Response::from_json(&configs)
        })
        .post_async("/api/projects", |mut req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            let new_config: ProjectConfig = req.json().await?;

            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;
            let saved_config = repo.add_config(new_config).await?;

            Response::from_json(&saved_config)
        })
        .delete_async("/api/projects", |mut req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            #[derive(Deserialize)]
            struct DeleteTarget {
                id: String,
            }
            let target: DeleteTarget = req.json().await?;

            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;

            if repo.delete_config(&target.id).await? {
                Response::ok("Project deleted")
            } else {
                Response::error("Not found", 404)
            }
        })
        .run(req, env)
        .await
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();

    // 初始化配置
    let config = RuntimeConfig::new(&env);
    let client = WorkerHttpClient;

    let repo = match KvProjectRepository::new(&env, &config) {
        Ok(r) => r,
        Err(e) => {
            log_error!("Repo Init Error: {}", e);
            return;
        }
    };

    // 使用动态配置的名称去读取 Secret
    let global_read = env
        .secret(&config.github_token_name)
        .ok()
        .map(|s| s.to_string());
    let global_dispatch = env
        .secret(&config.pat_token_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    let service = WatchdogService::new(repo, &client, global_read, global_dispatch);

    match service.run_all().await {
        Ok(msg) => log_info!("Cron Success: {}", msg),
        Err(e) => log_error!("Cron Error: {}", e),
    }
}

// =========================================================
// 单元测试 (Unit Tests)
// =========================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::MockHttpClient;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::HashMap;

    // Mock Repository
    struct MockRepository {
        data: RefCell<HashMap<String, String>>,
        configs: RefCell<Vec<ProjectConfig>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                data: RefCell::new(HashMap::new()),
                configs: RefCell::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Repository for MockRepository {
        async fn get_all_configs(&self) -> Result<Vec<ProjectConfig>> {
            Ok(self.configs.borrow().clone())
        }
        async fn add_config(&self, config: ProjectConfig) -> Result<ProjectConfig> {
            self.configs
                .borrow_mut()
                .retain(|c| c.unique_key() != config.unique_key());
            self.configs.borrow_mut().push(config.clone());
            Ok(config)
        }
        async fn delete_config(&self, id: &str) -> Result<bool> {
            let len_before = self.configs.borrow().len();
            self.configs.borrow_mut().retain(|c| c.id != id);
            Ok(self.configs.borrow().len() < len_before)
        }
        async fn get_last_version_time(&self, config: &ProjectConfig) -> Result<Option<String>> {
            Ok(self.data.borrow().get(&config.version_store_key()).cloned())
        }
        async fn update_last_version_time(&self, config: &ProjectConfig, time: &str) -> Result<()> {
            self.data
                .borrow_mut()
                .insert(config.version_store_key(), time.to_string());
            Ok(())
        }
    }

    // 单元测试不需要 RuntimeConfig，因为我们直接 mock 了 Repository
    #[tokio::test]
    async fn test_update_flow() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
        let config = ProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token: None,
            comparison_mode: ComparisonMode::PublishedAt,
            id: "test-id".to_string(),
        };

        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
            .await
            .unwrap();
        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
            .await
            .unwrap();
        repo.add_config(config.clone()).await.unwrap();

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

        let service = WatchdogService::new(repo, &client, None, "token".into());
        let res = service.run_all().await.unwrap();

        assert!(res.contains("Updated u/r to v2"));
    }

    #[tokio::test]
    async fn test_no_update() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
        let config = ProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token: None,
            comparison_mode: ComparisonMode::PublishedAt,
            id: "test-id".to_string(),
        };

        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
            .await
            .unwrap();
        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
            .await
            .unwrap();
        repo.add_config(config.clone()).await.unwrap();

        client.mock_response(
            "https://api.github.com/repos/u/r/releases/latest",
            200,
            json!({ "tag_name": "v1", "published_at": "2023-01-01T00:00:00Z" }),
        );

        let service = WatchdogService::new(repo, &client, None, "token".into());
        let res = service.run_all().await.unwrap();

        assert!(res.contains("No change"));
        let reqs = client.requests.borrow();
        assert!(!reqs.iter().any(|r| r.0.contains("/dispatches")));
    }
}
