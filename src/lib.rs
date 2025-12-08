use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use worker::*;

pub mod api;
pub mod request;

use api::GitHubGateway;
use request::{HttpClient, WorkerHttpClient};

// =========================================================
// 常量定义 (Constants)
// =========================================================

const PREFIX_PROJECT: &str = "p:";
const PREFIX_VERSION: &str = "v:";
const HEADER_AUTH_KEY: &str = "X-Auth-Key";
const DEFAULT_KV_BINDING: &str = "VERSION_STORE";
const DEFAULT_SECRET_VAR_NAME: &str = "ADMIN_SECRET";
const DEFAULT_GITHUB_TOKEN_VAR_NAME: &str = "GITHUB_TOKEN";
const DEFAULT_PAT_VAR_NAME: &str = "MY_GITHUB_PAT";

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
// 抽象接口：SecretResolver (用于解耦 Env 和 Service)
// =========================================================

pub trait SecretResolver {
    fn get_secret(&self, name: &str) -> Option<String>;
}

// 实现：生产环境使用 Env 获取 Secret
struct EnvSecretResolver<'a>(&'a Env);

impl<'a> SecretResolver for EnvSecretResolver<'a> {
    fn get_secret(&self, name: &str) -> Option<String> {
        self.0.secret(name).ok().map(|s| s.to_string())
    }
}

// =========================================================
// 运行时配置
// =========================================================

struct RuntimeConfig {
    kv_binding: String,
    admin_secret_name: String,
    github_token_name: String,
    pat_token_name: String,
}

