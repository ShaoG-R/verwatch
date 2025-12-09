use worker::*;

pub mod service;
pub(crate) mod utils {
    pub mod repository;
    pub mod request;
}

use service::{EnvSecretResolver, WatchdogService};
use utils::repository::{KvProjectRepository, Repository};
use utils::request::WorkerHttpClient;

// =========================================================
// 常量定义 (Constants)
// =========================================================

use verwatch_shared::{CreateProjectRequest, DeleteTarget, HEADER_AUTH_KEY, ProjectConfig};

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
// 响应处理宏 (Response Handling Macros)
// =========================================================

macro_rules! unwrap_or_resp {
    ($expr:expr, $log_prefix:expr, $code:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                log_error!("{}: {}", $log_prefix, e);
                let msg = match $code {
                    400 => "Bad Request",
                    _ => "Internal Server Error",
                };
                return Response::error(msg, $code);
            }
        }
    };
}

macro_rules! respond {
    (json, $expr:expr, $log_prefix:expr) => {
        match $expr {
            Ok(v) => Response::from_json(&v),
            Err(e) => {
                log_error!("{}: {}", $log_prefix, e);
                Response::error("Internal Server Error", 500)
            }
        }
    };
    (empty, $expr:expr, $log_prefix:expr) => {
        match $expr {
            Ok(_) => Response::empty(),
            Err(e) => {
                log_error!("{}: {}", $log_prefix, e);
                Response::error("Internal Server Error", 500)
            }
        }
    };
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

fn ensure_admin_auth(
    req: &Request,
    env: &Env,
    config: &RuntimeConfig,
) -> std::result::Result<(), Response> {
    let check = |req: &Request| -> Result<()> {
        let auth_header = req.headers().get(HEADER_AUTH_KEY)?.unwrap_or_default();

        let secret = env
            .secret(&config.admin_secret_name)
            .map(|s| s.to_string())
            .unwrap_or_default();

        if secret.is_empty() || !constant_time_eq(&auth_header, &secret) {
            return Err(Error::from("Unauthorized"));
        }
        Ok(())
    };

    if let Err(e) = check(req) {
        log_error!("Auth Check Failed: {}", e);
        // Explicitly return 401 Response on failure
        return Err(Response::error("Unauthorized", 401).unwrap());
    }
    Ok(())
}

async fn list_projects(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(res) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(res);
    }

    let repo = unwrap_or_resp!(
        KvProjectRepository::new(&ctx.env, &cfg.kv_binding),
        "Repo init failed",
        500
    );
    // Use list_projects
    respond!(json, repo.list_projects().await, "Get configs failed")
}

async fn create_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(res) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(res);
    }

    let req_data: CreateProjectRequest =
        unwrap_or_resp!(req.json().await, "Invalid request body", 400);

    let repo = unwrap_or_resp!(
        KvProjectRepository::new(&ctx.env, &cfg.kv_binding),
        "Repo init failed",
        500
    );

    // Construct config here (Controller Logic) and save
    let config = ProjectConfig::new(req_data);
    let result = repo.save_project(&config).await.map(|_| config);

    respond!(json, result, "Add config failed")
}

async fn delete_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(res) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(res);
    }

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, "Invalid request body", 400);
    let repo = unwrap_or_resp!(
        KvProjectRepository::new(&ctx.env, &cfg.kv_binding),
        "Repo init failed",
        500
    );

    // Use delete_project
    respond!(
        empty,
        repo.delete_project(&target.id).await,
        "Delete config failed"
    )
}

async fn pop_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(res) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(res);
    }

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, "Invalid request body", 400);
    let repo = unwrap_or_resp!(
        KvProjectRepository::new(&ctx.env, &cfg.kv_binding),
        "Repo init failed",
        500
    );

    // Pop logic moved to Controller: get -> delete
    let result: Result<Option<ProjectConfig>> = async {
        let current = repo.get_project(&target.id).await?;
        if let Some(c) = &current {
            repo.delete_project(&c.unique_key).await?;
        }
        Ok(current)
    }
    .await;

    respond!(json, result, "Pop config failed")
}

async fn toggle_pause_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(res) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(res);
    }

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, "Invalid request body", 400);
    let repo = unwrap_or_resp!(
        KvProjectRepository::new(&ctx.env, &cfg.kv_binding),
        "Repo init failed",
        500
    );

    respond!(
        json,
        repo.toggle_pause_project(&target.id).await,
        "Toggle pause failed"
    )
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let cors = Cors::new()
        .with_origins(vec!["*"])
        .with_methods(vec![
            Method::Get,
            Method::Post,
            Method::Delete,
            Method::Options,
        ])
        .with_allowed_headers(vec!["Content-Type", HEADER_AUTH_KEY]);

    let router = Router::new();

    router
        .get_async("/api/projects", list_projects)
        .post_async("/api/projects", create_project)
        .delete_async("/api/projects", delete_project)
        .delete_async("/api/projects/pop", pop_project)
        .post_async("/api/projects/toggle_pause", toggle_pause_project)
        .options_async("/api/projects", |_, _| async { Response::empty() })
        .options_async("/api/projects/pop", |_, _| async { Response::empty() })
        .options_async("/api/projects/toggle_pause", |_, _| async {
            Response::empty()
        })
        .run(req, env)
        .await?
        .with_cors(&cors)
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();

    let config = RuntimeConfig::new(&env);
    let client = WorkerHttpClient;
    let secret_resolver = EnvSecretResolver(&env);

    let repo = match KvProjectRepository::new(&env, &config.kv_binding) {
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
