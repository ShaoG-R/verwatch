use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use worker::*;

pub mod api;
pub mod request;

use api::GitHubGateway;
use request::{HttpClient, WorkerHttpClient};

// =========================================================
// 常量定义 (Constants) - 消除 Magic Strings
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

/// 基础配置字段，用于被 ProjectConfig 包含以及作为 API 请求体
/// 使用 #[serde(flatten)] 实现复用
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BaseProjectConfig {
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,
    pub dispatch_token: Option<String>,
    pub comparison_mode: ComparisonMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub unique_key: String,
    // 扁平化：JSON 中这些字段和 unique_key 处于同一层级
    #[serde(flatten)]
    pub base: BaseProjectConfig,
}

/// 创建请求只需要 Base 部分
/// 这里直接使用 BaseProjectConfig 的别名，或者包装一层，
/// 但为了演示 flatten 的效果，我们在 POST 处理中直接使用 BaseProjectConfig
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
        format!("{}{}/{}", PREFIX_VERSION, self.base.upstream_owner, self.base.upstream_repo)
    }

    pub fn generate_unique_key(&self) -> String {
        format!(
            "{}/{}->{}/{}",
            self.base.upstream_owner, self.base.upstream_repo, self.base.my_owner, self.base.my_repo
        )
    }

    pub fn get_dispatch_token<'a>(&'a self, global_token: &'a str) -> &'a str {
        self.base.dispatch_token.as_deref().unwrap_or(global_token)
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
    async fn delete_config(&self, id: &str) -> Result<bool>;
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
        // 优化：利用 List 的 Metadata 直接获取 Config，避免 N+1 次 Get 请求
        // 假设 Config 很小 (< 1KB)，可以直接存入 Metadata
        let list = self.kv.list().prefix(PREFIX_PROJECT.to_string()).execute().await?;
        
        let mut configs = Vec::new();
        let mut keys_without_meta = Vec::new();

        for key in list.keys {
            if let Some(meta) = key.metadata {
                // 尝试从 Metadata 反序列化
                if let Ok(cfg) = serde_json::from_value::<ProjectConfig>(meta) {
                    configs.push(cfg);
                } else {
                    // Metadata 损坏或格式旧，回退到 Get
                    keys_without_meta.push(key.name);
                }
            } else {
                // 无 Metadata，回退到 Get
                keys_without_meta.push(key.name);
            }
        }

        // 如果有部分 Key 没有 Metadata（例如旧数据），并行补全获取
        if !keys_without_meta.is_empty() {
             let futures = keys_without_meta.iter().map(|k| self.kv.get(k).json::<ProjectConfig>());
             // 并发获取剩余数据
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

        // 运行时检查：KV Metadata 限制为 1024 字节
        // 因为 String 是动态大小，无法在编译期确定 JSON 后的长度，必须运行时检查
        let serialized_json = serde_json::to_string(&config)?;
        let json_len = serialized_json.len();

        let mut query = self.kv.put(&key, &config)?; // Put Value (限制 25MB)

        if json_len < 1024 {
            // 只有小于 1KB 才存入 metadata 进行列表优化
            query = query.metadata(&config)?;
        } else {
            // 如果超大，仅记录日志，不写入 metadata，保证 save 操作成功
            log_info!("Config size ({} bytes) exceeds metadata limit (1024), skipping optimization.", json_len);
        }

        query.execute().await?;

        Ok(config)
    }

    async fn delete_config(&self, id: &str) -> Result<bool> {
        let project_key = format!("{}{}", PREFIX_PROJECT, id);
        // KV delete 是幂等的，其实可以不用先 check 再 delete，除非为了返回准确的 bool
        // 这里为了性能，可以直接 delete。如果必须要知道是否存在，为了避免 Race condition 和 额外开销，
        // 我们通常假设删除成功。
        // 但为了保持 API 语义 (404 Not Found)，我们还是只能 check。
        // 优化：使用 list prefix 来 check 是否存在，可能比 get text 便宜？不一定。
        // 维持原逻辑，但使用常量。
        
        let val = self.kv.get(&project_key).text().await?;
        if val.is_some() {
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
        // 注意：这里的访问路径变成了 config.base.upstream_owner
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

        let token = config.get_dispatch_token(&self.global_dispatch_token);
        self.gateway
            .trigger_dispatch(config, &release.tag_name, token)
            .await?;
        self.repo
            .update_last_version_time(config, remote_time)
            .await?;

        Ok(format!(
            "Updated {}/{} to {}",
            config.base.upstream_owner, config.base.upstream_repo, release.tag_name
        ))
    }

    // 优化：并发执行所有检查任务
    async fn run_all(&self) -> Result<String> {
        let configs = self.repo.get_all_configs().await?;
        if configs.is_empty() {
            return Ok("No projects configured.".to_string());
        }

        // 设置并发度：同时处理 5 个请求
        const CONCURRENCY_LIMIT: usize = 5;

        // 使用 stream 处理并发
        let results = stream::iter(configs)
            .map(|config| async move {
                // async block 捕获 config
                match self.check_project(&config).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Error checking {}: {}", config.base.upstream_repo, e),
                }
            })
            // buffer_unordered 允许并发执行，并不保证结果顺序（这里顺序不重要）
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

/// 安全的恒定时间比较，防止时序攻击 (Timing Attack)
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    a_bytes.iter().zip(b_bytes).fold(0, |acc, (&x, &y)| acc | (x ^ y)) == 0
}

fn ensure_admin_auth(req: &Request, env: &Env, config: &RuntimeConfig) -> Result<()> {
    let auth_header = req.headers().get(HEADER_AUTH_KEY)?.unwrap_or_default();

    let secret = env
        .secret(&config.admin_secret_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    // 使用恒定时间比较
    if secret.is_empty() || !constant_time_eq(&auth_header, &secret) {
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

            // 使用 BaseProjectConfig，利用 flatten 特性
            let req_data: BaseProjectConfig = req.json().await?;
            
            let repo = KvProjectRepository::new(&ctx.env, &cfg)?;
            // 移交所有权，减少 clone
            let saved_config = repo.add_config(req_data).await?;

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
                Response::from_body(ResponseBody::Body(vec![])) // 200 OK
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

    let config = RuntimeConfig::new(&env);
    let client = WorkerHttpClient;

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
        async fn add_config(&self, base: BaseProjectConfig) -> Result<ProjectConfig> {
            let config = ProjectConfig::new(base);
            self.configs
                .borrow_mut()
                .retain(|c| c.generate_unique_key() != config.generate_unique_key());
            self.configs.borrow_mut().push(config.clone());
            Ok(config)
        }
        async fn delete_config(&self, id: &str) -> Result<bool> {
            let len_before = self.configs.borrow().len();
            self.configs.borrow_mut().retain(|c| c.unique_key != id);
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

    #[tokio::test]
    async fn test_update_flow() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
        
        let base_config = BaseProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };

        // 直接添加，测试 add_config 逻辑
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

        let service = WatchdogService::new(repo, &client, None, "token".into());
        let res = service.run_all().await.unwrap();

        assert!(res.contains("Updated u/r to v2"));
    }

    #[tokio::test]
    async fn test_no_update() {
        let repo = MockRepository::new();
        let client = MockHttpClient::new();
         let base_config = BaseProjectConfig {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };

        let config = repo.add_config(base_config).await.unwrap();

        repo.update_last_version_time(&config, "2023-01-01T00:00:00Z")
            .await
            .unwrap();

        client.mock_response(
            "https://api.github.com/repos/u/r/releases/latest",
            200,
            json!({ "tag_name": "v1", "published_at": "2023-01-01T00:00:00Z" }),
        );

        let service = WatchdogService::new(repo, &client, None, "token".into());
        let res = service.run_all().await.unwrap();

        assert!(res.contains("No change"));
    }
}