impl RuntimeConfig {
    fn new(env: &Env) -> Self {
        Self {
            kv_binding: env
                .var("KV_BINDING")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_KV_BINDING.to_string()),
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BaseProjectConfig {
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,

    // Breaking Change: 存储 Secret 变量名，而不是 Token 本身
    // 对应 wrangler.toml 中的 [secrets] 或 [vars]
    pub dispatch_token_secret: Option<String>,

    pub comparison_mode: ComparisonMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub unique_key: String,
    #[serde(flatten)]
    pub base: BaseProjectConfig,
}

pub type CreateProjectRequest = BaseProjectConfig;

impl ProjectConfig {
    pub fn new(base: BaseProjectConfig) -> Self {
        let mut config = ProjectConfig {
            unique_key: String::new(),
            base,
        };
        config.unique_key = config.generate_unique_key();
        config
    }

    pub fn version_store_key(&self) -> String {
        format!(
            "{}{}/{}",
            PREFIX_VERSION, self.base.upstream_owner, self.base.upstream_repo
        )
    }

    pub fn generate_unique_key(&self) -> String {
        format!(
            "{}/{}->{}/{}",
            self.base.upstream_owner,
            self.base.upstream_repo,
            self.base.my_owner,
            self.base.my_repo
        )
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
    async fn add_config(&self, base: BaseProjectConfig) -> Result<ProjectConfig>;

    // 标准删除：只负责删除，不返回内容
    async fn delete_config(&self, id: &str) -> Result<()>;

    // 弹出删除：删除并返回旧内容
    async fn pop_config(&self, id: &str) -> Result<Option<ProjectConfig>>;

    async fn get_last_version_time(&self, config: &ProjectConfig) -> Result<Option<String>>;
    async fn update_last_version_time(&self, config: &ProjectConfig, time: &str) -> Result<()>;
}

struct KvProjectRepository {
    kv: KvStore,
}

impl KvProjectRepository {
    fn new(env: &Env, config: &RuntimeConfig) -> Result<Self> {
        Ok(Self {
            kv: env.kv(&config.kv_binding)?,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for KvProjectRepository {
    async fn get_all_configs(&self) -> Result<Vec<ProjectConfig>> {
        let list = self
            .kv
            .list()
            .prefix(PREFIX_PROJECT.to_string())
            .execute()
            .await?;

        let mut configs = Vec::new();
        let mut keys_without_meta = Vec::new();

        for key in list.keys {
            if let Some(meta) = key.metadata {
                if let Ok(cfg) = serde_json::from_value::<ProjectConfig>(meta) {
                    configs.push(cfg);
                } else {
                    keys_without_meta.push(key.name);
                }
            } else {
                keys_without_meta.push(key.name);
            }
        }

        if !keys_without_meta.is_empty() {
            let futures = keys_without_meta
                .iter()
                .map(|k| self.kv.get(k).json::<ProjectConfig>());
            let results = futures::future::join_all(futures).await;
            for res in results {
                if let Ok(Some(cfg)) = res {
                    configs.push(cfg);
                }
            }
        }

        Ok(configs)
    }

    async fn add_config(&self, base: BaseProjectConfig) -> Result<ProjectConfig> {
        let config = ProjectConfig::new(base);
        let key = format!("{}{}", PREFIX_PROJECT, config.unique_key);

        let serialized_json = serde_json::to_string(&config)?;
        let json_len = serialized_json.len();

        let mut query = self.kv.put(&key, &config)?;

        if json_len < 1024 {
            query = query.metadata(&config)?;
        } else {
            log_info!(
                "Config size ({} bytes) exceeds metadata limit (1024), skipping optimization.",
                json_len
            );
        }

        query.execute().await?;
        Ok(config)
    }

    async fn delete_config(&self, id: &str) -> Result<()> {
        let project_key = format!("{}{}", PREFIX_PROJECT, id);
        // 直接删除，不检查是否存在，节省一次 KV Read
        self.kv.delete(&project_key).await?;
        Ok(())
    }

    async fn pop_config(&self, id: &str) -> Result<Option<ProjectConfig>> {
        let project_key = format!("{}{}", PREFIX_PROJECT, id);

        // 查后删
        let val = self.kv.get(&project_key).json::<ProjectConfig>().await?;

        if let Some(config) = val {
            self.kv.delete(&project_key).await?;
            Ok(Some(config))
        } else {
            Ok(None)
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

struct WatchdogService<'a, C: HttpClient, R: Repository, S: SecretResolver> {
    repo: R,
    gateway: GitHubGateway<'a, C>,
    secret_resolver: &'a S,
    global_dispatch_token: String,
}

impl<'a, C: HttpClient, R: Repository, S: SecretResolver> WatchdogService<'a, C, R, S> {
    fn new(
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

    async fn check_project(&self, config: &ProjectConfig) -> Result<String> {
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

        let local_time = self.repo.get_last_version_time(config).await?;
        if let Some(local) = local_time {
            if &local == remote_time {
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
        self.repo
            .update_last_version_time(config, remote_time)
            .await?;

        Ok(format!(
            "Updated {}/{} to {}",
            config.base.upstream_owner, config.base.upstream_repo, release.tag_name
        ))
    }

    async fn run_all(&self) -> Result<String> {
        let configs = self.repo.get_all_configs().await?;
        if configs.is_empty() {
            return Ok("No projects configured.".to_string());
        }

        const CONCURRENCY_LIMIT: usize = 5;

        let results = stream::iter(configs)
            .map(|config| async move {
                match self.check_project(&config).await {
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
// 控制器层 (Controllers & Entry Points)
// =========================================================

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    a_bytes
        .iter()
        .zip(b_bytes)
        .fold(0, |acc, (&x, &y)| acc | (x ^ y))
        == 0
}

fn ensure_admin_auth(req: &Request, env: &Env, config: &RuntimeConfig) -> Result<()> {
    let auth_header = req.headers().get(HEADER_AUTH_KEY)?.unwrap_or_default();

    let secret = env
        .secret(&config.admin_secret_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if secret.is_empty() || !constant_time_eq(&auth_header, &secret) {
        return Err(Error::from("Unauthorized"));
    }
    Ok(())
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();

    #[derive(Deserialize)]
    struct DeleteTarget {
        id: String,
    }

    router
        .get_async("/api/projects", |req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;
            let configs = repo.get_all_configs().await?;
            Response::from_json(&configs)
        })
        .post_async("/api/projects", |mut req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            let req_data: BaseProjectConfig = req.json().await?;
            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;
            let saved_config = repo.add_config(req_data).await?;

            Response::from_json(&saved_config)
        })
        // 接口 1: 标准删除，返回 204 No Content
        .delete_async("/api/projects", |mut req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            let target: DeleteTarget = req.json().await?;
            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;

            repo.delete_config(&target.id).await?;
            Response::empty() // 204
        })
        // 接口 2: 弹出删除 (原逻辑改名)，返回 200 + Config Body
        .delete_async("/api/projects/pop", |mut req, ctx| async move {
            let cfg = RuntimeConfig::new(&ctx.env);
            ensure_admin_auth(&req, &ctx.env, &cfg)?;

            let target: DeleteTarget = req.json().await?;
            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;

            let deleted_config = repo.pop_config(&target.id).await?;
            Response::from_json(&deleted_config)
        })
        .run(req, env)
        .await
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();

    let config = RuntimeConfig::new(&env);
    let client = WorkerHttpClient;
    let secret_resolver = EnvSecretResolver(&env);

    let repo = match KvProjectRepository::new(&env, &config) {
        Ok(r) => r,
        Err(e) => {
            log_error!("Repo Init Error: {}", e);
            return;
        }
    };

    let global_read = env
        .secret(&config.github_token_name)
        .ok()
        .map(|s| s.to_string());
    let global_dispatch = env
        .secret(&config.pat_token_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    let service = WatchdogService::new(
        repo,
        &client,
        &secret_resolver,
        global_read,
        global_dispatch,
    );

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

    struct MockSecretResolver {
        secrets: HashMap<String, String>,
    }

    impl MockSecretResolver {
        fn new() -> Self {
            Self {
                secrets: HashMap::new(),
            }
        }
        fn with_secret(mut self, k: &str, v: &str) -> Self {
            self.secrets.insert(k.to_string(), v.to_string());
            self
        }
    }

    impl SecretResolver for MockSecretResolver {
        fn get_secret(&self, name: &str) -> Option<String> {
            self.secrets.get(name).cloned()
        }
    }

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
        async fn add_config(&self, base: BaseProjectConfig) -> Result<ProjectConfig> {
            let config = ProjectConfig::new(base);
            self.configs
                .borrow_mut()
                .retain(|c| c.generate_unique_key() != config.generate_unique_key());
            self.configs.borrow_mut().push(config.clone());
            Ok(config)
        }
        // 更新测试 Mock 实现
        async fn delete_config(&self, id: &str) -> Result<()> {
            self.configs.borrow_mut().retain(|c| c.unique_key != id);
            Ok(())
        }
        async fn pop_config(&self, id: &str) -> Result<Option<ProjectConfig>> {
            let mut configs = self.configs.borrow_mut();
            let pos = configs.iter().position(|c| c.unique_key == id);

            if let Some(index) = pos {
                Ok(Some(configs.remove(index)))
            } else {
                Ok(None)
            }
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

    #[tokio::test]
    async fn test_update_flow_with_custom_secret() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
        let resolver = MockSecretResolver::new().with_secret("MY_CUSTOM_TOKEN", "secret_value_123");

        let base_config = BaseProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token_secret: Some("MY_CUSTOM_TOKEN".into()),
            comparison_mode: ComparisonMode::PublishedAt,
        };

        let config = repo.add_config(base_config).await.unwrap();

        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
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

        let reqs = client.requests.borrow();
        let dispatch_req = reqs.iter().find(|r| r.0.contains("/dispatches")).unwrap();
        let headers = &dispatch_req.2;
        assert_eq!(
            headers.get("Authorization").expect("Missing Auth Header"),
            "Bearer secret_value_123"
        );
    }

    #[tokio::test]
    async fn test_delete_api_logic() {
        let repo = MockRepository::new();
        let base_config = BaseProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token_secret: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };
        let config = repo.add_config(base_config.clone()).await.unwrap();

        // 1. Pop Existing (Old Logic)
        let popped = repo.pop_config(&config.unique_key).await.unwrap();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().unique_key, config.unique_key);

        // 2. Add again for standard delete test
        repo.add_config(base_config).await.unwrap();

        // 3. Standard Delete (New Logic)
        repo.delete_config(&config.unique_key).await.unwrap();
        // Verify deletion
        let remaining = repo.get_all_configs().await.unwrap();
        assert!(remaining.is_empty());
    }
}
